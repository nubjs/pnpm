//! Put the running program's name into the help text.
//!
//! [`crate::prepare_cli_argv`] already gives the parser the host's `name` and
//! `bin_name`, so the usage line names whoever is running. The prose does not
//! move with it: `about`, `long_about`, `help` and `long_help` are string
//! literals from the doc comments on [`crate::cli_args`], and clap prints them
//! as written. Unlike a diagnostic, help never passes through
//! [`pnpm_default_reporter`], so a host embedding the engine has nothing to
//! hook — `--help` tells its users about a program they never installed.
//!
//! This pass is that hook. It runs over the built [`Command`] tree, so the doc
//! comments stay pnpm's own and only the rendering moves.

use clap::{Arg, Command};

/// pnpm's name as it appears in the help text this pass rewrites.
const PNPM: &str = "pnpm";

/// Rewrite every help string in `cmd` to name `program` instead of pnpm.
///
/// Returns `cmd` untouched when `program` is pnpm itself, so the standalone
/// binary's help is byte-identical by construction rather than by a
/// substitution that happens to be an identity.
pub fn with_program_name(cmd: Command, program: &str) -> Command {
    if program == PNPM {
        return cmd;
    }
    rewrite(cmd, program)
}

fn rewrite(mut cmd: Command, program: &str) -> Command {
    if let Some(about) = cmd.get_about().map(ToString::to_string) {
        cmd = cmd.about(substitute(&about, program));
    }
    if let Some(long) = cmd.get_long_about().map(ToString::to_string) {
        cmd = cmd.long_about(substitute(&long, program));
    }

    let ids: Vec<clap::Id> = cmd.get_arguments().map(|arg| arg.get_id().clone()).collect();
    for id in ids {
        cmd = cmd.mut_arg(id, |arg| rewrite_arg(arg, program));
    }

    let names: Vec<String> = cmd.get_subcommands().map(|sub| sub.get_name().to_string()).collect();
    for name in names {
        cmd = cmd.mut_subcommand(name, |sub| rewrite(sub, program));
    }

    cmd
}

fn rewrite_arg(mut arg: Arg, program: &str) -> Arg {
    if let Some(help) = arg.get_help().map(ToString::to_string) {
        arg = arg.help(substitute(&help, program));
    }
    if let Some(long) = arg.get_long_help().map(ToString::to_string) {
        arg = arg.long_help(substitute(&long, program));
    }
    arg
}

/// Whether the `pnpm` at `at` is the program's name rather than part of a
/// filename, a path segment or a hostname.
///
/// The distinction is what makes this a substitution and not a replace.
/// `pnpm-lock.yaml`, `.pnpmfile.cjs`, `node_modules/.pnpm` and `pnpm.io` name
/// real things on disk and on the network, and those keep their names whoever
/// is running — rewriting one would print a path that does not exist.
fn is_the_program_name(text: &str, at: usize) -> bool {
    let joins = |c: u8| c.is_ascii_alphanumeric() || matches!(c, b'_' | b'-' | b'/');
    let bytes = text.as_bytes();
    if at > 0 && (joins(bytes[at - 1]) || bytes[at - 1] == b'.') {
        return false;
    }
    match bytes.get(at + PNPM.len()) {
        None => true,
        Some(&next) if joins(next) => false,
        // A trailing dot ends a sentence unless something follows it, which
        // makes it a hostname (`pnpm.io`) or a filename.
        Some(b'.') => !bytes.get(at + PNPM.len() + 1).is_some_and(u8::is_ascii_alphanumeric),
        Some(_) => true,
    }
}

fn substitute(text: &str, program: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    let mut consumed = 0;
    while let Some(at) = rest.find(PNPM) {
        let (before, from) = rest.split_at(at);
        out.push_str(before);
        rest = &from[PNPM.len()..];
        out.push_str(if is_the_program_name(text, consumed + at) { program } else { PNPM });
        consumed += at + PNPM.len();
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
mod tests;
