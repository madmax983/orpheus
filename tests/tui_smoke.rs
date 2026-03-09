#[test]
fn tui_boots_and_renders_initial_frame() {
    let frame = orpheus_lang::tui::render_initial_frame_for_test(80, 24);

    assert!(frame.contains("Bindings"));
    assert!(frame.contains("REPL"));
    assert!(frame.contains("Transport"));
    assert!(frame.contains(":render"));
    assert!(frame.contains("Tab=complete"));
    assert!(frame.contains("Up/Down=history"));
    assert!(frame.contains("Left/Right=move"));
    assert!(frame.contains("Home/End/Delete"));
    assert!(frame.contains("Ctrl-A/E/K"));
}
