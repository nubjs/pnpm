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
    /// version management for its own users turns this off, which also
    /// stops `install` and `add` from announcing a newer pnpm.
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

    /// The name this host's users know the build allow-list by, for a
    /// message that tells them to edit it.
    ///
    /// pnpm calls it `allowBuilds`. A host that keeps the approvals somewhere
    /// else — see [`Self::allow_builds_writer`] — reads them back under a name
    /// of its own, and a hint naming pnpm's field sends the user to a setting
    /// that changes nothing.
    ///
    /// Defaults to `allowBuilds`, so standalone pnpm's wording is unchanged.
    pub allow_builds_display_name: &'static str,

    /// The file this host records overrides in, spelled as help text
    /// names it, for the flag that writes one.
    ///
    /// pnpm keeps overrides in its workspace manifest, so `audit --fix`
    /// says it adds them there. A host that records them through
    /// [`Self::overrides_writer`] puts them somewhere else, and
    /// substituting [`Self::settings_file_display_name`] does not reach
    /// that: a host whose settings live in one file can record overrides
    /// in another — the neutral `overrides` of `package.json`, say —
    /// which leaves the flag describing a file it never writes.
    ///
    /// `None` while the host records them where pnpm does, which keeps
    /// the sentence pnpm wrote true.
    pub overrides_file_display_name: Option<&'static str>,

    /// Hidden entries this host writes into a modules directory beside
    /// pnpm's own, which `clean` removes along with them.
    ///
    /// `clean` leaves every other dot-entry in `node_modules` alone — a
    /// tool's `.cache` is not the package manager's to delete — so a file the
    /// host keeps there, such as a stamp recording what built the tree, would
    /// outlive the tree it describes. The engine cannot know such a name, so
    /// the host lists it.
    ///
    /// Empty for pnpm.
    pub hidden_modules_dir_entries: &'static [&'static str],

    /// Whether the engine reads the configuration only pnpm defines: the
    /// `pnpm-workspace.yaml` search, the global `config.yaml` and `auth.ini`,
    /// `pnpm_config_*` / `PNPM_CONFIG_*` environment variables, and the
    /// default `.pnpmfile.cjs` / `.pnpmfile.mjs`.
    ///
    /// A host with a configuration file of its own turns this off and passes
    /// what it resolved as [`Self::workspace_settings`]. The `.npmrc` chain is
    /// read either way, as are command-line options; which `npm_config_*`
    /// variables apply is [`Self::reads_npm_config_env`]'s to decide.
    pub reads_pnpm_config: bool,

    /// Whether `npm_config_*` / `NPM_CONFIG_*` environment variables set the
    /// keys an `.npmrc` routes, proxies and secures requests with: `registry`,
    /// `@scope:registry`, the proxy keys, and `ca`, `cafile`, `cert`, `key`,
    /// `strict-ssl` and `local-address`.
    ///
    /// npm reads them above every `.npmrc` and pnpm reads none of them, so
    /// this is off for pnpm. A host whose users configure requests the npm
    /// way, such as a CI job exporting `npm_config_registry` or
    /// `NPM_CONFIG_STRICT_SSL`, turns it on, and the variables then rank above
    /// the project `.npmrc`. Credentials are not among them: the URL-scoped
    /// credential variables are read under either profile.
    pub reads_npm_config_env: bool,

    /// Whether the engine may EDIT pnpm's settings file — the write-side twin
    /// of [`Self::reads_pnpm_config`].
    ///
    /// Several paths record a decision by merging it into
    /// `pnpm-workspace.yaml`: the `minimumReleaseAgeExclude` entries an
    /// approved install persists, the catalog entries `add`/`update` resolve,
    /// the `allowBuilds` lines an install scaffolds for the builds it blocked,
    /// and the `configDependencies` `add --config` records. A host that
    /// resolves its own configuration reads none of them back, and the file
    /// may mean something to the host that the engine cannot know, so turning
    /// this off stops every one of those writes.
    ///
    /// It does not follow that the write can simply be dropped. Where the
    /// persisted entry is what lets the run proceed — the exclude list above
    /// all — the path refuses instead and names the entries for the user to
    /// add to [`Self::settings_file_display_name`] by hand; skipping it
    /// silently would report a change that never happened and leave the next
    /// install stopped at the same gate. The advisory writes are simply not
    /// made.
    ///
    /// [`Self::settings_file_display_name`] answers a different question —
    /// where the user should look — and is never a write target: it names the
    /// host's own file, in the host's own format, which this crate's
    /// format-preserving YAML writer cannot produce.
    pub writes_settings_file: bool,

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

    /// Where an override `link` records lives for this host, the write-side
    /// twin of the same problem `allow_builds_writer` solves. A link is only
    /// a link because an override points the dependency at the local
    /// directory, and pnpm keeps that override in its workspace manifest. A
    /// host that reads no such file has nowhere the next install would read
    /// the override back from, so `link` refuses rather than leave the
    /// dependency resolving to the registry copy.
    ///
    /// Supplying one gives the host that place: it is handed the directory
    /// the overrides belong to and each selector with the specifier to
    /// record, and merges them into whatever it already had. Hosts whose
    /// overrides come from a file they DO read — `package.json`'s neutral
    /// `overrides`, say — want this rather than the refusal.
    ///
    /// The same writer REMOVES, with `None` for the specifier, and `unlink`
    /// goes through it. That symmetry is load-bearing rather than tidiness:
    /// while removal was gated on the workspace manifest alone, a host with a
    /// writer could create a link it was then refused permission to undo, and
    /// the refusal named a settings file that host keeps no overrides in, and
    /// may not even have.
    pub overrides_writer: Option<OverridesWriter>,

    /// Where `patchedDependencies` lives for this host, the problem
    /// [`Self::overrides_writer`] solves for `link`. A patch applies only
    /// because `patchedDependencies` names its file, and pnpm keeps that map
    /// in its workspace manifest. A host that reads no such file has nowhere
    /// the next install would find the entry, so `patch-commit` and
    /// `patch-remove` refuse before touching the project rather than leave a
    /// patch file nothing applies, or an entry naming a file that is gone.
    ///
    /// Supplying one gives the host that place. It is handed the directory
    /// the entries belong to and each selector with the patch file to record,
    /// relative to that directory, or `None` to drop the selector, and merges
    /// them into whatever it already had. The engine still writes and deletes
    /// the patch files themselves exactly as pnpm does, and leaves the
    /// workspace manifest alone.
    ///
    /// Both commands install straight afterwards on a configuration built
    /// anew, so [`Self::workspace_settings`] has to answer with the edit by
    /// the time the writer returns.
    pub patched_dependencies_writer: Option<PatchedDependenciesWriter>,

    /// The Node.js executable this host runs scripts under.
    ///
    /// The engine records it as `NODE` and `npm_node_execpath` for every
    /// lifecycle and package script it spawns, and `scriptsPrependNodePath`
    /// puts its directory on the script's `PATH`. pnpm supplies none and
    /// records the first `node` on `PATH` instead. A host that fronts `PATH`
    /// with a directory of its own, such as a shim that re-enters the host,
    /// would otherwise have that shim recorded where a script expects a
    /// Node.js installation.
    pub node_execpath: Option<&'static std::path::Path>,

    /// The executable this host answers `dlx --package <spec> <command>`
    /// with, the way pnpm does.
    ///
    /// A git-hosted dependency that pins a package manager, such as
    /// `packageManager: yarn@1.22.22`, is prepared with that version: the
    /// engine puts shims on the build's `PATH` that run
    /// `<executable> dlx --package yarn@1.22.22 yarn`. pnpm supplies none and
    /// forwards to itself when the running executable is `pnpm`. Under a host
    /// the running executable is the host's, so without this the dependency
    /// is prepared with whatever package manager the machine has installed,
    /// at a version it did not ask for.
    pub pnpm_execpath: Option<&'static std::path::Path>,

    /// A directory of this host's own executables, put on the `PATH` of
    /// every script the engine spawns — and nowhere else. pnpm supplies
    /// none.
    ///
    /// A host that fronts scripts with shims of its own has one obvious
    /// place to put them: the process `PATH`, before the engine is ever
    /// called. That works, and it poisons every key the engine derives from
    /// the environment. The build pipeline's Cargo cache hashes `PATH` among
    /// its inputs, and a host whose shim directory is named per process —
    /// carrying a pid, or a nonce against collisions — moves that key on
    /// every run, so the cache is written and never restored. The same
    /// directory in `extraBinPaths` moves the task run-state fingerprint for
    /// the same reason, and `extraEnv` is hashed into the Cargo key directly.
    ///
    /// Supplying it here keeps it out of all three: the engine reads the
    /// process environment as it found it, and adds the directory only when
    /// it builds a script's `PATH`. It lands immediately ahead of the
    /// inherited `PATH`, which is exactly where a host-prepended entry would
    /// have sat, so a script resolves commands in the same order as before —
    /// the project's `node_modules/.bin`, then `extraBinPaths`, then
    /// `scriptsPrependNodePath`'s Node directory, then these shims, then
    /// whatever the user's `PATH` already offered.
    ///
    /// A function rather than a path because a profile is a `const`: a
    /// directory named per process cannot be spelled in one. It is asked
    /// each time a script's `PATH` is built, so a host that creates the
    /// directory lazily may answer `None` until it exists.
    pub script_bin_dir: Option<ScriptBinDirProvider>,

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

    /// Whether a `dlx` child that exits nonzero ends this process the same
    /// way.
    ///
    /// pnpm's own `dlx` is the last thing the process does, so becoming the
    /// child's exit status (and, on Unix, re-raising its fatal signal) is the
    /// only faithful way to report it. A host that embeds the engine is not
    /// finished when the child is: the call has to return so the host can run
    /// its own epilogue. With this `false`, a failed child surfaces as
    /// `ERR_PNPM_DLX_CHILD_FAILED` carrying the code instead of exiting, which
    /// also keeps it distinguishable from a failure to FETCH the tool at all.
    pub dlx_exits_like_child: bool,
}

/// Records a set of approve-builds decisions for a host that keeps them
/// outside pnpm's workspace manifest. See
/// [`Embedder::allow_builds_writer`].
pub type AllowBuildsWriter = fn(&std::path::Path, &[(&str, bool)]) -> std::io::Result<()>;

/// Records a set of overrides for a host that keeps them outside pnpm's
/// workspace manifest: each selector with the specifier to record, or `None`
/// to drop it. See [`Embedder::overrides_writer`].
pub type OverridesWriter = fn(&std::path::Path, &[(&str, Option<&str>)]) -> std::io::Result<()>;

/// Records an edit to `patchedDependencies` for a host that keeps them
/// outside pnpm's workspace manifest: each selector with the patch file to
/// record, or `None` to drop it. See [`Embedder::patched_dependencies_writer`].
pub type PatchedDependenciesWriter =
    fn(&std::path::Path, &[(&str, Option<&str>)]) -> std::io::Result<()>;

/// Answers with the directory of host executables to put on a spawned
/// script's `PATH`. See [`Embedder::script_bin_dir`].
pub type ScriptBinDirProvider = fn() -> Option<&'static std::path::Path>;

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
        allow_builds_display_name: "allowBuilds",
        overrides_file_display_name: None,
        hidden_modules_dir_entries: &[],
        reads_pnpm_config: true,
        reads_npm_config_env: false,
        writes_settings_file: true,
        workspace_settings: None,
        compat_package_extensions: None,
        allow_builds_writer: None,
        overrides_writer: None,
        patched_dependencies_writer: None,
        node_execpath: None,
        pnpm_execpath: None,

        script_bin_dir: None,
        extract_observer: None,
        materialize_policy: None,
        dlx_exits_like_child: true,
    };

    /// This run's [`script_bin_dir`](Self::script_bin_dir), asked of the
    /// host. Call it where a script's `PATH` is built, not once at startup:
    /// a host may create the directory lazily.
    #[must_use]
    pub fn resolve_script_bin_dir(&self) -> Option<&'static std::path::Path> {
        self.script_bin_dir.and_then(|ask| ask())
    }

    /// The wanted lockfile a command reads by name: [`Self::lockfile_basename`],
    /// then [`Self::lockfile_legacy_basenames`].
    ///
    /// For the commands that inspect a project's resolution rather than
    /// install it — `list`, `why`, `licenses`, `peers`, `deploy` and the rest.
    /// pnpm reads `pnpm-lock.yaml` for those whatever the per-branch lockfile
    /// settings say, so the settings stay out of this selection; an install
    /// reads [`Config::wanted_lockfile_selection`](crate::Config::wanted_lockfile_selection).
    /// Under pnpm's own profile this is exactly the one file those commands
    /// always read.
    #[must_use]
    pub fn lockfile_selection(&self) -> pnpm_lockfile::WantedLockfileSelection {
        pnpm_lockfile::WantedLockfileSelection {
            file_name: self.lockfile_basename.to_owned(),
            merge_git_branch_lockfiles: false,
            legacy_file_names: self.lockfile_legacy_basenames,
        }
    }
}

impl Default for Embedder {
    fn default() -> Self {
        Embedder::PNPM
    }
}

#[cfg(test)]
mod tests;
