use super::PeersArgs;
use pnpm_config::{Config, Embedder};

/// `--lockfile-only` reads the wanted lockfile by name, so under a host that
/// renamed it the check has to read the host's file: reading pnpm's instead
/// finds nothing, and the check reports a clean tree whatever the resolution
/// holds.
#[test]
fn lockfile_only_checks_the_profiles_lockfile() {
    let dir = tempfile::tempdir().expect("temp dir");
    std::fs::write(
        dir.path().join("host.lock"),
        "lockfileVersion: '9.0'\n\nimporters:\n\n  .: {}\n",
    )
    .expect("write the lockfile");
    let args = PeersArgs { json: false, lockfile_only: true, params: Vec::new() };

    let mut config = Config::default();
    assert!(
        args.load_lockfile(&config, dir.path()).expect("load").is_none(),
        "pnpm's profile reads pnpm-lock.yaml only",
    );
    config.embedder = Embedder { lockfile_basename: "host.lock", ..Embedder::PNPM };
    assert!(args.load_lockfile(&config, dir.path()).expect("load").is_some());
}
