#[test]
fn tui_boots_and_renders_initial_frame() {
    let frame = orpheus_lang::render_initial_frame_for_test(80, 30);

    assert!(frame.contains("Bindings"));
    assert!(frame.contains("REPL"));
    assert!(frame.contains("Transport"));

    // The new TUI output uses a larger table, so we need a larger height to fit all hints.
    let larger_frame = orpheus_lang::render_initial_frame_for_test(140, 40);

    assert!(larger_frame.contains(":render"));
    assert!(larger_frame.contains(":tempo"));
    assert!(larger_frame.contains(":play"));
    assert!(larger_frame.contains(":stop"));
    assert!(larger_frame.contains("Space"));
    assert!(frame.contains("empty input"));
    assert!(frame.contains("Hint: Tab completes commands."));
    assert!(frame.contains("Pattern: none"));
    assert!(frame.contains("120 BPM"));
    assert!(frame.contains("0.000"));
    assert!(frame.contains("Transport: playing"));
    assert!(larger_frame.contains("Help: ?"));
    assert!(larger_frame.contains("? help"));
    assert!(larger_frame.contains("Space toggle"));
    assert!(larger_frame.contains("PgUp/PgDn bindings"));
    assert!(!larger_frame.contains("Ctrl-A/E/K"));
}
