use super::discovered_workspace_dir;
use pnpm_config::{Config, Embedder};
use std::fs;

/// A profile that reads pnpm's configuration finds the workspace the yaml
/// declares, and one that does not finds nothing — so an ancestor
/// `pnpm-workspace.yaml` cannot capture a project whose host never reads it.
/// Without the second case the host's project takes the ancestor as its
/// workspace root, which anchors its lockfile and importer ids there.
#[test]
fn only_a_profile_that_reads_pnpm_config_discovers_the_workspace_yaml() {
    let tmp = tempfile::tempdir().unwrap();
    let root = dunce::canonicalize(tmp.path()).unwrap();
    let project = root.join("app");
    fs::create_dir(&project).unwrap();
    fs::write(root.join("pnpm-workspace.yaml"), "packages:\n  - app\n").unwrap();

    let pnpm = Config::default();
    assert_eq!(discovered_workspace_dir(&pnpm, &project).unwrap(), Some(root));

    let host = Config { embedder: Embedder { reads_pnpm_config: false, ..Embedder::PNPM }, ..pnpm };
    assert_eq!(discovered_workspace_dir(&host, &project).unwrap(), None);
}
