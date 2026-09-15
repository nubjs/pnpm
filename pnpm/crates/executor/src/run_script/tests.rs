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

#[cfg(unix)]
const SEP: char = ':';
#[cfg(windows)]
const SEP: char = ';';

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
        script_bin_dir: None,
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

/// The whole point of the seam: the host's bin directory is in the `PATH`
/// the script is spawned with, ahead of everything the process inherited,
/// while the process the engine runs in never learns about it. A host that
/// instead prepends the directory to its own `PATH` moves every cache key
/// the engine derives from the environment, which is what this avoids.
///
/// Asserts on [`child_env`] — the map handed to `Command::envs` — rather
/// than on a spawned shell, because [`crate::ProcessTracker::cancel`] kills
/// every descendant of this process, so a script running while
/// `process_tracker::tests` cancels is killed with SIGKILL under any `--test-threads`
/// above one.
#[test]
fn a_script_bin_dir_reaches_the_script_and_never_the_process() {
    let dir = tempdir().expect("temp dir");
    let shims = dir.path().join("host-shim-1234-abcd");

    let before = std::env::var_os("PATH").expect("the test process has a PATH");
    let built = script_env(dir.path(), Some(&shims));
    assert_eq!(
        std::env::var_os("PATH").as_ref(),
        Some(&before),
        "building a script environment must not edit the process PATH",
    );

    let path = built.get("PATH").expect("the script environment carries a PATH");
    let entries: Vec<&Path> = path.split(SEP).map(Path::new).collect();
    let shim_idx = entries
        .iter()
        .position(|entry| *entry == shims)
        .unwrap_or_else(|| panic!("the shim dir is missing from the script PATH: {entries:?}"));
    let inherited: Vec<PathBuf> = std::env::split_paths(&before).collect();
    let first_inherited = entries
        .iter()
        .position(|entry| inherited.iter().any(|original| original == *entry))
        .expect("the inherited PATH should survive into the script");
    assert_eq!(
        shim_idx + 1,
        first_inherited,
        "the shim dir must sit immediately ahead of the inherited PATH: {entries:?}",
    );

    // The control: without a host directory the script PATH holds no entry
    // outside the project's own `.bin` and what the process inherited.
    let plain = script_env(dir.path(), None);
    let plain_path = plain.get("PATH").expect("the script environment carries a PATH");
    assert!(
        !plain_path.split(SEP).any(|entry| Path::new(entry) == shims),
        "pnpm's own profile must add nothing: {plain_path:?}",
    );
}

/// The environment [`run_script`] would spawn a script with.
fn script_env(pkg_root: &Path, script_bin_dir: Option<&Path>) -> HashMap<String, String> {
    let extra_env = HashMap::new();
    super::child_env(
        &RunScript {
            manifest: &manifest(),
            stage: "build",
            script: "true",
            args: &[],
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
        },
        "true",
    )
}

#[test]
#[cfg_attr(target_os = "windows", ignore = "uses a POSIX shell script body")]
fn run_script_returns_the_scripts_exit_status() {
    let dir = tempdir().expect("temp dir");
    let status = run(dir.path(), "build", "exit 7", &[]);
    assert_eq!(status.code(), Some(7));
}
