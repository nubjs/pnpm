use clap::Args;
use miette::{Context, IntoDiagnostic};
use pnpm_config::Config;
use pnpm_workspace_manifest_writer::remove_overrides;
use std::path::Path;

/// Removing a `link:` override is the whole of `unlink`, so under a host whose
/// overrides come from its own configuration there is nothing this command can
/// do: dropping them from memory alone would report success and then re-link on
/// the next install.
#[derive(Debug, derive_more::Display, derive_more::Error, miette::Diagnostic)]
#[display(
    "Linked dependencies cannot be removed for you, because {settings_file} is not this program's to write."
)]
#[diagnostic(
    code(ERR_PNPM_UNLINK_OVERRIDES_NOT_WRITABLE),
    help("Remove these from overrides in {settings_file} by hand, then install:\n  {selectors}")
)]
pub struct UnlinkOverridesNotWritable {
    settings_file: &'static str,
    selectors: String,
}

/// Remove the link created by `pnpm link` and reinstall the package as
/// declared in `package.json`.
///
/// With package names, only the matching links are removed; with no
/// arguments, every link is removed.
#[derive(Debug, Args)]
pub struct UnlinkArgs {
    pub package_names: Vec<String>,

    /// Disable pnpm hooks defined in `.pnpmfile.cjs`, including the
    /// pnpmfiles of config dependencies.
    #[clap(long = "ignore-pnpmfile")]
    pub ignore_pnpmfile: bool,
}

impl UnlinkArgs {
    pub(crate) fn apply_cli_config(&self, config: &mut Config) {
        config.ignore_pnpmfile = self.ignore_pnpmfile || config.ignore_pnpmfile;
    }

    /// Strip the matching `link:` overrides from `config` (in memory) and
    /// from `pnpm-workspace.yaml`, returning whether the caller should
    /// reinstall.
    ///
    /// Mirrors pnpm: when no overrides are configured it prints "Nothing to
    /// unlink" and returns `false` so the caller stops; otherwise it removes
    /// the `link:` overrides — the ones named, or all of them — and returns
    /// `true` so the caller reinstalls, even when nothing matched.
    pub(crate) fn strip_link_overrides(
        &self,
        config: &mut Config,
        manifest_path: &Path,
    ) -> miette::Result<bool> {
        // Read off the profile before `overrides` takes a mutable borrow of
        // `config`; the profile is `Copy`, so this costs nothing.
        let embedder = config.embedder;
        let Some(overrides) = config.overrides.as_mut() else {
            println!("Nothing to unlink");
            return Ok(false);
        };

        let removed: Vec<String> = overrides
            .iter()
            .filter(|(selector, specifier)| {
                specifier.starts_with("link:")
                    && (self.package_names.is_empty()
                        || self.package_names.iter().any(|name| name == *selector))
            })
            .map(|(selector, _)| selector.clone())
            .collect();

        // Before the in-memory removal, so a refusal leaves the run's own
        // config consistent with what is on disk.
        // The predicate is `link`'s, deliberately. A host that can RECORD an
        // override can remove one through the same writer, and gating removal
        // on the workspace manifest alone left a link such a host could create
        // and never undo -- with a refusal naming a settings file it keeps no
        // overrides in, and may not even have.
        if !removed.is_empty()
            && embedder.overrides_writer.is_none()
            && !embedder.writes_settings_file
        {
            return Err(UnlinkOverridesNotWritable {
                settings_file: embedder.settings_file_display_name,
                selectors: removed.join("\n  "),
            }
            .into());
        }

        for selector in &removed {
            overrides.shift_remove(selector);
        }

        if !removed.is_empty() {
            let root_dir = config
                .workspace_dir
                .clone()
                .or_else(|| manifest_path.parent().map(Path::to_path_buf))
                .ok_or_else(|| miette::miette!("manifest path has no parent directory"))?;

            match embedder.overrides_writer {
                Some(write) => {
                    let removals: Vec<(&str, Option<&str>)> =
                        removed.iter().map(|selector| (selector.as_str(), None)).collect();
                    write(&root_dir, &removals)
                        .into_diagnostic()
                        .wrap_err("removing linked dependencies for the host")?;
                }
                None => remove_overrides(&root_dir, &removed)
                    .wrap_err("removing link: overrides from pnpm-workspace.yaml")?,
            }
        }

        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::{UnlinkArgs, UnlinkOverridesNotWritable};
    use indexmap::IndexMap;
    use pnpm_config::Config;
    use std::sync::Mutex;

    /// What [`recording_writer`] was handed. An `OverridesWriter` is a plain
    /// function pointer, so a closure cannot capture the assertion target.
    static RECORDED: Mutex<Vec<(String, Option<String>)>> = Mutex::new(Vec::new());

    fn recording_writer(
        _dir: &std::path::Path,
        entries: &[(&str, Option<&str>)],
    ) -> std::io::Result<()> {
        RECORDED.lock().expect("record the override edit").extend(
            entries.iter().map(|(selector, specifier)| {
                ((*selector).to_owned(), specifier.map(str::to_owned))
            }),
        );
        Ok(())
    }

    fn config_with_link_override() -> Config {
        let mut overrides = IndexMap::new();
        overrides.insert("sib".to_string(), "link:../sib".to_string());
        Config { overrides: Some(overrides), ..Config::default() }
    }

    /// pnpm's own profile writes the manifest, so the override is stripped
    /// from memory and from disk as before. This is the control for the test
    /// below: without it, a refusal that fired unconditionally would look
    /// like a pass.
    #[test]
    fn a_host_that_writes_the_settings_file_strips_the_override() {
        let dir = tempfile::tempdir().expect("temp dir");
        let mut config = config_with_link_override();
        config.workspace_dir = Some(dir.path().to_path_buf());

        let reinstall = UnlinkArgs { package_names: Vec::new(), ignore_pnpmfile: false }
            .strip_link_overrides(&mut config, &dir.path().join("package.json"))
            .expect("unlink applies under pnpm's own profile");

        assert!(reinstall, "the caller reinstalls after a successful unlink");
        assert!(
            config.overrides.as_ref().is_none_or(|o| !o.contains_key("sib")),
            "the link: override is gone from the run's own config"
        );
    }

    /// Under a host whose overrides come from its own configuration the
    /// removal cannot be persisted, so the command refuses instead of
    /// reporting a success the next install would undo.
    #[test]
    fn a_host_that_writes_no_settings_file_refuses_rather_than_report_a_removal() {
        let dir = tempfile::tempdir().expect("temp dir");
        let mut config = config_with_link_override();
        config.workspace_dir = Some(dir.path().to_path_buf());
        config.embedder.writes_settings_file = false;
        config.embedder.settings_file_display_name = "nub.jsonc";

        let err = UnlinkArgs { package_names: Vec::new(), ignore_pnpmfile: false }
            .strip_link_overrides(&mut config, &dir.path().join("package.json"))
            .expect_err("a host that writes no settings file cannot unlink");

        assert!(
            err.downcast_ref::<UnlinkOverridesNotWritable>().is_some(),
            "refused for the right reason, got: {err:?}"
        );
        assert!(
            config.overrides.as_ref().is_some_and(|o| o.contains_key("sib")),
            "the run's own config still matches what is on disk"
        );
        assert!(
            err.to_string().contains("nub.jsonc"),
            "the refusal names the host's settings file, got: {err}"
        );
    }

    /// A host that can RECORD an override can remove one through the same
    /// writer, so it must not be refused.
    ///
    /// The gate above used to read the workspace manifest alone, which made
    /// the pair asymmetric: `link` accepts a host with a writer, `unlink`
    /// refused it. Measured on a built nub before the fix -- `link` exited 0
    /// and wrote the override to `package.json`, then `unlink` exited 1 and
    /// told the user to edit `nub.jsonc`, which the fixture did not even have.
    /// The link was unremovable by any nub command.
    #[test]
    fn a_host_with_an_overrides_writer_removes_through_it() {
        RECORDED.lock().expect("clear the record").clear();
        let dir = tempfile::tempdir().expect("temp dir");
        let mut config = config_with_link_override();
        config.workspace_dir = Some(dir.path().to_path_buf());
        config.embedder.writes_settings_file = false;
        config.embedder.settings_file_display_name = "nub.jsonc";
        config.embedder.overrides_writer = Some(recording_writer);

        let reinstall = UnlinkArgs { package_names: Vec::new(), ignore_pnpmfile: false }
            .strip_link_overrides(&mut config, &dir.path().join("package.json"))
            .expect("a host with a writer can persist the removal");

        assert!(reinstall, "the caller reinstalls after a successful unlink");
        assert_eq!(
            *RECORDED.lock().expect("read the record"),
            vec![("sib".to_owned(), None)],
            "the writer is handed the selector with None, which is the removal spelling",
        );
        assert!(
            config.overrides.as_ref().is_none_or(|o| !o.contains_key("sib")),
            "and the run's own config no longer carries the link",
        );
    }
}
