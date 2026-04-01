#[test]
fn tui_boots_and_renders_initial_frame() {
    let frame = orpheus_lang::render_initial_frame_for_test(140, 40);

    assert!(frame.contains("Bindings"));
    assert!(frame.contains("REPL"));
    assert!(frame.contains("Transport"));
    assert!(frame.contains(":render"));
    assert!(frame.contains(":tempo"));
    assert!(frame.contains(":play"));
    assert!(frame.contains(":stop"));
    assert!(frame.contains("Space"));
    assert!(frame.contains("empty input"));
    assert!(frame.contains("Hint: Tab completes commands."));
    assert!(frame.contains("Pattern: none"));
    assert!(frame.contains("120 BPM"));
    assert!(frame.contains("0.000"));
    assert!(frame.contains("Transport: playing"));
    assert!(frame.contains("Help: ?"));
    assert!(frame.contains("? help"));
    assert!(frame.contains("Space toggle"));
    assert!(frame.contains("PgUp/PgDn bindings"));
    assert!(!frame.contains("Ctrl-A/E/K"));
}
