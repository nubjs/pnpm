use super::{remove_modules_dir_contents, remove_workspace_lockfiles};
use pnpm_config::Embedder;
use std::fs;

/// `--lockfile` removes the lockfile the running program writes and any it
/// still reads under a retired name, and nothing else: a file the profile does
/// not name stays. pnpm's own removal is pinned by the `clean` suite.
#[test]
fn removes_the_profiles_lockfile_and_its_retired_names() {
    let dir = tempfile::tempdir().expect("temp dir");
    for name in ["host.lock", "retired.lock", "pnpm-lock.yaml"] {
        fs::write(dir.path().join(name), "lockfileVersion: '9.0'\n").expect("write a lockfile");
    }
    let host = Embedder {
        lockfile_basename: "host.lock",
        lockfile_legacy_basenames: &["retired.lock"],
        ..Embedder::PNPM
    };

    remove_workspace_lockfiles(dir.path(), dir.path(), host).expect("remove the lockfiles");

    assert!(!dir.path().join("host.lock").exists(), "the host's lockfile is removed");
    assert!(!dir.path().join("retired.lock").exists(), "the retired lockfile is removed");
    assert!(dir.path().join("pnpm-lock.yaml").exists(), "an unnamed lockfile is left alone");
}

/// `clean` empties a modules directory of what the running program put there:
/// the virtual store under the profile's name, and the hidden entries the host
/// lists. pnpm's profile, over the same directory, leaves both — a dotfile the
/// profile does not name is not the package manager's to delete. The `clean`
/// suite pins pnpm's own `.pnpm` removal.
#[test]
fn removes_the_profiles_virtual_store_and_the_hosts_hidden_entries() {
    let host = Embedder {
        virtual_store_dirname: ".host-store",
        hidden_modules_dir_entries: &[".host-stamp"],
        ..Embedder::PNPM
    };
    for (embedder, removes_host_entries) in [(host, true), (Embedder::PNPM, false)] {
        let dir = tempfile::tempdir().expect("temp dir");
        let modules_dir = dir.path().join("node_modules");
        fs::create_dir_all(modules_dir.join("is-odd")).expect("seed a package");
        fs::create_dir_all(modules_dir.join(".host-store")).expect("seed the virtual store");
        fs::write(modules_dir.join(".host-stamp"), "").expect("seed the host's stamp");
        fs::create_dir_all(modules_dir.join(".cache")).expect("seed an unrelated dotfile");

        remove_modules_dir_contents(&modules_dir, &embedder).expect("clean the modules dir");

        assert!(!modules_dir.join("is-odd").exists(), "a package is removed under either profile");
        assert_eq!(!modules_dir.join(".host-store").exists(), removes_host_entries, ".host-store");
        assert_eq!(!modules_dir.join(".host-stamp").exists(), removes_host_entries, ".host-stamp");
        assert!(modules_dir.join(".cache").exists(), "an unnamed dotfile stays");
    }
}
