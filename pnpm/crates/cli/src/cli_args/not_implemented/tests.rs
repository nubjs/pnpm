use super::NotImplementedError;

/// The sentence names whichever program the user actually ran, so an
/// embedder's users are not told that some other program they never
/// installed has not implemented the command they typed.
#[test]
fn the_message_names_the_program_that_was_run() {
    let pnpm = NotImplementedError { command: "edit", program: "pnpm" }.to_string();
    assert_eq!(
        pnpm,
        r#"The "edit" command is not yet implemented in pnpm. Use the npm CLI directly: npm edit"#
    );

    let embedded = NotImplementedError { command: "edit", program: "nub" }.to_string();
    assert_eq!(
        embedded,
        r#"The "edit" command is not yet implemented in nub. Use the npm CLI directly: npm edit"#
    );
}

/// npm is named because npm is where the command lives, which is true
/// whoever asks — so it stays put while the program name moves.
#[test]
fn the_npm_pointer_does_not_move_with_the_program_name() {
    for program in ["pnpm", "nub"] {
        let rendered = NotImplementedError { command: "profile", program }.to_string();
        assert!(
            rendered.ends_with("Use the npm CLI directly: npm profile"),
            "{program}: {rendered}"
        );
    }
}
