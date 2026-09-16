use crate::State;
use clap::Args;
use derive_more::{Display, Error};
use indexmap::IndexMap;
use miette::{Context, Diagnostic, IntoDiagnostic};
use pnpm_config::Config;
use pnpm_package_manager::{Install, ProjectMutation};
use pnpm_package_manifest::{DependencyGroup, PackageManifest};
use pnpm_reporter::Reporter;
use pnpm_workspace_manifest_writer::set_overrides;
use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

/// Links a local package as a dependency.
#[derive(Debug, Args)]
pub struct LinkArgs {
    pub package_paths: Vec<String>,
}

#[derive(Debug, Display, Error, Diagnostic)]
#[non_exhaustive]
pub enum LinkError {
    #[display("You must provide a parameter. Usage: pnpm link <dir>")]
    #[diagnostic(code(ERR_PNPM_LINK_BAD_PARAMS))]
    NoParams,

    #[display(r#"Cannot link by package name. Use a relative or absolute path instead, e.g. "pnpm link ./{name}""#)]
    #[diagnostic(code(ERR_PNPM_LINK_BAD_PARAMS))]
    LinkByName {
        #[error(not(source))]
        name: String,
    },

    /// A link is only a link because an override points the dependency at the
    /// local directory, and that override has to survive the command. Under a
    /// host whose overrides come from its own configuration there is nowhere
    /// to record it that the next install would read back, so this refuses
    /// before touching `package.json` rather than leaving a dependency
    /// resolving to the registry copy.
    #[display(
        "Linked dependencies cannot be recorded for you, because {settings_file} is not this program's to write."
    )]
    #[diagnostic(
        code(ERR_PNPM_LINK_OVERRIDES_NOT_WRITABLE),
        help("Add these to overrides in {settings_file} by hand, then install:\n  {specifiers}")
    )]
    OverridesNotWritable { settings_file: &'static str, specifiers: String },
}

const DEPENDENCY_FIELDS: [&str; 3] = ["optionalDependencies", "dependencies", "devDependencies"];

fn is_filespec(input: &str) -> bool {
    let mut chars = input.chars();
    match chars.next() {
        Some('.' | '/') => true,
        Some('\\') if cfg!(windows) => true,
        Some('~') => chars.next() == Some('/'),
        Some(c) if c.is_ascii_alphabetic() => chars.next() == Some(':'),
        _ => false,
    }
}

fn link_spec(base: &Path, target: &Path) -> String {
    let rel = pathdiff::diff_paths(target, base).unwrap_or_else(|| target.to_path_buf());
    format!("link:{}", rel.display().to_string().replace('\\', "/"))
}

fn already_declared(manifest: &PackageManifest, name: &str) -> bool {
    DEPENDENCY_FIELDS.iter().any(|field| {
        manifest
            .value()
            .get(field)
            .and_then(serde_json::Value::as_object)
            .is_some_and(|deps| deps.contains_key(name))
    })
}

impl LinkArgs {
    pub async fn run<Reporter: self::Reporter + 'static>(
        self,
        config: &'static mut Config,
        manifest_path: PathBuf,
    ) -> miette::Result<()> {
        if self.package_paths.is_empty() {
            return Err(LinkError::NoParams.into());
        }

        if let Some(name) = self.package_paths.iter().find(|path| !is_filespec(path)) {
            return Err(LinkError::LinkByName { name: name.clone() }.into());
        }

        let manifest_dir = manifest_path
            .parent()
            .ok_or_else(|| miette::miette!("manifest path has no parent directory"))?
            .to_path_buf();

        let mut manifest = PackageManifest::create_if_needed(manifest_path.clone())
            .wrap_err("reading the project package.json")?;

        let root_dir = config.workspace_dir.clone().unwrap_or_else(|| manifest_dir.clone());

        let mut new_overrides = IndexMap::<String, String>::new();
        for path_str in &self.package_paths {
            let (target_dir, package_name) = link_target(&manifest_dir, path_str)?;

            if !already_declared(&manifest, &package_name) {
                manifest
                    .add_dependency(
                        &package_name,
                        &link_spec(&manifest_dir, &target_dir),
                        DependencyGroup::Prod,
                    )
                    .wrap_err("adding linked dependency to package.json")?;
            }
            new_overrides.insert(package_name, link_spec(&root_dir, &target_dir));
        }

        // Refusing has to happen while everything above this point is still an
        // in-memory edit — that is what leaves the project exactly as the
        // command found it. Saving first would add a dependency whose override
        // never lands, which resolves to the registry copy, the opposite of
        // what was asked for.
        if !overrides_recordable(&config.embedder) {
            return Err(LinkError::OverridesNotWritable {
                settings_file: config.embedder.settings_file_display_name,
                specifiers: new_overrides
                    .iter()
                    .map(|(selector, specifier)| format!("{selector}: {specifier}"))
                    .collect::<Vec<_>>()
                    .join("\n  "),
            }
            .into());
        }

        manifest.save().wrap_err("saving package.json with linked dependencies")?;

        record_link_overrides(config, &root_dir, &new_overrides)?;

        let state = State::init(manifest_path, config, false).wrap_err("initialize the state")?;
        install_linked::<Reporter>(&state).await
    }
}

/// Whether the overrides a link implies can be recorded somewhere the next
/// install will read them back.
///
/// A link is only a link because an override points the dependency at the
/// local directory, so a host with nowhere to keep that override cannot link
/// at all. See [`LinkError::OverridesNotWritable`].
fn overrides_recordable(embedder: &pnpm_config::Embedder) -> bool {
    embedder.overrides_writer.is_some() || embedder.writes_settings_file
}

/// Record `new_overrides` where this host reads them back, and mirror them
/// into the run's own configuration so the install that follows resolves
/// against what was just written.
///
/// Same seam `approve-builds` goes through: a host that supplies its own
/// writer is handed the decision, and the workspace manifest is left alone.
fn record_link_overrides(
    config: &mut Config,
    root_dir: &Path,
    new_overrides: &IndexMap<String, String>,
) -> miette::Result<()> {
    match config.embedder.overrides_writer {
        Some(write) => write(
            root_dir,
            &new_overrides
                .iter()
                .map(|(selector, specifier)| (selector.as_str(), Some(specifier.as_str())))
                .collect::<Vec<_>>(),
        )
        .into_diagnostic()
        .wrap_err("recording linked dependencies for the host")?,
        None => set_overrides(
            root_dir,
            new_overrides
                .iter()
                .map(|(selector, specifier)| (selector.as_str(), specifier.as_str())),
        )
        .wrap_err("recording linked dependencies in pnpm-workspace.yaml")?,
    }
    config.overrides.get_or_insert_with(IndexMap::new).extend(
        new_overrides.iter().map(|(selector, specifier)| (selector.clone(), specifier.clone())),
    );
    Ok(())
}

/// Install with the linked dependencies' overrides in place.
async fn install_linked<Reporter: self::Reporter + 'static>(state: &State) -> miette::Result<()> {
    let lockfile_path = state.lockfile_path();
    Install {
        lockfile_path: Some(&lockfile_path),
        prefer_frozen_lockfile: Some(false),
        mutation: ProjectMutation::NoInstall,
        installs_only: false,
        ..Install::new(
            Arc::clone(&state.tarball_mem_cache),
            &state.resolved_packages,
            (&state.http_client, Arc::clone(&state.http_client)),
            state.config,
            &state.manifest,
            pnpm_lockfile::MaybeLazyLockfile::Lazy(&state.lockfile),
            [DependencyGroup::Prod, DependencyGroup::Dev, DependencyGroup::Optional].into_iter(),
        )
    }
    .run::<Reporter>()
    .await
    .wrap_err("linking dependencies")
}

/// The linked package's directory, and the name it is declared under.
fn link_target(manifest_dir: &Path, path_str: &str) -> miette::Result<(PathBuf, String)> {
    let target_path = PathBuf::from(path_str);
    let target_dir =
        if target_path.is_absolute() { target_path } else { manifest_dir.join(&target_path) };
    let target_manifest_path = target_dir.join("package.json");
    let dir_display = target_dir.display();
    let target_manifest = PackageManifest::from_path(target_manifest_path)
        .map_err(|_| miette::miette!("No package.json found in {}", dir_display))?;
    let package_name = target_manifest.value()["name"]
        .as_str()
        .ok_or_else(|| miette::miette!("Target package does not have a name field"))?
        .to_string();
    Ok((target_dir, package_name))
}

#[cfg(test)]
mod tests {
    use super::{overrides_recordable, record_link_overrides};
    use indexmap::IndexMap;
    use pnpm_config::{Config, Embedder};

    fn one_override() -> IndexMap<String, String> {
        let mut overrides = IndexMap::new();
        overrides.insert("sib".to_string(), "link:../sib".to_string());
        overrides
    }

    fn host_that_writes_no_settings_file() -> Embedder {
        Embedder {
            writes_settings_file: false,
            settings_file_display_name: "nub.jsonc",
            ..Embedder::PNPM
        }
    }

    /// pnpm's own profile keeps the override in its workspace manifest, so a
    /// link is recordable with no host writer at all. The control for the two
    /// below: without it, a predicate stuck at `true` would look correct.
    #[test]
    fn pnpms_own_profile_records_through_the_workspace_manifest() {
        assert!(overrides_recordable(&Embedder::PNPM));
    }

    /// A host that resolves its own configuration has nowhere to keep the
    /// override, so the link refuses rather than leave the dependency
    /// resolving to the registry copy.
    #[test]
    fn a_host_with_neither_a_writer_nor_a_settings_file_cannot_record() {
        assert!(!overrides_recordable(&host_that_writes_no_settings_file()));
    }

    /// Supplying a writer is what gives such a host somewhere to keep it, and
    /// the engine then writes no workspace manifest behind its back.
    #[test]
    fn a_host_writer_takes_the_overrides_instead_of_the_workspace_manifest() {
        fn record(dir: &std::path::Path, entries: &[(&str, Option<&str>)]) -> std::io::Result<()> {
            // `None` is the removal spelling `unlink` uses. Rendering it
            // distinctly rather than skipping it keeps this writer honest if a
            // future caller sends one through the link path.
            let body: Vec<String> = entries
                .iter()
                .map(|(selector, spec)| match spec {
                    Some(spec) => format!("{selector}={spec}"),
                    None => format!("{selector}=<removed>"),
                })
                .collect();
            std::fs::write(dir.join("host-overrides"), body.join("\n"))
        }

        let dir = tempfile::tempdir().expect("a temp dir");
        let embedder =
            Embedder { overrides_writer: Some(record), ..host_that_writes_no_settings_file() };
        assert!(overrides_recordable(&embedder));

        let mut config = Config { embedder, ..Config::default() };
        record_link_overrides(&mut config, dir.path(), &one_override())
            .expect("the host writer runs");

        assert_eq!(
            std::fs::read_to_string(dir.path().join("host-overrides"))
                .expect("the host's own file"),
            "sib=link:../sib"
        );
        assert!(
            !dir.path().join("pnpm-workspace.yaml").exists(),
            "nothing reached the workspace manifest this host never reads"
        );
        assert_eq!(
            config.overrides.as_ref().and_then(|o| o.get("sib")).map(String::as_str),
            Some("link:../sib"),
            "the run's own config carries the override the install resolves against"
        );
    }
}
