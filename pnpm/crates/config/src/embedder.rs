//! The embedder profile: the brand-bearing names a host that is not pnpm
//! itself has to replace.
//!
//! pacquet is consumed as a library by hosts other than the `pnpm` binary —
//! `@pnpm/napi` exposes it to Node.js, and a native Rust host links the
//! crates directly. Most of what such a host needs to override is already a
//! plain [`Config`](crate::Config) field it can assign after
//! [`Config::current`](crate::Config::current): `user_agent`, `store_dir`,
//! `cache_dir` and the rest. Two names are not reachable that way, because
//! the engine derives them internally rather than reading them from a field:
//! the wanted lockfile's basename, and the leaf directory of the virtual
//! store. This profile carries those.
//!
//! The default is pnpm's own naming, so a host that never touches the field
//! behaves exactly as before.

/// The names an embedding host substitutes for pnpm's own.
///
/// Held by [`Config::embedder`](crate::Config::embedder) and read wherever
/// the engine would otherwise use a hard-coded pnpm name. [`Embedder::PNPM`]
/// is the default and reproduces pnpm's behavior exactly.
///
/// The fields are `&'static str` because a host's brand is fixed at compile
/// time; that also keeps the type `Copy` so passing it around costs nothing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Embedder {
    /// Name the program is invoked by. Shown in command-line help and in
    /// the reporter's completion footer.
    pub program_name: &'static str,

    /// Version rendered beside [`Self::program_name`] in the reporter's
    /// completion footer.
    pub program_version: &'static str,

    /// Whether a `packageManager` / `devEngines.packageManager` pin is
    /// acted on: resolved, downloaded, and delegated to, and reported as an
    /// error when it names a different package manager. A host that owns
    /// version management for its own users turns this off.
    pub manage_package_manager_versions: bool,

    /// Whether `devEngines.runtime` / `engines.runtime` entries are checked
    /// against the installed runtime.
    pub manage_runtimes: bool,

    /// Basename of the lockfile the engine reads and writes, as returned by
    /// [`Config::wanted_lockfile_name`](crate::Config::wanted_lockfile_name)
    /// when no git-branch lockfile is in play.
    pub lockfile_basename: &'static str,

    /// Leaf directory of the virtual store inside the modules directory —
    /// the `.pnpm` in `node_modules/.pnpm`. Applies only when the virtual
    /// store directory is derived; an explicit `virtualStoreDir` setting
    /// still wins.
    pub virtual_store_dirname: &'static str,
}

impl Embedder {
    /// pnpm's own names. The default, and what standalone pnpm always uses.
    pub const PNPM: Self = Embedder {
        program_name: "pnpm",
        program_version: crate::defaults::PNPM_VERSION,
        manage_package_manager_versions: true,
        manage_runtimes: true,
        lockfile_basename: pnpm_lockfile::Lockfile::FILE_NAME,
        virtual_store_dirname: ".pnpm",
    };
}

impl Default for Embedder {
    fn default() -> Self {
        Embedder::PNPM
    }
}

#[cfg(test)]
mod tests;
