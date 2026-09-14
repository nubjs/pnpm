//! Put the running program's names into the help text.
//!
//! [`crate::prepare_cli_argv`] already gives the parser the host's `name` and
//! `bin_name`, so the usage line names whoever is running. The prose does not
//! move with it: `about`, `long_about`, `help` and `long_help` are string
//! literals from the doc comments on [`crate::cli_args`], and clap prints them
//! as written. Unlike a diagnostic, help never passes through
//! [`pnpm_default_reporter`], so a host embedding the engine has nothing to
//! hook — `--help` tells its users about a program they never installed, and
//! about files that program never reads.
//!
//! This pass is that hook. It runs over the built [`Command`] tree, so the doc
//! comments stay pnpm's own and only the rendering moves.

use clap::{Arg, Command};
use pnpm_config::Embedder;

/// pnpm's name as it appears in the help text this pass rewrites.
const PNPM: &str = "pnpm";

/// pnpm's lockfile, as the help text spells it.
const PNPM_LOCKFILE: &str = pnpm_lockfile::Lockfile::FILE_NAME;

/// pnpm's settings file, as the help text spells it.
const PNPM_SETTINGS_FILE: &str = "pnpm-workspace.yaml";

/// The names the help text uses, as the running program gives them.
struct HostNames {
    program: &'static str,
    lockfile: &'static str,
    /// `None` while the host reads `pnpm-workspace.yaml` itself, which keeps
    /// every sentence naming that file true.
    settings_file: Option<&'static str>,
    /// Whether the host declares a workspace's projects in `package.json`'s
    /// `workspaces` instead of in `pnpm-workspace.yaml`.
    workspaces_in_manifest: bool,
}

impl HostNames {
    fn of(embedder: &Embedder) -> Self {
        let own_settings = !embedder.reads_pnpm_config;
        HostNames {
            program: embedder.program_name,
            lockfile: embedder.lockfile_basename,
            settings_file: own_settings.then_some(embedder.settings_file_display_name),
            workspaces_in_manifest: own_settings && embedder.workspaces_from_package_manifest,
        }
    }

    fn are_pnpms(&self) -> bool {
        self.program == PNPM
            && self.lockfile == PNPM_LOCKFILE
            && self.settings_file.is_none()
            && !self.workspaces_in_manifest
    }
}

/// Rewrite every help string in `cmd` to use the names `embedder` gives.
///
/// Returns `cmd` untouched under pnpm's own names, so the standalone binary's
/// help is byte-identical by construction rather than by a substitution that
/// happens to be an identity.
pub fn with_host_names(cmd: Command, embedder: &Embedder) -> Command {
    let names = HostNames::of(embedder);
    if names.are_pnpms() {
        return cmd;
    }
    rewrite(cmd, &names)
}

fn rewrite(mut cmd: Command, names: &HostNames) -> Command {
    if let Some(about) = cmd.get_about().map(ToString::to_string) {
        cmd = cmd.about(substitute(&about, names));
    }
    if let Some(long) = cmd.get_long_about().map(ToString::to_string) {
        cmd = cmd.long_about(substitute(&long, names));
    }

    let ids: Vec<clap::Id> = cmd.get_arguments().map(|arg| arg.get_id().clone()).collect();
    for id in ids {
        cmd = cmd.mut_arg(id, |arg| rewrite_arg(arg, names));
    }

    let subcommands: Vec<String> =
        cmd.get_subcommands().map(|sub| sub.get_name().to_string()).collect();
    for name in subcommands {
        cmd = cmd.mut_subcommand(name, |sub| rewrite(sub, names));
    }

    cmd
}

fn rewrite_arg(mut arg: Arg, names: &HostNames) -> Arg {
    if names.workspaces_in_manifest
        && let Some(help) = workspace_declaration_help(arg.get_id().as_str())
    {
        return arg.help(help);
    }
    if let Some(help) = arg.get_help().map(ToString::to_string) {
        arg = arg.help(substitute(&help, names));
    }
    if let Some(long) = arg.get_long_help().map(ToString::to_string) {
        arg = arg.long_help(substitute(&long, names));
    }
    arg
}

/// Help for the two flags that describe where a workspace's projects are
/// declared, under a host that declares them in `package.json`.
///
/// Rewritten whole rather than substituted: pnpm's sentences name a field of
/// `pnpm-workspace.yaml`, and the host's declaration is a different field in a
/// different file, which no replacement of a file name reaches.
fn workspace_declaration_help(arg_id: &str) -> Option<&'static str> {
    match arg_id {
        "ignore_workspace" => Some(
            "Run as if the project were standalone, ignoring the `workspaces` field of any `package.json` above it",
        ),
        "workspace_packages" => Some(
            "Glob patterns selecting the workspace's projects, overriding the `workspaces` field of `package.json`. Repeat to add more",
        ),
        _ => None,
    }
}

fn substitute(text: &str, names: &HostNames) -> String {
    let mut text = replace_standalone(text, PNPM_LOCKFILE, names.lockfile);
    if let Some(settings_file) = names.settings_file {
        text = replace_standalone(&text, PNPM_SETTINGS_FILE, settings_file);
    }
    replace_standalone(&text, PNPM, names.program)
}

/// Whether the `len` bytes at `at` stand alone rather than as part of a longer
/// name: a filename, a path segment or a hostname.
///
/// The distinction is what makes this a substitution and not a replace.
/// `.pnpmfile.cjs`, `node_modules/.pnpm`, `pnpm-lock.<branch>.yaml` and
/// `pnpm.io` name real things on disk and on the network, and those keep their
/// names whoever is running — rewriting one would print a path that does not
/// exist.
fn stands_alone(text: &str, at: usize, len: usize) -> bool {
    let joins = |c: u8| c.is_ascii_alphanumeric() || matches!(c, b'_' | b'-' | b'/');
    let bytes = text.as_bytes();
    if at > 0 && (joins(bytes[at - 1]) || bytes[at - 1] == b'.') {
        return false;
    }
    match bytes.get(at + len) {
        None => true,
        Some(&next) if joins(next) => false,
        // A trailing dot ends a sentence unless something follows it, which
        // makes it a hostname (`pnpm.io`) or a longer filename.
        Some(b'.') => !bytes.get(at + len + 1).is_some_and(u8::is_ascii_alphanumeric),
        Some(_) => true,
    }
}

/// Replace each occurrence of `from` that [stands alone](stands_alone).
fn replace_standalone(text: &str, from: &str, to: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut copied = 0;
    for (at, _) in text.match_indices(from) {
        if stands_alone(text, at, from.len()) {
            out.push_str(&text[copied..at]);
            out.push_str(to);
            copied = at + from.len();
        }
    }
    out.push_str(&text[copied..]);
    out
}

#[cfg(test)]
mod tests;
