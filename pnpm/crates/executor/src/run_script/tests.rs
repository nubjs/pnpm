use super::{
    RunScript, ScriptOutput, build_command, parsed_by_windows_shell, posix_quote, run_script,
};
use crate::{extend_path::ScriptsPrependNodePath, script_exit::ScriptExit};
use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
};
use tempfile::tempdir;

#[test]
fn posix_quote_leaves_safe_strings_unquoted() {
    assert_eq!(posix_quote("hello-world"), "hello-world");
    assert_eq!(posix_quote("a_b@1.0.0/path:to,thing"), "a_b@1.0.0/path:to,thing");
}

#[test]
fn posix_quote_wraps_unsafe_strings() {
    assert_eq!(posix_quote(""), "''");
    assert_eq!(posix_quote("a b"), "'a b'");
    assert_eq!(posix_quote("two words"), "'two words'");
}

#[test]
fn posix_quote_escapes_embedded_single_quotes() {
    assert_eq!(posix_quote("it's"), r#"'it'"'"'s'"#);
}

#[test]
fn build_command_without_args_returns_script_unchanged() {
    for windows_shell in [false, true] {
        assert_eq!(build_command("tsc --build", &[], windows_shell), "tsc --build");
    }
}

#[test]
fn build_command_appends_posix_quoted_args() {
    let args = ["plain".to_string(), "needs quoting".to_string()];
    assert_eq!(build_command("echo", &args, false), "echo plain 'needs quoting'");
}

#[test]
fn build_command_appends_json_quoted_args_for_the_windows_shell() {
    let args =
        [r"C:\dir\".to_string(), String::new(), r#"a"b"#.to_string(), "line\nbreak".to_string()];
    let expected = r#"echo "C:\\dir\\" "" "a\"b" "line\nbreak""#;
    assert_eq!(build_command("echo", &args, true), expected);
}

#[test]
fn only_a_native_windows_run_is_parsed_by_the_windows_shell() {
    assert!(parsed_by_windows_shell(true, false));
    assert!(!parsed_by_windows_shell(true, true));
    assert!(!parsed_by_windows_shell(false, false));
    assert!(!parsed_by_windows_shell(false, true));
}

fn manifest() -> serde_json::Value {
    serde_json::json!({ "name": "t", "version": "1.0.0" })
}

fn run(pkg_root: &Path, stage: &str, script: &str, args: &[String]) -> ScriptExit {
    run_with_script_bin_dir(pkg_root, stage, script, args, None)
}

fn run_with_script_bin_dir(
    pkg_root: &Path,
    stage: &str,
    script: &str,
    args: &[String],
    script_bin_dir: Option<&Path>,
) -> ScriptExit {
    let extra_env = HashMap::new();
    run_script(&RunScript {
        manifest: &manifest(),
        stage,
        script,
        args,
        pkg_root,
        init_cwd: pkg_root,
        extra_bin_paths: &[],
        script_shell: None,
        shell_emulator: false,
        scripts_prepend_node_path: ScriptsPrependNodePath::Never,
        node_execpath: None,
        script_bin_dir,
        npm_execpath: None,
        user_agent: None,
        extra_env: &extra_env,
        silent: true,
        output: ScriptOutput::Inherit,
        process_tracker: None,
    })
    .expect("run the script")
}

#[test]
#[cfg_attr(target_os = "windows", ignore = "uses a POSIX shell script body")]
fn run_script_stamps_npm_lifecycle_event() {
    let dir = tempdir().expect("temp dir");
    let marker = dir.path().join("stage.txt");
    let script = format!(r#"printf %s "$npm_lifecycle_event" > "{}""#, marker.display());

    let status = run(dir.path(), "build", &script, &[]);
    assert!(status.success(), "the script should exit cleanly");
    let written = fs::read_to_string(&marker).expect("read marker");
    assert_eq!(written, "build");
}

#[test]
#[cfg_attr(target_os = "windows", ignore = "uses a POSIX shell script body")]
fn run_script_prepends_node_modules_bin_to_path() {
    let dir = tempdir().expect("temp dir");
    let marker = dir.path().join("path.txt");
    let script = format!(r#"printf %s "$PATH" > "{}""#, marker.display());

    run(dir.path(), "build", &script, &[]);
    let written = fs::read_to_string(&marker).expect("read marker");
    let expected_bin = dir.path().join("node_modules").join(".bin");
    eprintln!("PATH:\n{written}\n");
    assert!(
        written.split(':').any(|entry| Path::new(entry) == expected_bin),
        "PATH should contain the project's node_modules/.bin",
    );
}

/// The whole point of the seam: an embedding host's bin directory reaches
/// the script — resolving a bare command name ahead of anything the
/// inherited `PATH` offers — while the process the engine runs in never
/// learns about it. A host that instead prepends the directory to its own
/// `PATH` moves every cache key the engine derives from the environment,
/// which is what this exists to avoid.
#[test]
#[cfg_attr(target_os = "windows", ignore = "uses a POSIX shell script body")]
fn a_script_bin_dir_reaches_the_script_and_never_the_process() {
    let dir = tempdir().expect("temp dir");
    let shims = dir.path().join("host-shim-1234-abcd");
    fs::create_dir_all(&shims).expect("create the shim dir");

    // Two shims: one under a name nothing else answers to, proving the dir
    // is reachable at all, and one shadowing `env`, proving it outranks the
    // inherited PATH. Resolution is the evidence — a substring match on
    // `$PATH` would pass on a directory no command could actually be found in.
    for (name, body) in [("probe-shim", "#!/bin/sh\nprintf ok\n"), ("env", "#!/bin/sh\n")] {
        let shim = shims.join(name);
        fs::write(&shim, body).expect("write the shim");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&shim, fs::Permissions::from_mode(0o755)).expect("chmod the shim");
        }
    }

    let marker = dir.path().join("out.txt");
    let script = format!(
        r#"printf '%s|%s|%s' "$(probe-shim)" "$(command -v env)" "$PATH" > "{}""#,
        marker.display(),
    );

    let before = std::env::var_os("PATH");
    let status = run_with_script_bin_dir(dir.path(), "build", &script, &[], Some(&shims));
    let after = std::env::var_os("PATH");

    assert!(status.success(), "the script should exit cleanly");
    assert_eq!(before, after, "running a script must not edit the process PATH");

    let written = fs::read_to_string(&marker).expect("read marker");
    let fields: Vec<&str> = written.split('|').collect();
    assert_eq!(fields.len(), 3, "marker should hold three fields, got {written:?}");
    assert_eq!(fields[0], "ok", "the shim dir's own command should be runnable");
    assert_eq!(
        Path::new(fields[1]),
        shims.join("env"),
        "`env` should resolve to the shim, not the system one",
    );

    let entries: Vec<&Path> = fields[2].split(':').map(Path::new).collect();
    let shim_idx = entries
        .iter()
        .position(|entry| *entry == shims)
        .unwrap_or_else(|| panic!("the shim dir is missing from the script PATH: {entries:?}"));
    let inherited: Vec<PathBuf> =
        before.as_ref().map(|value| std::env::split_paths(value).collect()).unwrap_or_default();
    let first_inherited = entries
        .iter()
        .position(|entry| inherited.iter().any(|orig| orig == *entry))
        .expect("the inherited PATH should survive into the script");
    assert_eq!(
        shim_idx + 1,
        first_inherited,
        "the shim dir must sit immediately ahead of the inherited PATH: {entries:?}",
    );
}

#[test]
#[cfg_attr(target_os = "windows", ignore = "uses a POSIX shell script body")]
fn run_script_returns_the_scripts_exit_status() {
    let dir = tempdir().expect("temp dir");
    let status = run(dir.path(), "build", "exit 7", &[]);
    assert_eq!(status.code(), Some(7));
}
