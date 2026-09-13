use super::{command_name, working_dir};
use std::ffi::OsString;

fn name_of(argv: &[&str]) -> Option<String> {
    command_name(&argv.iter().map(OsString::from).collect::<Vec<_>>())
}

/// The first bare token names the command in the ordinary case, and an
/// alias answers with the name it stands for so a host can match one
/// spelling per command.
#[test]
fn a_command_is_named_by_any_of_its_spellings() {
    assert_eq!(name_of(&["pnpm", "install"]).as_deref(), Some("install"));
    assert_eq!(name_of(&["pnpm", "i"]).as_deref(), Some("install"));
    assert_eq!(name_of(&["pnpm", "rm", "left-pad"]).as_deref(), Some("remove"));
    assert_eq!(name_of(&["pnpm", "dist-tags"]).as_deref(), Some("dist-tag"));
}

/// The reason a host cannot scan for the command itself: an option before
/// it claims the token after it, so the first bare token is the option's
/// value rather than the command. `--store-dir` is a global option,
/// `--virtual-store-dir` a setting flag, and the two are recognized by
/// different tables.
#[test]
fn an_option_before_the_command_does_not_hide_it() {
    assert_eq!(name_of(&["pnpm", "--store-dir", "/s", "install"]).as_deref(), Some("install"));
    assert_eq!(name_of(&["pnpm", "--dir", "/p", "add", "left-pad"]).as_deref(), Some("add"));
    assert_eq!(
        name_of(&["pnpm", "--virtual-store-dir", "/v", "install"]).as_deref(),
        Some("install")
    );
    assert_eq!(name_of(&["pnpm", "--filter", "app", "rebuild"]).as_deref(), Some("rebuild"));
}

/// `pnpm recursive run build` runs `run`, so that is the command a host
/// has to recognize.
#[test]
fn recursive_answers_with_the_command_it_wraps() {
    assert_eq!(name_of(&["pnpm", "recursive", "run", "build"]).as_deref(), Some("run"));
    assert_eq!(name_of(&["pnpm", "-r", "install"]).as_deref(), Some("install"));
}

/// A command line naming no command of the engine's answers `None`, so a
/// host keeps its own meaning for it. A script name is one such: the
/// engine would run the script, and a host that spells that differently
/// must not have the token taken for a command.
#[test]
fn a_command_line_naming_no_command_answers_none() {
    assert_eq!(name_of(&["pnpm"]), None);
    assert_eq!(name_of(&["pnpm", "--version"]), None);
    assert_eq!(name_of(&["pnpm", "build"]), None);
    assert_eq!(name_of(&["pnpm", "--filter", "app"]), None);
}

/// A host writes options of its own before the command. Reading one as a
/// flag would take its value for the command, so an option this grammar
/// does not declare ends the answer instead: `--require add script.js`
/// preloads a module and is not an `add`.
#[test]
fn an_option_this_grammar_does_not_declare_names_nothing() {
    assert_eq!(name_of(&["nub", "--require", "add", "script.js"]), None);
    assert_eq!(name_of(&["nub", "--experimental-loader", "install", "app.js"]), None);
    assert_eq!(name_of(&["nub", "--", "install"]), None);
}

/// `--silent` is a shorthand the engine expands before it parses, so a
/// host that passes one through still gets an answer.
#[test]
fn a_universal_shorthand_does_not_hide_the_command() {
    assert_eq!(name_of(&["pnpm", "--silent", "install"]).as_deref(), Some("install"));
}

fn dir_of(argv: &[&str]) -> Option<String> {
    working_dir(&argv.iter().map(OsString::from).collect::<Vec<_>>())
        .map(|dir| dir.to_string_lossy().into_owned())
}

/// Every spelling of the one option, before the command and after it.
/// `add` takes both a directory and package names, and pnpm reads the
/// option on either side of them.
#[test]
fn the_working_directory_is_named_by_any_of_its_spellings() {
    assert_eq!(dir_of(&["pnpm", "install", "--dir", "target"]).as_deref(), Some("target"));
    assert_eq!(dir_of(&["pnpm", "--dir", "target", "install"]).as_deref(), Some("target"));
    assert_eq!(dir_of(&["pnpm", "install", "--dir=target"]).as_deref(), Some("target"));
    assert_eq!(dir_of(&["pnpm", "install", "-C", "target"]).as_deref(), Some("target"));
    assert_eq!(dir_of(&["pnpm", "install", "-Ctarget"]).as_deref(), Some("target"));
    assert_eq!(dir_of(&["pnpm", "install", "--prefix", "target"]).as_deref(), Some("target"));
    assert_eq!(dir_of(&["pnpm", "add", "left-pad", "-C", "target"]).as_deref(), Some("target"));
    assert_eq!(dir_of(&["pnpm", "install"]), None);
}

/// The cases a host scanning for the option itself would get wrong: an
/// earlier option claims the token after it, a short cluster hides the
/// `C`, and a `C` that follows a value-taking short is that option's
/// value rather than this one.
#[test]
fn the_working_directory_is_not_what_a_naive_scan_would_find() {
    assert_eq!(dir_of(&["pnpm", "--store-dir", "--dir", "install"]), None);
    assert_eq!(dir_of(&["pnpm", "-rC", "target", "install"]).as_deref(), Some("target"));
    assert_eq!(dir_of(&["pnpm", "--filter", "-C", "install"]), None);
    assert_eq!(dir_of(&["pnpm", "-FC", "install"]), None);
    assert_eq!(dir_of(&["pnpm", "install", "--", "--dir", "target"]), None);
}

/// A command that passes its tail to a child gives the tail away, so the
/// option belongs to the child. Measured against pnpm 12.4.1: `pnpm run
/// show --dir target` hands `--dir target` to the script, and node
/// rejects it.
#[test]
fn a_directory_in_a_child_s_tail_is_the_child_s() {
    assert_eq!(dir_of(&["pnpm", "run", "build", "--dir", "target"]), None);
    assert_eq!(dir_of(&["pnpm", "run", "--dir", "target", "build"]).as_deref(), Some("target"));
}

/// An option this grammar does not declare is a host's own, and reading
/// its value as a directory would send the host somewhere the command
/// line never named.
#[test]
fn an_undeclared_option_withholds_an_answer() {
    assert_eq!(dir_of(&["pnpm", "--require", "--dir", "install"]), None);
}
