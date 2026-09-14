use super::{
    PatchCommitArgs, PatchCommitError, cleanup_after_diff, normalize_patches_dir_name,
    patch_target_from_state, path_from_forward_slash, remove_dir_if_exists,
    write_patch_file_atomically,
};
use crate::{State, cli_args::patch_state::EditDirState};
use pnpm_config::Embedder;
use pnpm_lockfile::{ComVer, Lockfile, LockfileVersion, PackageKey, PackageMetadata};
use pnpm_package_manager::PkgFilesForDiff;
use pnpm_reporter::SilentReporter;
use pretty_assertions::assert_eq;
use serde_json::json;
use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
};
use tempfile::tempdir;

fn empty_lockfile() -> Lockfile {
    Lockfile {
        lockfile_version: LockfileVersion::<9>::try_from(ComVer { major: 9, minor: 0 }).unwrap(),
        settings: None,
        catalogs: None,
        overrides: None,
        package_extensions_checksum: None,
        pnpmfile_checksum: None,
        ignored_optional_dependencies: None,
        patched_dependencies: None,
        importers: HashMap::new(),
        packages: None,
        snapshots: None,
        time: None,
        extra: pnpm_lockfile::LockfileExtra::default(),
    }
}

fn lockfile_with_packages(keys: &[&str]) -> Lockfile {
    let packages =
        keys.iter().map(|key| (key.parse::<PackageKey>().unwrap(), registry_metadata())).collect();
    Lockfile { packages: Some(packages), ..empty_lockfile() }
}

fn lockfile_with_package_entries(entries: Vec<(PackageKey, PackageMetadata)>) -> Lockfile {
    Lockfile { packages: Some(entries.into_iter().collect()), ..empty_lockfile() }
}

fn registry_metadata() -> PackageMetadata {
    serde_json::from_value(json!({
        "resolution": {
            "integrity": "sha512-aGVsbG8=",
        },
    }))
    .unwrap()
}

fn git_tarball_metadata(version: &str, tarball: &str) -> PackageMetadata {
    serde_json::from_value(json!({
        "resolution": {
            "tarball": tarball,
            "gitHosted": true,
        },
        "version": version,
    }))
    .unwrap()
}

#[test]
fn patch_target_from_state_falls_back_to_manifest_name_and_version() {
    let lockfile = lockfile_with_packages(&["foo@1.0.0"]);
    let state = EditDirState {
        patched_pkg: "old-name@9.9.9".to_string(),
        apply_to_all: true,
        package_key: None,
    };

    let target =
        patch_target_from_state(&state, "foo", "1.0.0", &lockfile).expect("target from fallback");

    assert_eq!(target.alias, "foo");
    assert_eq!(target.version, "1.0.0");
    assert_eq!(target.bare_specifier, "1.0.0");
    assert!(target.apply_to_all);
}

#[test]
fn patch_target_from_state_reports_missing_manifest_version() {
    let lockfile = lockfile_with_packages(&["foo@1.0.0"]);
    let state = EditDirState {
        patched_pkg: "old-name@9.9.9".to_string(),
        apply_to_all: false,
        package_key: None,
    };

    let err = patch_target_from_state(&state, "foo", "2.0.0", &lockfile)
        .expect_err("missing fallback version");

    assert!(err.to_string().contains("foo@2.0.0"), "error names fallback target: {err}");
}

#[test]
fn patch_target_from_state_reports_installed_name_version_mismatch() {
    let lockfile = lockfile_with_packages(&["foo@1.0.0"]);
    let state =
        EditDirState { patched_pkg: "foo".to_string(), apply_to_all: false, package_key: None };

    let err = patch_target_from_state(&state, "foo", "2.0.0", &lockfile)
        .expect_err("missing selected version");

    assert!(err.to_string().contains("did you forget to install foo@2.0.0"), "{err}");
}

#[test]
fn patch_target_from_state_uses_persisted_package_key_for_same_version_candidates() {
    let git_tarball_url = "https://codeload.github.com/foo/foo/tar.gz/0123456789abcdef";
    let git_package_key = format!("foo@{git_tarball_url}").parse::<PackageKey>().unwrap();
    let lockfile = lockfile_with_package_entries(vec![
        ("foo@1.0.0".parse::<PackageKey>().unwrap(), registry_metadata()),
        (git_package_key.clone(), git_tarball_metadata("1.0.0", git_tarball_url)),
    ]);
    let state = EditDirState {
        patched_pkg: "foo".to_string(),
        apply_to_all: false,
        package_key: Some(git_package_key.clone()),
    };

    let target =
        patch_target_from_state(&state, "foo", "1.0.0", &lockfile).expect("target from state");

    assert_eq!(target.package_key, git_package_key);
    assert_eq!(target.git_tarball_url, Some(git_tarball_url.to_string()));
}

#[test]
fn normalize_patches_dir_name_removes_dot_and_parent_components() {
    assert_eq!(normalize_patches_dir_name("./patches"), "patches");
    assert_eq!(normalize_patches_dir_name("patches/nested/../final"), "patches/final");
    assert_eq!(normalize_patches_dir_name("../outside"), "outside");
    assert_eq!(normalize_patches_dir_name("."), ".");
}

#[cfg(unix)]
#[test]
fn normalize_patches_dir_name_ignores_root_component() {
    assert_eq!(normalize_patches_dir_name("/tmp/patches"), "tmp/patches");
}

#[test]
fn remove_dir_if_exists_removes_existing_dir() {
    let tmp = tempdir().expect("temp dir");
    let dir = tmp.path().join("patch-commit-temp");
    std::fs::create_dir(&dir).expect("create temp dir");

    remove_dir_if_exists(&dir).expect("remove temp dir");

    assert!(!dir.exists(), "temp dir should be removed");
}

#[test]
fn remove_dir_if_exists_reports_non_directory_cleanup_error() {
    let tmp = tempdir().expect("temp dir");
    let path = tmp.path().join("patch-commit-temp");
    std::fs::write(&path, "").expect("create file");

    let err = remove_dir_if_exists(&path).expect_err("file cleanup should fail");

    assert!(path.is_file(), "cleanup errors are ignored but not fatal");
    assert_eq!(err.kind(), std::io::ErrorKind::NotADirectory);
}

#[test]
fn patch_commit_atomic_writer_replaces_existing_patch_file() {
    let tmp = tempdir().expect("temp dir");
    let patch_file = tmp.path().join("pkg.patch");
    std::fs::write(&patch_file, "old").expect("write old patch");

    write_patch_file_atomically(&patch_file, b"new").expect("replace patch");

    assert_eq!(std::fs::read_to_string(patch_file).expect("read patch"), "new");
}

#[test]
fn cleanup_after_diff_removes_temporary_filtered_dir() {
    let tmp = tempdir().expect("temp dir");
    let clean_dir = tmp.path().join("clean");
    let filtered_dir = tmp.path().join("filtered");
    std::fs::create_dir(&clean_dir).expect("create clean dir");
    std::fs::create_dir(&filtered_dir).expect("create filtered dir");

    cleanup_after_diff(&clean_dir, &PkgFilesForDiff::Temporary(filtered_dir.clone()))
        .expect("cleanup temp dirs");

    assert!(!clean_dir.exists(), "clean dir should be removed");
    assert!(!filtered_dir.exists(), "filtered dir should be removed");
}

#[test]
fn cleanup_after_diff_keeps_original_patch_dir() {
    let tmp = tempdir().expect("temp dir");
    let clean_dir = tmp.path().join("clean");
    let patch_dir = tmp.path().join("patch");
    std::fs::create_dir(&clean_dir).expect("create clean dir");
    std::fs::create_dir(&patch_dir).expect("create patch dir");

    cleanup_after_diff(&clean_dir, &PkgFilesForDiff::Original(patch_dir.clone()))
        .expect("cleanup clean dir");

    assert!(!clean_dir.exists(), "clean dir should be removed");
    assert!(patch_dir.exists(), "original patch dir should remain");
}

#[test]
fn path_from_forward_slash_builds_platform_path() {
    assert_eq!(
        path_from_forward_slash("patches/@scope__pkg.patch"),
        PathBuf::from("patches").join("@scope__pkg.patch"),
    );
}

/// The profile of a host that resolves its own configuration: it names its
/// own settings file and has the engine write none.
fn host_that_writes_no_settings_file() -> Embedder {
    Embedder {
        writes_settings_file: false,
        settings_file_display_name: "host.jsonc",
        ..Embedder::PNPM
    }
}

/// Records each edit in a file of the host's own, one `selector=patch` line
/// per entry, `-` standing for a dropped one.
fn record_in_host_file(dir: &Path, entries: &[(&str, Option<&str>)]) -> std::io::Result<()> {
    let lines: Vec<String> = entries
        .iter()
        .map(|(selector, patch_file)| format!("{selector}={}", patch_file.unwrap_or("-")))
        .collect();
    fs::write(dir.join("host-patched-dependencies"), lines.join("\n"))
}

fn state_in(dir: &Path, embedder: Embedder) -> State {
    fs::write(dir.join("package.json"), "{}").expect("write package.json");
    let mut config = pnpm_config::Config::new();
    config.embedder = embedder;
    config.workspace_dir = Some(dir.to_path_buf());
    let config: &'static pnpm_config::Config = Box::leak(Box::new(config));
    State {
        tarball_mem_cache: std::sync::Arc::new(pnpm_tarball::MemCache::default()),
        http_client: std::sync::Arc::new(pnpm_network::ThrottledClient::default()),
        config,
        manifest: pnpm_package_manifest::PackageManifest::from_path(dir.join("package.json"))
            .expect("package manifest"),
        lockfile: pnpm_lockfile::LazyLockfile::disabled(),
        resolved_packages: pnpm_package_manager::ResolvedPackages::new(),
    }
}

/// With nowhere to record `patchedDependencies`, the command stops before it
/// reads the edited package, fetches the original or writes anything: this
/// edit directory was never made by `patch`, and the refusal still comes
/// first. The help points at the edits the user still has, never at a patch
/// file that was not written.
#[tokio::test]
async fn a_host_with_nowhere_to_record_the_patch_is_refused_before_anything_happens() {
    let tmp = tempdir().expect("temp dir");
    let edit_dir = tmp.path().join("edit-dir");
    let state = state_in(tmp.path(), host_that_writes_no_settings_file());

    let err = PatchCommitArgs { patch_dir: edit_dir.clone(), patches_dir: None }
        .run::<SilentReporter>(tmp.path(), state)
        .await
        .expect_err("a host with nowhere to record the patch is refused");

    assert!(
        matches!(
            &err,
            PatchCommitError::PatchedDependenciesNotWritable { settings_file: "host.jsonc", .. }
        ),
        "{err:?}"
    );
    let help = miette::Diagnostic::help(&err).map(|help| help.to_string()).unwrap_or_default();
    assert!(help.contains(&edit_dir.display().to_string()), "{help}");
    assert!(!help.contains(".patch"), "the help names no patch file: {help}");
    assert!(!tmp.path().join("patches").exists());
}

/// A host that supplies a writer gets the patch recorded through it: the patch
/// file lands where pnpm writes it, the host is handed the entry relative to
/// the directory it belongs to, and no workspace manifest appears.
#[test]
fn a_host_writer_records_the_patch_instead_of_the_workspace_manifest() {
    let tmp = tempdir().expect("temp dir");
    let embedder = Embedder {
        patched_dependencies_writer: Some(record_in_host_file),
        ..host_that_writes_no_settings_file()
    };
    let state = state_in(tmp.path(), embedder);

    PatchCommitArgs { patch_dir: tmp.path().join("edit-dir"), patches_dir: None }
        .record_patch(&state, tmp.path(), "ms", "2.1.3", false, "patch body\n")
        .expect("the host writer records the patch");

    assert_eq!(
        fs::read_to_string(tmp.path().join("patches").join("ms@2.1.3.patch"))
            .expect("the patch file"),
        "patch body\n"
    );
    assert_eq!(
        fs::read_to_string(tmp.path().join("host-patched-dependencies"))
            .expect("the host's own file"),
        "ms@2.1.3=patches/ms@2.1.3.patch"
    );
    assert!(
        !tmp.path().join("pnpm-workspace.yaml").exists(),
        "nothing reached the workspace manifest this host never reads"
    );
}
