//! The embedder profile: what a host that is not pnpm itself has to change
//! about the engine.
//!
//! pacquet is consumed as a library by hosts other than the `pnpm` binary —
//! `@pnpm/napi` exposes it to Node.js, and a native Rust host links the
//! crates directly. Most of what such a host needs to override is already a
//! plain [`Config`](crate::Config) field it can assign after
//! [`Config::current`](crate::Config::current): `user_agent`, `store_dir`,
//! `cache_dir` and the rest. This profile carries what is not reachable that
//! way: names the engine derives internally rather than reading from a field,
//! behavior decided before any `Config` exists, and where the configuration
//! itself comes from.
//!
//! The default is pnpm's own profile, so a host that never touches the field
//! behaves exactly as before.

/// The names an embedding host substitutes for pnpm's own.
///
/// Held by [`Config::embedder`](crate::Config::embedder) and read wherever
/// the engine would otherwise use a hard-coded pnpm name. [`Embedder::PNPM`]
/// is the default and reproduces pnpm's behavior exactly.
///
/// Borrowed fields are `'static`, which keeps the type `Copy` so passing it
/// around costs nothing: a host's brand is fixed at compile time, and settings
/// a host resolves at startup live for the rest of the run.
///
/// Not `PartialEq`: [`Self::allow_builds_writer`] is a function, and comparing
/// two of those compares addresses rather than behavior. Compare the field a
/// question is actually about.
#[derive(Debug, Clone, Copy)]
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

    /// Whether the root `package.json`'s `workspaces` array declares the
    /// workspace's projects, the way npm and Yarn spell it.
    ///
    /// pnpm declares them in `pnpm-workspace.yaml` and warns when it finds
    /// the manifest field instead, so this is off for pnpm. A host whose
    /// users declare a workspace the npm way turns it on: an ancestor
    /// manifest carrying a non-empty `workspaces` list (the array, or the
    /// `packages` of the object spelling) then marks the
    /// workspace root and supplies
    /// [`Config::workspace_package_patterns`](crate::Config::workspace_package_patterns),
    /// and the warning goes away. For a profile that also
    /// [reads pnpm's configuration](Self::reads_pnpm_config),
    /// `pnpm-workspace.yaml` still wins wherever both exist.
    pub workspaces_from_package_manifest: bool,

    /// Basename of the lockfile the engine reads and writes, as returned by
    /// [`Config::wanted_lockfile_name`](crate::Config::wanted_lockfile_name)
    /// when no git-branch lockfile is in play.
    pub lockfile_basename: &'static str,

    /// Basenames the engine still READS when
    /// [`Self::lockfile_basename`] is absent, most preferred first.
    ///
    /// A host that renames its lockfile has projects on disk carrying the
    /// old name, and they must keep installing across the upgrade. Read-only
    /// by design: an install writes [`Self::lockfile_basename`], so the first
    /// write after the rename is what retires the old file, and a frozen or
    /// headless install that writes nothing leaves it exactly as it found it.
    pub lockfile_legacy_basenames: &'static [&'static str],

    /// Leaf directory of the virtual store inside the modules directory —
    /// the `.pnpm` in `node_modules/.pnpm`. Applies only when the virtual
    /// store directory is derived; an explicit `virtualStoreDir` setting
    /// still wins.
    pub virtual_store_dirname: &'static str,

    /// The name this host tells the user to edit when a setting has to be
    /// changed by hand.
    ///
    /// Every diagnostic that asks for a settings edit — the
    /// `minimumReleaseAgeExclude` prompts above all — has to name a real file,
    /// and under an embedder that resolves its own configuration the file pnpm
    /// would name is one the host never reads and never writes. Naming it
    /// sends the user to edit a file that changes nothing, which is worse than
    /// saying nothing at all.
    ///
    /// Defaults to `pnpm-workspace.yaml`, so standalone pnpm's wording is
    /// unchanged.
    pub settings_file_display_name: &'static str,

    /// Whether the engine reads the configuration only pnpm defines: the
    /// `pnpm-workspace.yaml` search, the global `config.yaml` and `auth.ini`,
    /// `pnpm_config_*` / `PNPM_CONFIG_*` environment variables, and the
    /// default `.pnpmfile.cjs` / `.pnpmfile.mjs`.
    ///
    /// A host with a configuration file of its own turns this off and passes
    /// what it resolved as [`Self::workspace_settings`]. The sources pnpm
    /// shares with npm — the `.npmrc` chain and `npm_config_*` — are read
    /// either way, as are command-line options.
    pub reads_pnpm_config: bool,

    /// Settings the host resolves from its own configuration, applied where
    /// `pnpm-workspace.yaml` sits in the cascade: above the `.npmrc` chain and
    /// the global `config.yaml`, below `PNPM_CONFIG_*`. They pass through the
    /// same filtering as the workspace yaml, since a host's project
    /// configuration is exactly as repository-controlled, and relative paths
    /// in them resolve against the workspace root, or the project directory
    /// when there is no workspace. Supplying them does not make a directory a
    /// workspace.
    ///
    /// Asked again for each configuration the engine builds, not resolved
    /// once: a command that writes the host's own configuration and then
    /// reloads — `approve-builds` recording an answer and rebuilding on it —
    /// must see what it just wrote, exactly as pnpm re-reads its yaml.
    pub workspace_settings: Option<WorkspaceSettingsProvider>,

    /// Compatibility rules the host adds beneath the engine's own database of
    /// `@yarnpkg/extensions` and pnpm's additions. They repair published
    /// manifests at resolve time exactly as the built-in rules do, stay out of
    /// the lockfile's `packageExtensionsChecksum`, and are declined together
    /// with them by `ignoreCompatibilityDb`. Where a host rule and a built-in
    /// rule set the same field, the built-in rule wins.
    pub compat_package_extensions:
        Option<&'static indexmap::IndexMap<String, crate::PackageExtension>>,

    /// Where `allowBuilds` lives for this host. pnpm keeps it in its
    /// workspace manifest, writing the user's `approve-builds` answer there
    /// and scaffolding a line to edit for every build an install blocked. A
    /// host that reads no such file has to keep the answer somewhere it
    /// will read back, or the next install asks the same question again —
    /// and the manifest must not be written behind its back, since the file
    /// may mean something to the host that the engine cannot know.
    ///
    /// Supplying one moves both writes: the host is handed the directory
    /// the decision belongs to and the decided packages, each with whether
    /// its scripts may run, and merges them into whatever it already had;
    /// nothing scaffolds the engine's own manifest.
    pub allow_builds_writer: Option<AllowBuildsWriter>,

    /// An observer notified of every package this run extracts into the
    /// store, for a host that inspects package contents — pnpm registers
    /// none. Supplied as a function rather than as the observer itself so
    /// the profile stays plain data: an `Arc` on it would cost the `Copy`
    /// that makes passing it around free. Asked once per configuration.
    pub extract_observer: Option<ExtractObserverProvider>,

    /// The policy deciding which packages this host keeps out of the
    /// shared virtual store, supplied the same way and for the same
    /// reason. pnpm sets none, and then every package is shared.
    pub materialize_policy: Option<MaterializePolicyProvider>,
}

/// Records a set of approve-builds decisions for a host that keeps them
/// outside pnpm's workspace manifest. See
/// [`Embedder::allow_builds_writer`].
pub type AllowBuildsWriter = fn(&std::path::Path, &[(&str, bool)]) -> std::io::Result<()>;

/// Supplies the observer a host wants notified of each extraction. See
/// [`Embedder::extract_observer`].
pub type ExtractObserverProvider = fn() -> std::sync::Arc<dyn pnpm_store_dir::ExtractObserver>;

/// Supplies the policy deciding what a host keeps out of the shared virtual
/// store. See [`Embedder::materialize_policy`].
pub type MaterializePolicyProvider = fn() -> std::sync::Arc<dyn pnpm_store_dir::MaterializePolicy>;

/// Answers with the settings a host resolves for the directory a
/// configuration is being built for. See [`Embedder::workspace_settings`].
pub type WorkspaceSettingsProvider =
    fn(&std::path::Path) -> Option<&'static crate::WorkspaceSettings>;

impl Embedder {
    /// pnpm's own names. The default, and what standalone pnpm always uses.
    pub const PNPM: Self = Embedder {
        program_name: "pnpm",
        program_version: crate::defaults::PNPM_VERSION,
        manage_package_manager_versions: true,
        manage_runtimes: true,
        workspaces_from_package_manifest: false,
        lockfile_basename: pnpm_lockfile::Lockfile::FILE_NAME,
        lockfile_legacy_basenames: &[],
        virtual_store_dirname: ".pnpm",
        settings_file_display_name: "pnpm-workspace.yaml",
        reads_pnpm_config: true,
        workspace_settings: None,
        compat_package_extensions: None,
        allow_builds_writer: None,
        extract_observer: None,
        materialize_policy: None,
    };
}

impl Default for Embedder {
    fn default() -> Self {
        Embedder::PNPM
    }
}

#[cfg(test)]
mod tests;
