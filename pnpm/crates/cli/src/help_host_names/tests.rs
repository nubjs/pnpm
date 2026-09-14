use clap::{Arg, Command, CommandFactory};
use pnpm_config::Embedder;

use super::{HostNames, substitute, with_host_names};
use crate::cli_args::CliArgs;

/// A host with names of its own throughout: its program, its lockfile, a
/// settings file of its own, workspaces declared in `package.json`, and
/// overrides recorded there too rather than in the file its settings come
/// from.
const HOST: Embedder = Embedder {
    program_name: "host",
    lockfile_basename: "host.lock",
    settings_file_display_name: "host.jsonc",
    overrides_file_display_name: Some("package.json"),
    reads_pnpm_config: false,
    workspaces_from_package_manifest: true,
    ..Embedder::PNPM
};

/// Every command's help, as `--help` prints it, at any depth.
fn every_long_help(cmd: &mut Command) -> String {
    cmd.build();
    let mut help = cmd.render_long_help().to_string();
    for sub in cmd.get_subcommands_mut() {
        help.push_str(&every_long_help(sub));
    }
    help
}

/// The standalone binary must not be rewritten at all, so its help cannot
/// drift as this pass changes. Comparing the RENDERED help rather than one
/// field is what makes this a real guard: an `about`-only assertion stays
/// green while every argument's help silently moves.
#[test]
fn pnpms_own_help_is_left_exactly_as_written() {
    let built = || {
        Command::new("pnpm")
            .about("Directory in which pnpm persists state")
            .arg(
                Arg::new("state")
                    .long("state-dir")
                    .help("Mirrors pnpm-lock.yaml and pnpm-workspace.yaml"),
            )
            .subcommand(Command::new("store").about("Functionally equivalent to pnpm add"))
    };
    assert_eq!(
        with_host_names(built(), &Embedder::PNPM).render_help().to_string(),
        built().render_help().to_string()
    );
}

/// ...and the same rendering really does move for a host, so the test above
/// is pinning the guard rather than a pass that never does anything.
#[test]
fn a_hosts_rendered_help_differs_from_pnpms() {
    let built = || Command::new("pnpm").about("pnpm installs packages");
    assert_ne!(
        with_host_names(built(), &HOST).render_help().to_string(),
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
    let cmd = with_host_names(cmd, &HOST);

    assert_eq!(cmd.get_about().unwrap().to_string(), "host installs packages");
    let arg = cmd.get_arguments().find(|a| a.get_long() == Some("state-dir")).unwrap();
    assert_eq!(arg.get_help().unwrap().to_string(), "Directory in which host persists state");

    let sub = cmd.get_subcommands().find(|s| s.get_name() == "store").unwrap();
    assert_eq!(sub.get_about().unwrap().to_string(), "Functionally equivalent to host add");
    let nested = sub.get_arguments().find(|a| a.get_long() == Some("dir")).unwrap();
    assert_eq!(nested.get_long_help().unwrap().to_string(), "Where host writes the store");
}

/// Across the real command tree, a host's `--help` names neither of pnpm's
/// files, and the flags that locate a workspace's projects describe where the
/// host declares them. pnpm's own tree names both files, so their absence is
/// this pass's doing rather than help that never mentioned them.
#[test]
fn a_hosts_help_names_its_own_files_in_every_command() {
    let pnpm = every_long_help(&mut CliArgs::command());
    let host = every_long_help(&mut with_host_names(CliArgs::command(), &HOST));
    for file in ["pnpm-lock.yaml", "pnpm-workspace.yaml"] {
        assert!(pnpm.contains(file), "pnpm's help names {file}");
        assert!(!host.contains(file), "a host's help still names {file}");
    }
    assert!(host.contains("host.lock"), "a host's help names its lockfile");
    assert!(
        host.contains("overriding the `workspaces` field of `package.json`"),
        "--workspace-packages describes the host's workspace declaration"
    );
}

/// `audit --fix` writes an override, and a host that keeps overrides outside
/// the file its settings come from has that flag name the file it really
/// writes. Substituting the settings file is what this guards against: it
/// leaves the sentence readable and wrong, pointing the user at a file the
/// fix never touches.
#[test]
fn audit_fix_names_the_file_the_host_records_an_override_in() {
    let fix_help = |embedder: &Embedder| {
        let cmd = with_host_names(CliArgs::command(), embedder);
        let audit = cmd.get_subcommands().find(|s| s.get_name() == "audit").expect("audit");
        audit
            .get_arguments()
            .find(|a| a.get_long() == Some("fix"))
            .expect("--fix")
            .get_help()
            .expect("help")
            .to_string()
    };

    let host = fix_help(&HOST);
    assert!(host.contains("adds overrides to `package.json`"), "got: {host}");
    assert!(
        !host.contains("host.jsonc"),
        "the settings file is not where the override lands: {host}"
    );

    // The rest of the sentence is still pnpm's, so the rewrite replaces the
    // file rather than the flag's meaning.
    assert!(host.contains("re-resolves the lockfile to non-vulnerable versions"), "got: {host}");

    // A host that records overrides where pnpm does keeps the substituted
    // settings file, so the rewrite above is the new field's doing.
    let same_file = fix_help(&Embedder { overrides_file_display_name: None, ..HOST });
    assert!(same_file.contains("adds overrides to `host.jsonc`"), "got: {same_file}");

    // ...and pnpm's own help is untouched.
    let pnpm = fix_help(&Embedder::PNPM);
    assert!(pnpm.contains("adds overrides to `pnpm-workspace.yaml`"), "got: {pnpm}");
}

/// A file, a path segment and a host are not the program's name, nor pnpm's
/// lockfile or settings file: they exist under those names whoever is running,
/// so a rewrite would point the reader at something that is not there. This is
/// the assertion that makes the pass a word-boundary walk rather than a replace.
#[test]
fn a_name_that_is_not_the_programs_own_survives_verbatim() {
    let host = HostNames::of(&HOST);
    for (written, expected) in [
        ("ignoring .pnpmfile.cjs and .pnpmrc", "ignoring .pnpmfile.cjs and .pnpmrc"),
        ("linked from node_modules/.pnpm/is-odd", "linked from node_modules/.pnpm/is-odd"),
        ("see https://pnpm.io/errors", "see https://pnpm.io/errors"),
        // A branch lockfile keeps pnpm's pattern under every profile.
        (
            "fold pnpm-lock.<branch>.yaml into pnpm-lock.yaml",
            "fold pnpm-lock.<branch>.yaml into host.lock",
        ),
        ("the `pipelines` section of pnpm-workspace.yaml", "the `pipelines` section of host.jsonc"),
        // Sentence-final: a dot ends it, so the name is still the program's.
        ("run pnpm install, then read pnpm-lock.yaml.", "run host install, then read host.lock."),
    ] {
        assert_eq!(substitute(written, &host), expected, "input: {written}");
    }

    // A host that still reads pnpm's configuration reads `pnpm-workspace.yaml`
    // whatever it calls its own settings, so its help keeps naming that file.
    let reads_pnpm_config = HostNames::of(&Embedder {
        program_name: "host",
        settings_file_display_name: "host.jsonc",
        ..Embedder::PNPM
    });
    assert_eq!(
        substitute("writes a pnpm-workspace.yaml", &reads_pnpm_config),
        "writes a pnpm-workspace.yaml"
    );
}
