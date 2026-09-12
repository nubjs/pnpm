use crate::{Config, embedder::Embedder};
use pretty_assertions::assert_eq;
use std::path::PathBuf;

/// A host that is not pnpm: its own naming, and version management left to
/// the host rather than the engine.
const NUB: Embedder = Embedder {
    program_name: "nub",
    program_version: "0.0.0-test",
    manage_package_manager_versions: false,
    manage_runtimes: false,
    workspaces_from_package_manifest: true,
    lockfile_basename: "nub.lock",
    virtual_store_dirname: ".store",
};

#[test]
fn default_profile_keeps_pnpm_naming() {
    let config = Config::default();
    assert_eq!(config.embedder, Embedder::PNPM);
    assert_eq!(config.embedder.program_name, "pnpm");
    assert!(config.embedder.manage_package_manager_versions);
    assert!(config.embedder.manage_runtimes);
    assert!(!config.embedder.workspaces_from_package_manifest);
    assert_eq!(config.wanted_lockfile_name(), "pnpm-lock.yaml");
    assert_eq!(config.embedder.virtual_store_dirname, ".pnpm");
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

    assert_eq!(config.embedder, NUB);
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

/// `pnpm-workspace.yaml` is the authoritative declaration wherever it
/// exists, so a project carrying both files installs pnpm's projects, not
/// the manifest's. The manifest patterns here would select a different set,
/// which is what makes the assertion discriminate.
#[test]
fn the_workspace_yaml_still_wins_over_the_package_manifest() {
    let dir = tempfile::tempdir().expect("create a temp project dir");
    write_manifest_workspace(dir.path());
    std::fs::write(dir.path().join("pnpm-workspace.yaml"), "packages:\n  - libs/*\n")
        .expect("write the workspace manifest");

    let config = Config { embedder: NUB, ..Config::default() }
        .current::<crate::Host>(dir.path())
        .expect("load config");

    assert_eq!(config.workspace_dir.as_deref(), Some(dir.path()));
    assert_eq!(config.workspace_package_patterns, Some(vec!["libs/*".to_string()]));
}
