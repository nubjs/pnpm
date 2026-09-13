//! The command a host's command line names.
//!
//! A host embedding the engine serves some commands itself, so before it
//! runs anything it has to know which command a command line names. Only
//! this grammar can say: an option written before the command may consume
//! the token after it, so the first bare token is not reliably the
//! command, and a host scanning for one of its own would have to keep a
//! copy of every option's arity to find it.

use crate::{
    boolean_negations::with_boolean_negations,
    cli_args::CliArgs,
    config_overrides::ConfigOverrides,
    flag_relocation::{ArgTable, short_cluster_consumes_value, token_width},
    renamed_options, shorthands,
};
use clap::CommandFactory;
use std::ffi::OsString;

/// The canonical name of the command `argv` names, whatever spelling it
/// used, with `recursive` followed through to the command it wraps.
///
/// `None` when the command line names none of the engine's commands: it
/// carries only options, its first bare token is a script name — which
/// [`crate::run`] dispatches to `run` and a host may spell its own way —
/// or it holds an option this grammar does not declare, which is a
/// host's own and whose value must not be read as a command.
///
/// This only parses. Nothing runs, no configuration is read, and no
/// command line is rewritten — the answer describes `argv` as given.
#[must_use]
pub fn command_name(argv: &[OsString]) -> Option<String> {
    // The setting flags come out first, as they do before the parse
    // itself: they are not clap options, so a scan that meets one has no
    // arity for it and would read the value after it as the command.
    let (_, argv) = ConfigOverrides::extract(argv.to_vec());
    let command = with_boolean_negations(CliArgs::command());
    let argv = shorthands::expand_universal_shorthands(&command, argv);
    let argv = renamed_options::drop_shadowed_aliases(&command, argv);
    let top_level = ArgTable::top_level(&command);
    let subcommand_union = ArgTable::subcommand_union(&command);
    let mut index = command_index(&argv, 1, &top_level, &subcommand_union)?;
    loop {
        let subcommand = command.find_subcommand(&argv[index])?;
        if subcommand.get_name() != "recursive" {
            return Some(subcommand.get_name().to_string());
        }
        index = command_index(&argv, index + 1, &top_level, &subcommand_union)?;
    }
}

/// The index of the token naming the command, stepping over the options
/// this grammar declares.
///
/// Deliberately stricter than the scan the parse itself uses, which steps
/// over an undeclared option as a flag so clap can report it: a host
/// writes options of its own ahead of the command, and reading one of
/// those as a flag takes its VALUE for the command. `--require add
/// script.js` asks a runtime to preload a module; answering `add` would
/// have the host install a dependency instead.
fn command_index(
    argv: &[OsString],
    mut index: usize,
    top_level: &ArgTable,
    subcommand_union: &ArgTable,
) -> Option<usize> {
    loop {
        let token = argv.get(index)?.to_str()?;
        if token == "--" {
            return None;
        }
        let Some(rest) = token.strip_prefix('-').filter(|rest| !rest.is_empty()) else {
            return Some(index);
        };
        index += declared_option_width(rest, top_level, subcommand_union)?;
    }
}

/// How many tokens an option occupies, or `None` when neither table
/// declares it. `rest` is the token with its leading `-` stripped.
fn declared_option_width(
    rest: &str,
    top_level: &ArgTable,
    subcommand_union: &ArgTable,
) -> Option<usize> {
    if let Some(long) = rest.strip_prefix('-') {
        let (name, has_inline_value) =
            long.split_once('=').map_or((long, false), |(name, _)| (name, true));
        let consumes_value = top_level
            .long_consumes_value(name)
            .or_else(|| subcommand_union.long_consumes_value(name))?;
        return Some(token_width(consumes_value, has_inline_value));
    }
    let mut undeclared = false;
    let consumes_value = short_cluster_consumes_value(rest, |short| {
        let arity = top_level
            .short_consumes_value(short)
            .or_else(|| subcommand_union.short_consumes_value(short));
        undeclared |= arity.is_none();
        arity
    });
    (!undeclared).then(|| token_width(consumes_value, false))
}

#[cfg(test)]
mod tests;
