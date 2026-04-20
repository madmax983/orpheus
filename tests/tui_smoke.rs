//! Smoke tests for the Terminal User Interface.
#[test]
fn tui_boots_and_renders_initial_frame() {
    let frame = orpheus_lang::render_initial_frame_for_test(80, 30);

    assert!(frame.contains("Bindings"));
    assert!(frame.contains("REPL"));
    assert!(frame.contains("Transport"));

    let larger_frame = orpheus_lang::render_initial_frame_for_test(140, 40);

    // Transport pane shows command reference.
    assert!(larger_frame.contains(":render"));
    assert!(larger_frame.contains(":tempo"));
    assert!(larger_frame.contains(":play"));
    assert!(larger_frame.contains(":stop"));
    assert!(larger_frame.contains("Space"));
    assert!(larger_frame.contains("empty input"));
    assert!(larger_frame.contains("Hint: Tab completes commands."));
    assert!(larger_frame.contains("Pattern: none"));

    // Footer shows transport metrics.
    assert!(larger_frame.contains("120 BPM"));
    assert!(larger_frame.contains("0.000"));

    // REPL pane shows transport status.
    assert!(larger_frame.contains("Transport: playing"));

    // Footer shows mode indicator (starts in INPUT mode).
    assert!(larger_frame.contains("INPUT"));

    // Help reference is in the transport pane.
    assert!(larger_frame.contains("Help      : ?"));

    // Help overlay internals should NOT be visible at boot.
    assert!(!larger_frame.contains("Ctrl-A/E/K"));
}
