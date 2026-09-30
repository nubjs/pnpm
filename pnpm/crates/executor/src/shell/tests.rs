use super::{ScriptShellError, SelectedShell, select_shell};
use pretty_assertions::assert_eq;
use std::{collections::HashMap, ffi::OsString, path::Path};

fn os(text: &str) -> OsString {
    OsString::from(text)
}

#[test]
fn posix_default_is_sh_minus_c() {
    let shell = select_shell(None, false).expect("select_shell");
    assert_eq!(
        shell,
        SelectedShell {
            program: Path::new("sh").to_path_buf(),
            args: vec![os("-c")],
            windows_verbatim_args: false,
            restore_env_casing: false,
        },
    );
}

/// We do not assert the program path here because the runner env may
/// or may not have `ComSpec` set — the args + verbatim flag are the
/// load-bearing part.
#[test]
fn windows_default_uses_cmd_with_d_s_c_and_verbatim_args() {
    let shell = select_shell(None, true).expect("select_shell");
    assert_eq!(shell.args, vec![os("/d"), os("/s"), os("/c")]);
    assert!(shell.windows_verbatim_args, "verbatim must be set for cmd.exe");
    let program = shell.program.to_string_lossy().to_ascii_lowercase();
    assert!(
        program == "cmd" || program.ends_with("cmd.exe"),
        "expected cmd or *cmd.exe, got {program:?}",
    );
}

#[test]
fn custom_script_shell_wins_on_both_platforms() {
    let custom = Path::new("/usr/local/bin/bash");
    for is_windows in [false, true] {
        let shell = select_shell(Some(custom), is_windows).expect("select_shell");
        assert_eq!(
            shell,
            SelectedShell {
                program: custom.to_path_buf(),
                args: vec![os("-c")],
                windows_verbatim_args: false,
                restore_env_casing: false,
            },
            "is_windows={is_windows}",
        );
    }
}

#[test]
fn batch_file_script_shell_rejected_on_windows() {
    for ext in [".cmd", ".CMD", ".bat", ".BAT"] {
        let path = format!(r"C:\tools\shell-mock{ext}");
        let err = select_shell(Some(Path::new(&path)), true).expect_err("must reject");
        match err {
            ScriptShellError::BatchFileOnWindows { path: got } => {
                assert_eq!(got, path, "error path must echo input");
            }
        }
    }
}

/// Upstream's `isWindowsBatchFile` gates on `process.platform === 'win32'`,
/// so a Linux user pointing scriptShell at `something.cmd` is left
/// alone (it'd fail elsewhere, but that's not this guard's job).
#[test]
fn batch_file_script_shell_allowed_on_posix() {
    let path = Path::new("/tmp/weird.cmd");
    let shell = select_shell(Some(path), false).expect("must accept on POSIX");
    assert_eq!(shell.program, path);
}

#[test]
fn busybox_script_shell_runs_its_sh_applet_and_restores_env_casing() {
    for path in [r"C:\nub\bin\busybox.exe", r"C:\tools\BusyBox64.exe", "/opt/busybox"] {
        let shell = select_shell(Some(Path::new(path)), true).expect("select_shell");
        assert_eq!(shell.program, Path::new(path));
        assert_eq!(
            shell.args,
            vec![os("sh"), os("-c")],
            "{path}: busybox dispatches on argv[0] or a leading applet name, so `busybox -c` runs no shell",
        );
        assert!(shell.restore_env_casing, "{path}");
        assert!(!shell.windows_verbatim_args, "{path}");
    }
}

#[test]
fn a_casing_restoring_shell_re_exports_the_lowercase_names_it_was_handed() {
    let env: HashMap<String, String> = [
        ("npm_package_name", "a"),
        ("PATH", "x"),
        ("npm_lifecycle_event", "postinstall"),
        ("9lives", "no"),
        ("with-dash", "no"),
        ("Mixed", "yes"),
    ]
    .into_iter()
    .map(|(name, value)| (name.to_string(), value.to_string()))
    .collect();
    let busybox = select_shell(Some(Path::new(r"C:\nub\busybox.exe")), true).expect("select_shell");
    assert_eq!(
        busybox.script_body("node build.js", &env),
        "export Mixed=\"$MIXED\"; export npm_lifecycle_event=\"$NPM_LIFECYCLE_EVENT\"; \
         export npm_package_name=\"$NPM_PACKAGE_NAME\"; node build.js",
    );
    let plain = select_shell(Some(Path::new("/bin/bash")), false).expect("select_shell");
    assert_eq!(plain.script_body("node build.js", &env), "node build.js");
}
