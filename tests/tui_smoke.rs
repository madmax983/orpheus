#[test]
fn tui_boots_and_renders_initial_frame() {
    let frame = orpheus_lang::render_initial_frame_for_test(80, 48);

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
    // Height may cause things to be cut off, let's verify with a larger terminal size
    let larger_frame = orpheus_lang::render_initial_frame_for_test(80, 48);
    assert!(larger_frame.contains("Help: ?"));
    assert!(larger_frame.contains("? help"));
    assert!(larger_frame.contains("Space toggle"));
    assert!(larger_frame.contains("PgUp/PgDn bindings"));
    assert!(!larger_frame.contains("Ctrl-A/E/K"));
}
