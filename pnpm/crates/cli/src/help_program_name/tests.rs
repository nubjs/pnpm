use clap::{Arg, Command};

use super::{substitute, with_program_name};

/// The standalone binary must not be rewritten at all, so its help cannot
/// drift as this pass changes. Comparing the RENDERED help rather than one
/// field is what makes this a real guard: an `about`-only assertion stays
/// green while every argument's help silently moves.
#[test]
fn pnpms_own_help_is_left_exactly_as_written() {
    let built = || {
        Command::new("pnpm")
            .about("Directory in which pnpm persists state")
            .arg(Arg::new("state").long("state-dir").help("Mirrors pnpm's universal `--loglevel`"))
            .subcommand(Command::new("store").about("Functionally equivalent to pnpm add"))
    };
    assert_eq!(
        with_program_name(built(), "pnpm").render_help().to_string(),
        built().render_help().to_string()
    );
}

/// ...and the same rendering really does move for a host, so the test above
/// is pinning the guard rather than a pass that never does anything.
#[test]
fn a_hosts_rendered_help_differs_from_pnpms() {
    let built = || Command::new("pnpm").about("pnpm installs packages");
    assert_ne!(
        with_program_name(built(), "nub").render_help().to_string(),
        built().render_help().to_string()
    );
}

/// A host's users are told about the program they ran, in the command's own
/// description and in every argument's help, at any depth.
#[test]
fn a_hosts_name_reaches_the_whole_command_tree() {
    let cmd = Command::new("pnpm")
        .about("pnpm installs packages")
        .arg(Arg::new("state").long("state-dir").help("Directory in which pnpm persists state"))
        .subcommand(
            Command::new("store")
                .about("Functionally equivalent to pnpm add")
                .arg(Arg::new("dir").long("dir").long_help("Where pnpm writes the store")),
        );
    let cmd = with_program_name(cmd, "nub");

    assert_eq!(cmd.get_about().unwrap().to_string(), "nub installs packages");
    let arg = cmd.get_arguments().find(|a| a.get_long() == Some("state-dir")).unwrap();
    assert_eq!(arg.get_help().unwrap().to_string(), "Directory in which nub persists state");

    let sub = cmd.get_subcommands().find(|s| s.get_name() == "store").unwrap();
    assert_eq!(sub.get_about().unwrap().to_string(), "Functionally equivalent to nub add");
    let nested = sub.get_arguments().find(|a| a.get_long() == Some("dir")).unwrap();
    assert_eq!(nested.get_long_help().unwrap().to_string(), "Where nub writes the store");
}

/// A file, a path segment and a host are not the program's name: they exist
/// under those names whoever is running, so a rewrite would point the reader
/// at something that is not there. This is the assertion that makes the pass
/// a word-boundary walk rather than a replace.
#[test]
fn a_name_that_is_not_the_program_survives_verbatim() {
    for (written, expected) in [
        ("Resolve the conflict in pnpm-lock.yaml", "Resolve the conflict in pnpm-lock.yaml"),
        ("ignoring .pnpmfile.cjs and .pnpmrc", "ignoring .pnpmfile.cjs and .pnpmrc"),
        ("linked from node_modules/.pnpm/is-odd", "linked from node_modules/.pnpm/is-odd"),
        ("see https://pnpm.io/errors", "see https://pnpm.io/errors"),
        ("writes a pnpm-workspace.yaml", "writes a pnpm-workspace.yaml"),
        // The mixed case is the point: the FILE keeps its name while the
        // program beside it takes the host's.
        ("run pnpm install, then read pnpm-lock.yaml", "run nub install, then read pnpm-lock.yaml"),
        // Sentence-final: a dot ends it, so the name is still the program's.
        ("not yet implemented in pnpm.", "not yet implemented in nub."),
    ] {
        assert_eq!(substitute(written, "nub"), expected, "input: {written}");
    }
}
