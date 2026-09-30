use derive_more::{Display, Error};
use miette::Diagnostic;
use std::{
    borrow::Cow,
    collections::HashMap,
    env,
    ffi::OsString,
    path::{Path, PathBuf},
};

/// Failure to pick a shell for a lifecycle hook.
#[derive(Debug, Display, Error, Diagnostic)]
#[non_exhaustive]
pub enum ScriptShellError {
    /// Setting `scriptShell` to a `.bat` or `.cmd` file on Windows is
    /// blocked because Node refuses to spawn batch files without
    /// `shell: true`, and re-escaping arguments for a shell-wrapped
    /// batch invocation is unsafe (cf. CVE-2024-27980 / CVE-2024-24576).
    #[display(
        "Cannot spawn .bat or .cmd as a script shell. \
         The pnpm-workspace.yaml scriptShell option was configured to a .bat or .cmd file. \
         These cannot be used as a script shell reliably. \
         Please unset the scriptShell option, or configure it to a .exe instead. \
         (scriptShell={path})"
    )]
    #[diagnostic(code(ERR_PNPM_INVALID_SCRIPT_SHELL_WINDOWS))]
    BatchFileOnWindows { path: String },
}

/// The result of [`select_shell`]: a program path plus the leading
/// flag arguments (`-c`, `/d /s /c`, etc.) that go before the script
/// body when spawning.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectedShell {
    pub program: PathBuf,
    pub args: Vec<OsString>,
    /// Whether Node's `windowsVerbatimArguments` flag would have
    /// fired for this combination. Used by the Windows caller to opt
    /// into Rust's `std::os::windows::process::CommandExt::raw_arg`
    /// (Windows-only API; not linked as an intra-doc reference
    /// because rustdoc on non-Windows targets cannot resolve it).
    ///
    /// On non-Windows platforms the field is set but ignored —
    /// keeping the struct platform-independent simplifies tests.
    pub windows_verbatim_args: bool,
    /// Whether the script body is prefixed with [`lowercase_env_prologue`]:
    /// busybox-w32 up-cases every environment name it loads, so a body
    /// sees `NPM_PACKAGE_NAME` and `$npm_package_name` expands to nothing.
    pub restore_env_casing: bool,
}

/// Pick the shell to spawn a lifecycle script under.
///
/// `is_windows` lets tests drive both branches without `#[cfg(windows)]`
/// gating the test bodies. Production callers pass `cfg!(windows)`.
pub fn select_shell(
    script_shell: Option<&Path>,
    is_windows: bool,
) -> Result<SelectedShell, ScriptShellError> {
    if is_windows
        && let Some(p) = script_shell
        && is_windows_batch_file(p)
    {
        return Err(ScriptShellError::BatchFileOnWindows {
            path: p.to_string_lossy().into_owned(),
        });
    }

    if let Some(p) = script_shell {
        // busybox is a multi-call binary that dispatches on `argv[0]` or on a
        // leading applet name, so spawned under its own name it needs `sh`
        // before `-c`: `busybox -c <body>` runs no shell at all.
        let busybox = is_busybox(p);
        let mut args = Vec::new();
        if busybox {
            args.push(OsString::from("sh"));
        }
        args.push(OsString::from("-c"));
        return Ok(SelectedShell {
            program: p.to_path_buf(),
            args,
            windows_verbatim_args: false,
            restore_env_casing: busybox,
        });
    }

    if is_windows {
        let comspec = env::var_os("ComSpec")
            .or_else(|| env::var_os("COMSPEC"))
            .map_or_else(|| PathBuf::from("cmd"), PathBuf::from);
        return Ok(SelectedShell {
            program: comspec,
            args: vec![OsString::from("/d"), OsString::from("/s"), OsString::from("/c")],
            windows_verbatim_args: true,
            restore_env_casing: false,
        });
    }

    Ok(SelectedShell {
        program: PathBuf::from("sh"),
        args: vec![OsString::from("-c")],
        windows_verbatim_args: false,
        restore_env_casing: false,
    })
}

/// `.cmd` / `.bat` suffix check, case-insensitive on the suffix. The
/// upstream `isWindowsBatchFile` also gates on `process.platform === 'win32'`;
/// here we factor that out and let the caller pass `is_windows`.
fn is_windows_batch_file(path: &Path) -> bool {
    let lowered = path.to_string_lossy().to_ascii_lowercase();
    lowered.ends_with(".cmd") || lowered.ends_with(".bat")
}

/// Whether `path` names a busybox binary — `busybox.exe`, or a release
/// build such as `busybox64.exe` — by its file stem.
fn is_busybox(path: &Path) -> bool {
    // Split on both separators: a Windows path is selected on a Unix host too
    // (`is_windows` is a parameter, not `cfg!`), where `Path` would treat
    // `C:\\x\\busybox.exe` as one component.
    let name = path.to_string_lossy();
    let name = name.rsplit(['/', '\\']).next().unwrap_or("");
    let stem = name.split_once('.').map_or(name, |(stem, _)| stem);
    stem.to_ascii_lowercase().starts_with("busybox")
}

impl SelectedShell {
    /// The body to hand this shell for `script`, given the environment the
    /// child is spawned with: `script` itself, or under a casing-restoring
    /// shell the [`lowercase_env_prologue`] for `env` followed by `script`.
    #[must_use]
    pub fn script_body<'s>(&self, script: &'s str, env: &HashMap<String, String>) -> Cow<'s, str> {
        if self.restore_env_casing {
            let prologue = lowercase_env_prologue(env.keys().map(String::as_str));
            Cow::Owned(format!("{prologue}{script}"))
        } else {
            Cow::Borrowed(script)
        }
    }
}

/// An `export` prologue re-binding, at the head of a script body, every
/// lowercase name among `names`.
///
/// busybox-w32's shell up-cases every name when it loads the Windows
/// environment, so a body sees `NPM_PACKAGE_NAME` and `$npm_package_name`
/// expands to nothing, while npm, pnpm, yarn and bun all deliver the
/// lowercase name. Upstream considers the up-casing correct and declined a
/// preserve-casing patch (rmyorston/busybox-w32#125), so the spawner
/// restores it. busybox's own lookup is case-sensitive, so one prologue
/// makes `$npm_package_name` expand, puts the lowercase name back in the
/// environment every child inherits, and shows it to `Object.keys`.
///
/// Names are re-bound, never values, so nothing is quoted and a value
/// carrying quotes or newlines cannot break the body. A name with no
/// lowercase letter is unaffected by the up-casing, and one that is not a
/// shell identifier cannot be exported, so both are skipped.
#[must_use]
pub fn lowercase_env_prologue<'a>(names: impl IntoIterator<Item = &'a str>) -> String {
    let mut names: Vec<&str> = names
        .into_iter()
        .filter(|name| {
            name.contains(|c: char| c.is_ascii_lowercase())
                && !name.starts_with(|c: char| c.is_ascii_digit())
                && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
        })
        .collect();
    names.sort_unstable();
    names.dedup();
    let mut prologue = String::new();
    for name in names {
        prologue.push_str("export ");
        prologue.push_str(name);
        prologue.push_str("=\"$");
        prologue.push_str(&name.to_ascii_uppercase());
        prologue.push_str("\"; ");
    }
    prologue
}

#[cfg(test)]
mod tests;
