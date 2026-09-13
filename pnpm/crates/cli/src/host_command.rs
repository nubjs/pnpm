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
    flag_relocation::{ArgTable, find_positional},
    renamed_options,
};
use clap::CommandFactory;
use std::ffi::OsString;

/// The canonical name of the command `argv` names, whatever spelling it
/// used, with `recursive` followed through to the command it wraps.
///
/// `None` when the command line names none of the engine's commands: it
/// carries only options, or its first bare token is a script name, which
/// [`crate::run`] dispatches to `run` and a host may spell its own way.
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
    let argv = renamed_options::drop_shadowed_aliases(&command, argv);
    let top_level = ArgTable::top_level(&command);
    let subcommand_union = ArgTable::subcommand_union(&command);
    let mut index = find_positional(&argv, 1, &top_level, &subcommand_union)?;
    loop {
        let subcommand = command.find_subcommand(&argv[index])?;
        if subcommand.get_name() != "recursive" {
            return Some(subcommand.get_name().to_string());
        }
        index = find_positional(&argv, index + 1, &top_level, &subcommand_union)?;
    }
}

#[cfg(test)]
mod tests;
