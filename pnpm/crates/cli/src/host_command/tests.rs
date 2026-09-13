use super::command_name;
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
