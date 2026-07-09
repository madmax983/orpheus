//! End-to-end TUI test for the per-track level meters (ADR 0013).
//!
//! Seeds a live signal through the public [`ReplSession`] API, renders the
//! mixer summary (which carries the meter bars) to an in-memory ratatui
//! `TestBackend`, and asserts the seeded meter bar reaches the terminal buffer.

use orpheus_dsp::EngineHandle;
use orpheus_lang::ReplSession;
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::text::Text;
use ratatui::widgets::Paragraph;

fn buffer_to_string(buffer: &ratatui::buffer::Buffer) -> String {
    let area = buffer.area;
    let mut lines = Vec::with_capacity(usize::from(area.height));
    for y in 0..area.height {
        let mut line = String::new();
        for x in 0..area.width {
            line.push_str(buffer[(x, y)].symbol());
        }
        lines.push(line);
    }
    lines.join("\n")
}

#[test]
fn seeded_track_meter_bar_renders_to_the_terminal_buffer() {
    let mut session = ReplSession::with_engine(EngineHandle::stub());

    // Bind a pattern to the main track and advance past a cycle boundary so the
    // triggers sound and the peak meter fills.
    session
        .eval_line("drums = bd sn")
        .unwrap_or_else(|error| panic!("binding should evaluate: {error}"));
    let _ = session.render_test_block_for_tui(100_000);

    // Sanity: the engine reports a live post-fader peak on track 0.
    assert!(
        session.meter_view().track_peak(0) > 0.0,
        "expected a live peak on the main track"
    );

    // The mixer summary carries the Theme-colored meter spans; render them.
    let mixer = session.mixer_view();
    let lines = mixer.tui_summary().to_vec();

    let backend = TestBackend::new(80, 12);
    let mut terminal = Terminal::new(backend)
        .unwrap_or_else(|error| panic!("test backend should create: {error}"));
    terminal
        .draw(|frame| {
            frame.render_widget(Paragraph::new(Text::from(lines)), frame.area());
        })
        .unwrap_or_else(|error| panic!("test backend should render: {error}"));

    let rendered = buffer_to_string(terminal.backend().buffer());
    assert!(rendered.contains("Meter"), "meter column header missing");
    assert!(
        rendered.contains('\u{2588}'),
        "expected a filled meter glyph for the sounding track, got:\n{rendered}"
    );
}
