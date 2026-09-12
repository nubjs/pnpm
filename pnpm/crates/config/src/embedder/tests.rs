use crate::{Config, embedder::Embedder};
use pretty_assertions::assert_eq;
use std::path::PathBuf;

/// A host that is not pnpm, with both names replaced.
const NUB: Embedder = Embedder { lockfile_basename: "nub.lock", virtual_store_dirname: ".store" };

#[test]
fn default_profile_keeps_pnpm_naming() {
    let config = Config::default();
    assert_eq!(config.embedder, Embedder::PNPM);
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
