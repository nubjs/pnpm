use crate::{Config, NodeLinker, WorkspaceSettings, embedder::Embedder};
use pretty_assertions::assert_eq;
use std::path::PathBuf;

/// A host that is not pnpm: its own naming, version management left to the
/// host rather than the engine, and a configuration file of its own.
const NUB: Embedder = Embedder {
    program_name: "nub",
    program_version: "0.0.0-test",
    manage_package_manager_versions: false,
    manage_runtimes: false,
    workspaces_from_package_manifest: true,
    lockfile_basename: "nub.lock",
    lockfile_legacy_basenames: &["lock.yaml"],
    virtual_store_dirname: ".store",
    reads_pnpm_config: false,
    reads_npm_config_env: true,
    writes_settings_file: false,
    workspace_settings: None,
    compat_package_extensions: None,
    allow_builds_writer: None,
    overrides_writer: None,
    patched_dependencies_writer: None,
    node_execpath: None,
    pnpm_execpath: None,

    script_bin_dir: None,
    extract_observer: None,
    materialize_policy: None,
    settings_file_display_name: "nub.jsonc",
    allow_builds_display_name: "allowScripts",
    hidden_modules_dir_entries: &[".nub-engine"],
    dlx_exits_like_child: false,
};

#[test]
fn default_profile_keeps_pnpm_naming() {
    let config = Config::default();
    assert_eq!(config.embedder.program_name, Embedder::PNPM.program_name);
    assert_eq!(config.embedder.program_name, "pnpm");
    assert!(config.embedder.manage_package_manager_versions);
    assert!(config.embedder.manage_runtimes);
    assert!(!config.embedder.workspaces_from_package_manifest);
    assert!(config.embedder.reads_pnpm_config);
    assert!(!config.embedder.reads_npm_config_env);
    assert!(config.embedder.workspace_settings.is_none());
    assert_eq!(config.embedder.compat_package_extensions, None);
    assert!(config.embedder.patched_dependencies_writer.is_none());
    assert_eq!(config.embedder.node_execpath, None);
    assert_eq!(config.embedder.pnpm_execpath, None);
    assert_eq!(config.wanted_lockfile_name(), "pnpm-lock.yaml");
    assert_eq!(config.embedder.virtual_store_dirname, ".pnpm");
    // pnpm's dlx IS the last thing the process does, so it becomes its
    // child's exit status; only an embedder needs the call to return.
    assert!(config.embedder.dlx_exits_like_child);
    let embedded = Config { embedder: NUB, ..Config::default() };
    assert!(!embedded.embedder.dlx_exits_like_child);
}

#[test]
fn embedder_profile_renames_the_wanted_lockfile() {
    let config = Config { embedder: NUB, ..Config::default() };
    assert_eq!(config.wanted_lockfile_name(), "nub.lock");
}

/// A git-branch lockfile is an explicit per-branch override, so it still wins
/// over the profile's basename — the profile only supplies the fallback.
#[test]
fn git_branch_lockfile_still_wins_over_the_profile() {
    let config = Config {
        embedder: NUB,
        git_branch_lockfile_name: Some("pnpm-lock.feature-x.yaml".to_string()),
        merge_git_branch_lockfiles: false,
        ..Config::default()
    };
    assert_eq!(config.wanted_lockfile_name(), "pnpm-lock.feature-x.yaml");
}

/// The derived virtual store follows the profile. This drives the real
/// anchoring step rather than repeating its arithmetic, so it fails if the
/// derivation stops consulting the profile.
#[test]
fn embedder_profile_renames_the_derived_virtual_store() {
    let start_dir = PathBuf::from("/tmp/project");

    let mut pnpm = Config::default();
    pnpm.anchor_default_module_dirs(&start_dir);
    assert_eq!(pnpm.virtual_store_dir, start_dir.join("node_modules").join(".pnpm"));

    let mut nub = Config { embedder: NUB, ..Config::default() };
    nub.anchor_default_module_dirs(&start_dir);
    assert_eq!(nub.virtual_store_dir, start_dir.join("node_modules").join(".store"));
}

/// The profile must survive the whole config cascade, since a host sets it on
/// the seed config and every later read happens after `current`. If any step
/// rebuilt the struct from defaults, the host's naming would be silently lost.
#[test]
fn profile_survives_the_config_cascade() {
    let dir = tempfile::tempdir().expect("create a temp project dir");
    let config = Config { embedder: NUB, ..Config::default() }
        .current::<crate::Host>(dir.path())
        .expect("load config");

    assert_eq!(config.embedder.program_name, NUB.program_name);
    assert_eq!(config.embedder.lockfile_basename, NUB.lockfile_basename);
    assert_eq!(config.wanted_lockfile_name(), "nub.lock");
    assert_eq!(config.virtual_store_dir, dir.path().join("node_modules").join(".store"));
}

/// Write a workspace whose membership is declared the npm way: a root
/// manifest with a `workspaces` array and one member under it.
fn write_manifest_workspace(root: &std::path::Path) {
    std::fs::write(
        root.join("package.json"),
        r#"{"name":"root","private":true,"workspaces":["packages/*"]}"#,
    )
    .expect("write the root manifest");
    let member = root.join("packages").join("a");
    std::fs::create_dir_all(&member).expect("create the member dir");
    std::fs::write(member.join("package.json"), r#"{"name":"@fx/a","version":"1.0.0"}"#)
        .expect("write the member manifest");
}

/// pnpm declares its projects in `pnpm-workspace.yaml` and reads the
/// manifest field for nothing but a warning, so the default profile finds
/// no workspace here at all. This is the control for the two tests below:
/// without it they would pass on a `Config` that simply always reads the
/// field.
#[test]
fn default_profile_ignores_the_manifest_workspaces_field() {
    let dir = tempfile::tempdir().expect("create a temp project dir");
    write_manifest_workspace(dir.path());

    let config = Config::default().current::<crate::Host>(dir.path()).expect("load config");

    assert_eq!(config.workspace_dir, None);
    assert_eq!(config.workspace_package_patterns, None);
}

/// With the profile on, the manifest's `workspaces` array marks the
/// workspace root and supplies the patterns that select its projects —
/// the two values the install path needs and that only
/// `pnpm-workspace.yaml` provides otherwise.
#[test]
fn embedder_profile_takes_the_workspace_from_the_package_manifest() {
    let dir = tempfile::tempdir().expect("create a temp project dir");
    write_manifest_workspace(dir.path());

    let config = Config { embedder: NUB, ..Config::default() }
        .current::<crate::Host>(dir.path())
        .expect("load config");

    assert_eq!(config.workspace_dir.as_deref(), Some(dir.path()));
    assert_eq!(config.workspace_package_patterns, Some(vec!["packages/*".to_string()]));
}

/// The object spelling of `workspaces` — `packages` beside Bun's catalogs —
/// declares the same projects as the array.
#[test]
fn the_object_spelling_of_manifest_workspaces_declares_the_projects_too() {
    let dir = tempfile::tempdir().expect("create a temp project dir");
    std::fs::write(
        dir.path().join("package.json"),
        r#"{"name":"root","workspaces":{"packages":["packages/*"],"catalog":{"react":"19.2.0"}}}"#,
    )
    .expect("write the root manifest");

    let config = Config { embedder: NUB, ..Config::default() }
        .current::<crate::Host>(dir.path())
        .expect("load config");

    assert_eq!(config.workspace_dir.as_deref(), Some(dir.path()));
    assert_eq!(config.workspace_package_patterns, Some(vec!["packages/*".to_string()]));
}

/// A command run from inside a member walks up to the root, the same way
/// the `pnpm-workspace.yaml` search does. Without this a member install
/// would treat its own directory as the whole workspace.
#[test]
fn the_manifest_workspace_root_is_found_from_a_subdirectory() {
    let dir = tempfile::tempdir().expect("create a temp project dir");
    write_manifest_workspace(dir.path());

    let config = Config { embedder: NUB, ..Config::default() }
        .current::<crate::Host>(&dir.path().join("packages").join("a"))
        .expect("load config");

    assert_eq!(config.workspace_dir.as_deref(), Some(dir.path()));
    assert_eq!(config.workspace_package_patterns, Some(vec!["packages/*".to_string()]));
}

/// For a profile that still reads pnpm's configuration, `pnpm-workspace.yaml`
/// is the authoritative declaration wherever it exists, so a project carrying
/// both files installs pnpm's projects, not the manifest's. The manifest
/// patterns here would select a different set, which is what makes the
/// assertion discriminate.
#[test]
fn the_workspace_yaml_still_wins_over_the_package_manifest() {
    let dir = tempfile::tempdir().expect("create a temp project dir");
    write_manifest_workspace(dir.path());
    std::fs::write(dir.path().join("pnpm-workspace.yaml"), "packages:\n  - libs/*\n")
        .expect("write the workspace manifest");

    let embedder = Embedder { reads_pnpm_config: true, ..NUB };
    let config = Config { embedder, ..Config::default() }
        .current::<crate::Host>(dir.path())
        .expect("load config");

    assert_eq!(config.workspace_dir.as_deref(), Some(dir.path()));
    assert_eq!(config.workspace_package_patterns, Some(vec!["libs/*".to_string()]));
}

/// A host that reads no pnpm configuration takes neither settings nor
/// projects from `pnpm-workspace.yaml`. The pnpm profile on the same
/// directory is the control: it shows the yaml's `nodeLinker` would
/// otherwise apply.
#[test]
fn a_host_that_reads_no_pnpm_config_ignores_the_workspace_yaml() {
    let dir = tempfile::tempdir().expect("create a temp project dir");
    write_manifest_workspace(dir.path());
    std::fs::write(
        dir.path().join("pnpm-workspace.yaml"),
        "packages:\n  - libs/*\nnodeLinker: hoisted\n",
    )
    .expect("write the workspace manifest");

    let pnpm = Config::default().current::<crate::Host>(dir.path()).expect("load config");
    assert_eq!(pnpm.node_linker, NodeLinker::Hoisted);

    let nub = Config { embedder: NUB, ..Config::default() }
        .current::<crate::Host>(dir.path())
        .expect("load config");
    assert_ne!(nub.node_linker, NodeLinker::Hoisted);
    assert_eq!(nub.workspace_package_patterns, Some(vec!["packages/*".to_string()]));
}

thread_local! {
    /// The settings the provider below answers with. A provider is a plain
    /// function, so what it answers has to live somewhere it can reach
    /// without capturing, and the engine asks while the test that set it is
    /// still running: a thread-local is read from that same thread, so it
    /// keeps one test's answer out of another's under a shared-process
    /// runner without a lock the provider would then have to take
    /// re-entrantly.
    static HOST_SETTINGS: std::cell::Cell<Option<&'static WorkspaceSettings>> =
        const { std::cell::Cell::new(None) };
}

fn provided_host_settings(_dir: &std::path::Path) -> Option<&'static WorkspaceSettings> {
    HOST_SETTINGS.get()
}

fn set_host_settings(settings: serde_json::Value) {
    HOST_SETTINGS.set(Some(Box::leak(Box::new(
        serde_json::from_value(settings).expect("parse the host settings"),
    ))));
}

/// Run `body` with `settings` as what the host answers.
fn with_host_settings<T>(settings: serde_json::Value, body: impl FnOnce(Embedder) -> T) -> T {
    set_host_settings(settings);
    let embedder = Embedder { workspace_settings: Some(provided_host_settings), ..NUB };
    let outcome = body(embedder);
    HOST_SETTINGS.set(None);
    outcome
}

/// A host's settings land where the yaml's would, explicit-setting record
/// included, and supplying them changes no project's workspace membership: a
/// single project stays one, and a member still resolves to its root.
#[test]
fn host_settings_apply_where_the_workspace_yaml_would() {
    with_host_settings(
        serde_json::json!({ "nodeLinker": "hoisted", "dedupePeers": true }),
        |embedder| {
            let single = tempfile::tempdir().expect("create a temp project dir");
            std::fs::write(
                single.path().join("package.json"),
                r#"{"name":"app","version":"1.0.0"}"#,
            )
            .expect("write the manifest");
            let config = Config { embedder, ..Config::default() }
                .current::<crate::Host>(single.path())
                .expect("load config");
            assert_eq!(config.node_linker, NodeLinker::Hoisted);
            assert!(config.dedupe_peers);
            assert_eq!(
                config.explicit_settings.get("nodeLinker"),
                Some(&serde_json::json!("hoisted"))
            );
            assert_eq!(config.workspace_dir, None);

            let workspace = tempfile::tempdir().expect("create a temp project dir");
            write_manifest_workspace(workspace.path());
            let config = Config { embedder, ..Config::default() }
                .current::<crate::Host>(&workspace.path().join("packages").join("a"))
                .expect("load config");
            assert_eq!(config.node_linker, NodeLinker::Hoisted);
            assert_eq!(config.workspace_dir.as_deref(), Some(workspace.path()));
        },
    );
}

/// `catalog:` specifiers resolve against `pnpm-workspace.yaml` read a second
/// time as a workspace manifest, which a host that supplies settings has no
/// equivalent of, so the host's catalogs have to reach `Config::catalogs` —
/// the override every consumer reads first. An explicit `catalogs.default`
/// wins over `catalog`, as it does for the manifest.
#[test]
fn host_settings_carry_the_catalogs_a_workspace_manifest_would() {
    let dir = tempfile::tempdir().expect("create a temp project dir");
    std::fs::write(dir.path().join("package.json"), r#"{"name":"app","version":"1.0.0"}"#)
        .expect("write the manifest");

    with_host_settings(
        serde_json::json!({
            "catalog": { "picocolors": "1.1.1" },
            "catalogs": { "default": { "picocolors": "1.1.0" }, "legacy": { "semver": "6.3.1" } },
        }),
        |embedder| {
            let config = Config { embedder, ..Config::default() }
                .current::<crate::Host>(dir.path())
                .expect("load config");
            let catalogs = config.catalogs.expect("the host catalogs reach the config");
            assert_eq!(catalogs["default"]["picocolors"], "1.1.0");
            assert_eq!(catalogs["legacy"]["semver"], "6.3.1");

            // pnpm's own profile reads its catalogs from the workspace manifest, so
            // nothing here fills the field for it.
            let config = Config::default().current::<crate::Host>(dir.path()).expect("load config");
            assert_eq!(config.catalogs, None);
        },
    );
}

/// Settings a host supplies can declare `patchedDependencies` for a project
/// with no workspace. Their paths resolve against the project root, so the
/// patches are hashed rather than dropped for want of a workspace directory.
#[test]
fn host_patched_dependencies_resolve_without_a_workspace() {
    let dir = tempfile::tempdir().expect("create a temp project dir");
    std::fs::write(dir.path().join("package.json"), r#"{"name":"app","version":"1.0.0"}"#)
        .expect("write the manifest");
    std::fs::create_dir(dir.path().join("patches")).expect("create the patches dir");
    std::fs::write(dir.path().join("patches").join("left-pad.patch"), "patch body\n")
        .expect("write the patch");

    with_host_settings(
        serde_json::json!({
            "patchedDependencies": { "left-pad@1.3.0": "patches/left-pad.patch" },
        }),
        |embedder| {
            let config = Config { embedder, ..Config::default() }
                .current::<crate::Host>(dir.path())
                .expect("load config");
            assert_eq!(config.workspace_dir, None);
            let hashes = config
                .patched_dependency_hashes()
                .expect("hash the configured patch")
                .expect("the host patch is configured");
            assert!(hashes.contains_key("left-pad@1.3.0"), "{hashes:?}");
        },
    );
}

/// With no pnpm configuration read there is no default pnpmfile to look for
/// either. `None` is what sends the hook finder looking for `.pnpmfile.cjs`;
/// an empty list runs none.
#[test]
fn a_host_that_reads_no_pnpm_config_runs_no_default_pnpmfile() {
    let dir = tempfile::tempdir().expect("create a temp project dir");
    std::fs::write(dir.path().join(".pnpmfile.cjs"), "module.exports = { hooks: {} }\n")
        .expect("write the pnpmfile");

    let pnpm = Config::default().current::<crate::Host>(dir.path()).expect("load config");
    assert_eq!(pnpm.pnpmfile, None);

    let nub = Config { embedder: NUB, ..Config::default() }
        .current::<crate::Host>(dir.path())
        .expect("load config");
    assert_eq!(nub.pnpmfile, Some(Vec::new()));
}

/// The host is asked for every configuration the engine builds, not once
/// for the profile: a command that writes the host's own settings and then
/// reloads — `approve-builds` recording an answer and rebuilding on it —
/// has to see what it just wrote, exactly as pnpm re-reads its workspace
/// manifest. One profile, two configurations, two different answers.
#[test]
fn the_host_is_asked_again_for_every_configuration() {
    let dir = tempfile::tempdir().expect("create a temp project dir");
    std::fs::write(dir.path().join("package.json"), r#"{"name":"app","version":"1.0.0"}"#)
        .expect("write the manifest");
    let embedder = Embedder { workspace_settings: Some(provided_host_settings), ..NUB };

    set_host_settings(serde_json::json!({ "nodeLinker": "hoisted" }));
    let before = Config { embedder, ..Config::default() }
        .current::<crate::Host>(dir.path())
        .expect("load config");
    assert_eq!(before.node_linker, NodeLinker::Hoisted);

    set_host_settings(serde_json::json!({ "nodeLinker": "isolated" }));
    let after = Config { embedder, ..Config::default() }
        .current::<crate::Host>(dir.path())
        .expect("load config");
    assert_eq!(after.node_linker, NodeLinker::Isolated);

    HOST_SETTINGS.set(None);
}

/// The profile's legacy names travel into the loader's selection. That trip
/// is the whole mechanism: the loader is where a file name is resolved, and
/// it never sees the profile itself.
#[test]
fn embedder_legacy_lockfile_names_reach_the_loader_selection() {
    let config = Config { embedder: NUB, ..Config::default() };
    let selection = config.wanted_lockfile_selection();
    assert_eq!(selection.file_name, "nub.lock");
    assert_eq!(selection.legacy_file_names, ["lock.yaml"]);

    // pnpm's own profile carries none, so standalone pnpm still reads
    // exactly the one file it always did.
    assert!(Config::default().wanted_lockfile_selection().legacy_file_names.is_empty());
}

/// A command that reads the lockfile by name — `list`, `licenses`, `peers`,
/// `deploy` — asks the profile instead of spelling pnpm's file, and takes the
/// host's retired names with it. pnpm's own profile asks for exactly the one
/// file those commands always read.
#[test]
fn commands_reading_the_lockfile_by_name_read_the_profiles_files() {
    assert_eq!(
        Embedder::PNPM.lockfile_selection(),
        pnpm_lockfile::WantedLockfileSelection::default()
    );

    let lockfile = "lockfileVersion: '9.0'\n\nimporters:\n\n  .: {}\n";
    let current = tempfile::tempdir().expect("create a temp project dir");
    std::fs::write(current.path().join("nub.lock"), lockfile).expect("write the host's lockfile");
    let retired = tempfile::tempdir().expect("create a temp project dir");
    std::fs::write(retired.path().join("lock.yaml"), lockfile).expect("write a retired lockfile");

    let finds = |dir: &tempfile::TempDir, embedder: Embedder| {
        pnpm_lockfile::Lockfile::load_wanted(dir.path(), &embedder.lockfile_selection())
            .expect("load the lockfile")
            .is_some()
    };
    assert!(finds(&current, NUB));
    assert!(finds(&retired, NUB));
    assert!(!finds(&current, Embedder::PNPM), "pnpm's profile reads pnpm-lock.yaml only");
}

thread_local! {
    /// What the bin-dir provider below answers with, for the same reason
    /// [`HOST_SETTINGS`] is a thread-local: a provider is a plain function
    /// and captures nothing.
    static HOST_BIN_DIR: std::cell::Cell<Option<&'static std::path::Path>> =
        const { std::cell::Cell::new(None) };
}

fn provided_host_bin_dir() -> Option<&'static std::path::Path> {
    HOST_BIN_DIR.get()
}

/// The host is asked for the directory each time, not once when the profile
/// is built — a directory named per process does not exist yet when a `const`
/// profile is written, and a host may create it lazily. pnpm supplies no
/// provider, so it never gains an entry.
#[test]
fn the_script_bin_dir_is_asked_of_the_host_each_time() {
    assert!(Embedder::PNPM.script_bin_dir.is_none());
    assert_eq!(Embedder::PNPM.resolve_script_bin_dir(), None);
    assert_eq!(Config::default().embedder.resolve_script_bin_dir(), None);

    let embedder = Embedder { script_bin_dir: Some(provided_host_bin_dir), ..NUB };
    assert_eq!(embedder.resolve_script_bin_dir(), None, "no directory yet");

    HOST_BIN_DIR.set(Some(std::path::Path::new("/tmp/nub-node-shim-4171-9c2a")));
    assert_eq!(
        embedder.resolve_script_bin_dir(),
        Some(std::path::Path::new("/tmp/nub-node-shim-4171-9c2a")),
    );

    HOST_BIN_DIR.set(Some(std::path::Path::new("/tmp/nub-node-shim-4172-31f0")));
    assert_eq!(
        embedder.resolve_script_bin_dir(),
        Some(std::path::Path::new("/tmp/nub-node-shim-4172-31f0")),
        "the profile must not cache the first answer",
    );

    HOST_BIN_DIR.set(None);
}

/// A host's own settings file is what the maturity-gate diagnostics name, and
/// pnpm's wording is unchanged when no host overrides it.
///
/// The gate's four messages tell the user to add an entry to
/// `minimumReleaseAgeExclude`, which is only actionable if the file named is
/// one the running program actually reads. An embedder that resolves its own
/// configuration reads no `pnpm-workspace.yaml`, so naming it would send the
/// user to edit a file that changes nothing.
#[test]
fn the_settings_file_a_diagnostic_names_comes_from_the_profile() {
    assert_eq!(Embedder::PNPM.settings_file_display_name, "pnpm-workspace.yaml");
    assert_eq!(Embedder::default().settings_file_display_name, "pnpm-workspace.yaml");

    // A host that resolves its own settings names its own file, which is the
    // whole point: NUB reads no `pnpm-workspace.yaml`, so a diagnostic naming
    // one would be advice its user cannot act on.
    assert_eq!(NUB.settings_file_display_name, "nub.jsonc");
}

/// Naming a settings file and editing it are separate permissions. pnpm both
/// names and writes its own manifest; a host that resolves its own
/// configuration names its file for the user — the test above — and has the
/// engine write nothing, because the entries would never be read back and the
/// host's format is not one this engine emits.
///
/// Read through `Config`, which is where every consumer reads it and which
/// pins the default at the same time.
#[test]
fn only_pnpms_own_profile_lets_the_engine_write_the_settings_file() {
    assert!(Config::default().embedder.writes_settings_file);
    assert!(!Config { embedder: NUB, ..Config::default() }.embedder.writes_settings_file);
}
