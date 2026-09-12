//! Where the workspace root comes from when the config carries none.

use pnpm_config::Config;
use std::path::{Path, PathBuf};

/// Discover the workspace root by walking up for `pnpm-workspace.yaml`.
///
/// A host that reads none of pnpm's configuration has already resolved its
/// own workspace root into [`Config::workspace_dir`], from whatever file it
/// does read. Walking for the yaml here would let a file the host never
/// reads override that — and an ancestor `pnpm-workspace.yaml` would then
/// capture a project belonging to the host, anchoring its lockfile and
/// importer ids at a root it never chose.
pub(crate) fn discovered_workspace_dir(
    config: &Config,
    start_dir: &Path,
) -> Result<Option<PathBuf>, pnpm_workspace::FindWorkspaceDirError> {
    if !config.embedder.reads_pnpm_config {
        return Ok(None);
    }
    pnpm_workspace::find_workspace_dir(start_dir)
}

#[cfg(test)]
mod tests;
