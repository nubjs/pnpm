use super::LoadedState;
use pnpm_config::Embedder;
use std::fs;

const LOCKFILE: &str = "lockfileVersion: '9.0'\n\nimporters:\n\n  .: {}\n";

/// `list` and `why` read two files an install wrote: the wanted lockfile, and
/// the current one inside the virtual store. A host names both. pnpm's profile
/// pointed at the same directory finds neither, which is what shows the
/// profile decides rather than the directory.
#[test]
fn reads_both_lockfiles_under_the_profiles_names() {
    let dir = tempfile::tempdir().expect("temp dir");
    fs::write(dir.path().join("host.lock"), LOCKFILE).expect("write the wanted lockfile");
    let virtual_store = dir.path().join("node_modules").join(".host-store");
    fs::create_dir_all(&virtual_store).expect("create the virtual store");
    fs::write(virtual_store.join("lock.yaml"), LOCKFILE).expect("write the current lockfile");
    let host = Embedder {
        lockfile_basename: "host.lock",
        virtual_store_dirname: ".host-store",
        ..Embedder::PNPM
    };

    let state = LoadedState::load(dir.path(), None, false, host).expect("load the host's state");
    assert!(state.wanted_lockfile.is_some(), "the wanted lockfile is read");
    assert!(state.current_lockfile.is_some(), "the current lockfile is read");

    let pnpm = LoadedState::load(dir.path(), None, false, Embedder::PNPM).expect("load");
    assert!(pnpm.wanted_lockfile.is_none(), "pnpm's profile reads pnpm-lock.yaml only");
    assert!(pnpm.current_lockfile.is_none(), "pnpm's profile reads node_modules/.pnpm only");
}
