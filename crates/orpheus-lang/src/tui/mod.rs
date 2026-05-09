//! Hypertile-powered TUI for the Orpheus live-coding environment.
//!
//! Replaces the fixed 3-pane layout with a dynamic tiling window manager
//! backed by `ratatui-hypertile-extras`. Users can split, resize, move, and
//! close panes at will, switch between layout and input modes, and use the
//! command palette to spawn new pane types.

mod plugins;
mod state;
#[allow(clippy::redundant_pub_crate)]
pub(crate) mod style;

use std::cell::RefCell;
use std::io;
use std::path::Path;
use std::rc::Rc;
use std::time::{Duration, Instant};

use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use orpheus_dsp::EngineHandle;
use ratatui::backend::{Backend, CrosstermBackend};
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Wrap};
use ratatui::{Frame, Terminal};
use ratatui_hypertile::PaneId;
use ratatui_hypertile::raw::Node;
use ratatui_hypertile_extras::{
    AnimationConfig, HypertileRuntime, HypertileRuntimeBuilder, InputMode, ModeIndicator,
    MoveBindings, SplitBehavior, WorkspaceRuntime, event_from_crossterm,
};

use plugins::{BindingsPlugin, ReplPlugin, TransportPlugin};
use state::SharedState;
use style::{
    format_cycle_position, format_tempo_bpm, format_transport_status, help_overlay_border_style,
    help_overlay_footer_style, key_legend_style, transport_status_style,
};

const EVENT_POLL_INTERVAL: Duration = Duration::from_millis(50);

const FULL_KEY_LEGEND: &str = "? help   i input   p palette   s/v split";
const MEDIUM_KEY_LEGEND: &str = "? help   i input   p palette";
const COMPACT_KEY_LEGEND: &str = "? i p s/v";

const FULL_HELP_FOOTER: &str = "Esc close   ? toggle   Ctrl-C quit";
const MEDIUM_HELP_FOOTER: &str = "Esc close   ?   Ctrl-C";
const COMPACT_HELP_FOOTER: &str = "Esc ? Ctrl-C";
const MIN_HELP_FOOTER: &str = "Esc ?";

fn help_key_line(
    key: &'static str,
    desc: &'static str,
    key_style: Style,
    desc_style: Style,
) -> Line<'static> {
    Line::from(vec![
        Span::styled(key, key_style),
        Span::styled(format!(": {desc}"), desc_style),
    ])
}

fn help_overlay_body() -> Vec<Line<'static>> {
    use ratatui::style::Modifier;

    let header_style = Style::default()
        .fg(Color::Yellow)
        .add_modifier(Modifier::BOLD);
    let key_style = Style::default()
        .fg(Color::Cyan)
        .add_modifier(Modifier::BOLD);
    let desc_style = Style::default().fg(Color::DarkGray);

    let mut lines = vec![
        help_key_line("Toggle", "?", key_style, desc_style),
        help_key_line("Close", "Esc", key_style, desc_style),
        Line::raw(""),
        Line::styled("Layout mode (Esc from input):", header_style),
    ];

    let layout_keys = [
        ("  hjkl / arrows", "focus pane"),
        ("  s / v", "split horizontal / vertical"),
        ("  d", "close pane"),
        ("  [ / ]", "resize pane"),
        ("  p", "command palette"),
        ("  i / Enter", "enter input mode"),
        ("  HJKL / Shift+arrows", "move pane"),
        ("  Tab / Shift+Tab", "cycle focus"),
        ("  Ctrl+t/w", "new/close tab"),
        ("  Ctrl+n/p", "next/prev tab"),
    ];

    for (k, d) in layout_keys {
        lines.push(help_key_line(k, d, key_style, desc_style));
    }

    lines.push(Line::raw(""));
    lines.push(Line::styled("Input mode (i or Enter):", header_style));

    let input_keys = [
        ("  Type", "evaluate expressions"),
        ("  Tab", "complete commands"),
        ("  Up/Down", "history recall"),
        ("  Space", "toggle transport (empty input)"),
        ("  Ctrl-A/E/K/U/W/L", "emacs editing"),
        ("  Alt-B/F", "word navigation"),
        ("  Esc", "back to layout mode"),
    ];

    for (k, d) in input_keys {
        lines.push(help_key_line(k, d, key_style, desc_style));
    }

    lines.push(Line::raw(""));
    lines.push(Line::styled("Commands:", header_style));
    lines.push(Line::from(vec![Span::styled(
        "  :play / :stop / :tempo <bpm>",
        key_style,
    )]));
    lines.push(Line::from(vec![Span::styled(
        "  :track / :bus new|fx / :send / :mixer",
        key_style,
    )]));
    lines.push(Line::from(vec![Span::styled(
        "  :render / :export / :roll / :stats / :explain",
        key_style,
    )]));
    lines.push(Line::from(vec![Span::styled(
        "  :open <path>   :quit",
        key_style,
    )]));

    lines
}

/// `PaneId` for the initial 3-pane layout.
const REPL_PANE: PaneId = PaneId::ROOT;

fn bindings_pane_id() -> PaneId {
    PaneId::new(1)
}

fn transport_pane_id() -> PaneId {
    PaneId::new(2)
}

/// Plugin type names registered with the runtime.
const REPL_PLUGIN: &str = "repl";
const BINDINGS_PLUGIN: &str = "bindings";
const TRANSPORT_PLUGIN: &str = "transport";

/// Runs the interactive ratatui session shell with the provided audio engine.
///
/// # Errors
///
/// Returns any terminal initialization, draw, input polling, or terminal
/// restoration failure encountered while the shell is active.
pub fn run_with_engine(engine: EngineHandle) -> io::Result<()> {
    run_with_engine_and_path(engine, None, None)
}

/// Runs the interactive ratatui session shell with an optional startup `.ode`
/// preload.
///
/// # Errors
///
/// Returns startup file load failures or terminal initialization, draw, input
/// polling, or restoration failures encountered while the shell is active.
pub fn run_with_engine_and_path(
    engine: EngineHandle,
    startup_path: Option<&Path>,
    warning: Option<String>,
) -> io::Result<()> {
    let _terminal_guard = TerminalGuard::enter()?;
    let backend = CrosstermBackend::new(io::stdout());
    let mut terminal = Terminal::new(backend)?;

    let shared = Rc::new(RefCell::new(SharedState::with_startup(
        engine,
        startup_path,
        warning,
    )));

    let mut workspace = build_workspace(Rc::clone(&shared));
    let result = run_event_loop(&mut terminal, &shared, &mut workspace);
    terminal.show_cursor()?;
    result
}

fn build_runtime(shared: &Rc<RefCell<SharedState>>) -> HypertileRuntime {
    let mut runtime = HypertileRuntimeBuilder::default()
        .with_move_bindings(MoveBindings::VimAndShiftArrows)
        .with_split_behavior(SplitBehavior::PromptPalette)
        .with_animation_config(AnimationConfig {
            enabled: true,
            duration: Duration::from_millis(150),
            frame_interval: Duration::from_millis(16),
        })
        .with_gap(1)
        .build();

    // Register plugin types with factories that capture shared state.
    let s = Rc::clone(shared);
    runtime.register_plugin_type(REPL_PLUGIN, move || ReplPlugin {
        state: Rc::clone(&s),
    });

    let s = Rc::clone(shared);
    runtime.register_plugin_type(BINDINGS_PLUGIN, move || BindingsPlugin::new(Rc::clone(&s)));

    let s = Rc::clone(shared);
    runtime.register_plugin_type(TRANSPORT_PLUGIN, move || TransportPlugin {
        state: Rc::clone(&s),
    });

    // Set up initial 3-pane layout:
    //   left (bindings | repl) | right (transport)
    let initial_tree = Node::Split {
        direction: Direction::Horizontal,
        ratio: 0.72,
        first: Box::new(Node::Split {
            direction: Direction::Vertical,
            ratio: 0.58,
            first: Box::new(Node::Pane(bindings_pane_id())),
            second: Box::new(Node::Pane(REPL_PANE)),
        }),
        second: Box::new(Node::Pane(transport_pane_id())),
    };

    // Replace default tree and mount our plugins.
    let _ = runtime.set_root(initial_tree);
    let _ = runtime.replace_pane_plugin(REPL_PANE, REPL_PLUGIN);
    let _ = runtime.replace_pane_plugin(bindings_pane_id(), BINDINGS_PLUGIN);
    let _ = runtime.replace_pane_plugin(transport_pane_id(), TRANSPORT_PLUGIN);

    // Start with REPL focused and in input mode.
    let _ = runtime.focus_pane(REPL_PANE);
    runtime.set_mode(InputMode::PluginInput);

    runtime
}

fn build_workspace(shared: Rc<RefCell<SharedState>>) -> WorkspaceRuntime {
    WorkspaceRuntime::new(move || build_runtime(&shared))
}

fn run_event_loop<B>(
    terminal: &mut Terminal<B>,
    shared: &Rc<RefCell<SharedState>>,
    workspace: &mut WorkspaceRuntime,
) -> io::Result<()>
where
    B: Backend,
    io::Error: From<B::Error>,
{
    loop {
        {
            let mut state = shared.borrow_mut();
            state.clear_status_if_expired(Instant::now());
            if state.should_quit {
                break;
            }
        }

        terminal.draw(|frame| render_frame(frame, shared, workspace))?;

        let poll_duration = workspace
            .next_frame_in()
            .map_or(EVENT_POLL_INTERVAL, |d| d.min(EVENT_POLL_INTERVAL));

        if !event::poll(poll_duration)? {
            continue;
        }

        if let Event::Key(key) = event::read()? {
            if key.kind != KeyEventKind::Press {
                continue;
            }
            handle_key(shared, workspace, key);
        }
    }

    Ok(())
}

fn handle_key(
    shared: &Rc<RefCell<SharedState>>,
    workspace: &mut WorkspaceRuntime,
    key: crossterm::event::KeyEvent,
) {
    let mut state = shared.borrow_mut();

    // Ctrl-C always quits.
    if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
        state.should_quit = true;
        return;
    }

    // Help overlay is modal — swallow all keys except toggle/close/quit.
    if state.show_help {
        match key.code {
            KeyCode::Esc => state.close_help(),
            KeyCode::Char('?') if key.modifiers.is_empty() => state.toggle_help(),
            _ => {}
        }
        return;
    }

    let mode = workspace.active_runtime().mode();

    // '?' toggles help (only in layout mode so it doesn't eat typing).
    if key.code == KeyCode::Char('?') && key.modifiers.is_empty() && mode == InputMode::Layout {
        state.toggle_help();
        return;
    }

    // Space toggles transport globally in layout mode.
    if key.code == KeyCode::Char(' ') && key.modifiers.is_empty() && mode == InputMode::Layout {
        state.toggle_transport_hotkey();
        return;
    }

    // Drop borrow before forwarding to workspace (plugins also borrow shared).
    drop(state);

    if let Some(ht_event) = event_from_crossterm(key) {
        workspace.handle_event(ht_event);
    }
}

fn render_frame(
    frame: &mut Frame<'_>,
    shared: &Rc<RefCell<SharedState>>,
    workspace: &mut WorkspaceRuntime,
) {
    let [body, footer] = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(1)])
        .areas(frame.area());

    // Render hypertile panes into the body area.
    workspace.render(body, frame.buffer_mut());

    // Footer: mode indicator + key legend + transport status.
    let state = shared.borrow();
    let mode = workspace.active_runtime().mode();
    let footer_line = build_footer_line(footer.width, mode, &state);

    if state.show_help {
        render_help_overlay(frame);
    }

    frame.render_widget(
        Paragraph::new(footer_line)
            .style(if state.show_help {
                help_overlay_footer_style().bg(Color::DarkGray)
            } else {
                key_legend_style()
            })
            .wrap(Wrap { trim: false }),
        footer,
    );

    // Render mode indicator badge in the footer.
    let mode_area = Rect::new(footer.x, footer.y, 9, 1);
    frame.render_widget(ModeIndicator::new(mode), mode_area);
}

fn build_footer_line(width: u16, mode: InputMode, state: &SharedState) -> Line<'static> {
    if state.show_help {
        return help_footer_line(width);
    }

    let transport = state.transport_view();
    let status = format_transport_status(&transport).to_owned();
    let tempo = format!("{} BPM", format_tempo_bpm(transport.snapshot()));
    let cycle = format_cycle_position(transport.snapshot());

    // Mode badge takes ~9 chars, add spacer.
    let badge_width = 10;
    let remaining = usize::from(width).saturating_sub(badge_width);

    let legend = match mode {
        InputMode::Layout => {
            if remaining > 60 {
                Some(FULL_KEY_LEGEND)
            } else if remaining > 40 {
                Some(MEDIUM_KEY_LEGEND)
            } else if remaining > 20 {
                Some(COMPACT_KEY_LEGEND)
            } else {
                None
            }
        }
        InputMode::PluginInput => {
            if remaining > 50 {
                Some("Esc layout   ? help   Space toggle(empty)")
            } else if remaining > 30 {
                Some("Esc layout   ? help")
            } else {
                Some("Esc ?")
            }
        }
    };

    let mut spans = vec![Span::raw(" ".repeat(badge_width))]; // space for mode badge
    if let Some(legend) = legend {
        spans.push(Span::styled(legend.to_owned(), key_legend_style()));
    }
    spans.push(Span::styled(" | ", key_legend_style()));
    spans.push(Span::styled(status, transport_status_style(&transport)));
    spans.push(Span::styled(
        format!(" | {tempo} | {cycle}"),
        key_legend_style(),
    ));

    Line::from(spans)
}

fn help_footer_line(width: u16) -> Line<'static> {
    let width = usize::from(width);
    for footer in [
        FULL_HELP_FOOTER,
        MEDIUM_HELP_FOOTER,
        COMPACT_HELP_FOOTER,
        MIN_HELP_FOOTER,
    ] {
        if footer.len() <= width {
            return Line::styled(footer, help_overlay_footer_style());
        }
    }
    Line::styled("?", help_overlay_footer_style())
}

fn render_help_overlay(frame: &mut Frame<'_>) {
    let overlay_area = centered_rect(frame.area(), 68, 72);
    render_modal_backdrop(frame, overlay_area);
    frame.render_widget(Clear, overlay_area);
    let border_style = help_overlay_border_style();
    let block = Block::default()
        .title("Help")
        .title_style(border_style)
        .border_style(border_style)
        .borders(Borders::ALL);
    let inner = block.inner(overlay_area);
    let [body_area, footer_area] = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(1)])
        .areas(inner);
    frame.render_widget(block.style(Style::default().bg(Color::Black)), overlay_area);
    frame.render_widget(
        Paragraph::new(help_overlay_body())
            .style(Style::default().bg(Color::Black))
            .wrap(Wrap { trim: false }),
        body_area,
    );
    frame.render_widget(
        Paragraph::new(FULL_HELP_FOOTER)
            .style(help_overlay_footer_style())
            .wrap(Wrap { trim: false }),
        footer_area,
    );
}

fn centered_rect(area: Rect, width_percent: u16, height_percent: u16) -> Rect {
    let [_, vertical, _] = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - height_percent) / 2),
            Constraint::Percentage(height_percent),
            Constraint::Percentage((100 - height_percent) / 2),
        ])
        .areas(area);
    let [_, horizontal, _] = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - width_percent) / 2),
            Constraint::Percentage(width_percent),
            Constraint::Percentage((100 - width_percent) / 2),
        ])
        .areas(vertical);
    horizontal
}

fn render_modal_backdrop(frame: &mut Frame<'_>, overlay_area: Rect) {
    let backdrop_style = Style::default().bg(Color::DarkGray);
    for area in modal_backdrop_areas(frame.area(), overlay_area) {
        if area.width == 0 || area.height == 0 {
            continue;
        }
        frame.render_widget(Block::default().style(backdrop_style), area);
    }
}

const fn modal_backdrop_areas(area: Rect, overlay_area: Rect) -> [Rect; 4] {
    let area_bottom = area.y.saturating_add(area.height);
    let area_right = area.x.saturating_add(area.width);
    let overlay_bottom = overlay_area.y.saturating_add(overlay_area.height);
    let overlay_right = overlay_area.x.saturating_add(overlay_area.width);

    [
        Rect::new(
            area.x,
            area.y,
            area.width,
            overlay_area.y.saturating_sub(area.y),
        ),
        Rect::new(
            area.x,
            overlay_bottom,
            area.width,
            area_bottom.saturating_sub(overlay_bottom),
        ),
        Rect::new(
            area.x,
            overlay_area.y,
            overlay_area.x.saturating_sub(area.x),
            overlay_area.height,
        ),
        Rect::new(
            overlay_right,
            overlay_area.y,
            area_right.saturating_sub(overlay_right),
            overlay_area.height,
        ),
    ]
}

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

struct TerminalGuard;

impl TerminalGuard {
    fn enter() -> io::Result<Self> {
        enable_raw_mode()?;
        execute!(io::stdout(), EnterAlternateScreen)?;
        Ok(Self)
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let _ = execute!(io::stdout(), LeaveAlternateScreen);
    }
}

#[must_use]
/// Renders the first TUI frame against a test backend and returns its text.
///
/// # Panics
///
/// Panics if the in-memory ratatui test backend fails to initialize or render a
/// single frame.
pub fn render_initial_frame_for_test(width: u16, height: u16) -> String {
    use ratatui::backend::TestBackend;

    let shared = Rc::new(RefCell::new(SharedState::new(EngineHandle::stub())));
    let mut workspace = build_workspace(Rc::clone(&shared));

    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend)
        .unwrap_or_else(|error| panic!("test backend should create a terminal: {error}"));
    terminal
        .draw(|frame| render_frame(frame, &shared, &mut workspace))
        .unwrap_or_else(|error| panic!("test backend should render one frame: {error}"));

    buffer_to_string(terminal.backend().buffer())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyEvent, KeyEventKind};
    use orpheus_dsp::EngineHandle;
    use ratatui::backend::TestBackend;
    fn test_setup() -> (Rc<RefCell<SharedState>>, WorkspaceRuntime) {
        let shared = Rc::new(RefCell::new(SharedState::new(EngineHandle::stub())));
        let workspace = build_workspace(Rc::clone(&shared));
        (shared, workspace)
    }

    fn press(code: KeyCode) -> crossterm::event::KeyEvent {
        KeyEvent {
            code,
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Press,
            state: crossterm::event::KeyEventState::NONE,
        }
    }

    fn ctrl(code: KeyCode) -> crossterm::event::KeyEvent {
        KeyEvent {
            code,
            modifiers: KeyModifiers::CONTROL,
            kind: KeyEventKind::Press,
            state: crossterm::event::KeyEventState::NONE,
        }
    }

    fn render_frame_str(
        shared: &Rc<RefCell<SharedState>>,
        workspace: &mut WorkspaceRuntime,
        width: u16,
        height: u16,
    ) -> String {
        let backend = TestBackend::new(width, height);
        let mut terminal = Terminal::new(backend)
            .unwrap_or_else(|error| panic!("test backend should create: {error}"));
        terminal
            .draw(|frame| render_frame(frame, shared, workspace))
            .unwrap_or_else(|error| panic!("test backend should render: {error}"));
        buffer_to_string(terminal.backend().buffer())
    }

    #[test]
    fn initial_frame_shows_three_panes() {
        let (shared, mut workspace) = test_setup();
        let frame = render_frame_str(&shared, &mut workspace, 120, 30);
        assert!(frame.contains("REPL"));
        assert!(frame.contains("Bindings"));
        assert!(frame.contains("Transport"));
    }

    #[test]
    fn initial_frame_starts_in_input_mode() {
        let (_shared, workspace) = test_setup();
        assert_eq!(workspace.active_runtime().mode(), InputMode::PluginInput);
    }

    #[test]
    fn typing_in_input_mode_updates_repl() {
        let (shared, mut workspace) = test_setup();

        // Type "bd sn" into the REPL
        for c in ['b', 'd', ' ', 's', 'n'] {
            let key = press(KeyCode::Char(c));
            handle_key(&shared, &mut workspace, key);
        }

        let state = shared.borrow();
        assert_eq!(state.input, "bd sn");
    }

    #[test]
    fn ctrl_c_quits() {
        let (shared, mut workspace) = test_setup();
        handle_key(&shared, &mut workspace, ctrl(KeyCode::Char('c')));
        assert!(shared.borrow().should_quit);
    }

    #[test]
    fn help_overlay_toggles_with_question_mark_in_layout_mode() {
        let (shared, mut workspace) = test_setup();

        // Switch to layout mode first (Esc)
        handle_key(&shared, &mut workspace, press(KeyCode::Esc));
        assert_eq!(workspace.active_runtime().mode(), InputMode::Layout);

        // Toggle help
        handle_key(&shared, &mut workspace, press(KeyCode::Char('?')));
        assert!(shared.borrow().show_help);

        // Close help
        handle_key(&shared, &mut workspace, press(KeyCode::Esc));
        assert!(!shared.borrow().show_help);
    }

    #[test]
    fn submit_line_creates_binding() {
        let (shared, mut workspace) = test_setup();

        // Type and submit
        for c in "drums = bd sn".chars() {
            handle_key(&shared, &mut workspace, press(KeyCode::Char(c)));
        }
        handle_key(&shared, &mut workspace, press(KeyCode::Enter));

        let state = shared.borrow();
        assert!(state.input.is_empty());
        assert!(
            state
                .session
                .binding_summaries()
                .iter()
                .any(|s| s.contains("drums"))
        );
    }

    #[test]
    fn footer_shows_mode_indicator() {
        let (shared, mut workspace) = test_setup();
        let frame = render_frame_str(&shared, &mut workspace, 120, 30);
        // Input mode should show INPUT badge
        assert!(frame.contains("INPUT"));
    }

    #[test]
    fn escape_switches_to_layout_mode() {
        let (shared, mut workspace) = test_setup();
        handle_key(&shared, &mut workspace, press(KeyCode::Esc));
        assert_eq!(workspace.active_runtime().mode(), InputMode::Layout,);
    }
}

#[cfg(test)]
mod help_overlay_tests {
    use super::*;
    use ratatui::backend::TestBackend;

    #[test]
    fn test_render_help_overlay() {
        let backend = TestBackend::new(100, 100);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|frame| {
                render_help_overlay(frame);
            })
            .unwrap();
        let buffer = terminal.backend().buffer();
        let content = buffer_to_string(buffer);
        assert!(content.contains("Help"));
    }
}

#[cfg(test)]
mod layout_tests {
    use super::*;

    #[test]
    fn test_help_footer_line() {
        let full_width = FULL_HELP_FOOTER.len() as u16;
        let line = help_footer_line(full_width);
        assert_eq!(line.spans[0].content, FULL_HELP_FOOTER);

        let med_width = MEDIUM_HELP_FOOTER.len() as u16;
        let line = help_footer_line(med_width);
        assert_eq!(line.spans[0].content, MEDIUM_HELP_FOOTER);

        let compact_width = COMPACT_HELP_FOOTER.len() as u16;
        let line = help_footer_line(compact_width);
        assert_eq!(line.spans[0].content, COMPACT_HELP_FOOTER);

        let min_width = MIN_HELP_FOOTER.len() as u16;
        let line = help_footer_line(min_width);
        assert_eq!(line.spans[0].content, MIN_HELP_FOOTER);

        let tiny_width = 1;
        let line = help_footer_line(tiny_width);
        assert_eq!(line.spans[0].content, "?");
    }

    #[test]
    fn test_centered_rect() {
        let area = Rect::new(0, 0, 100, 100);
        let rect = centered_rect(area, 50, 50);
        assert_eq!(rect.x, 25);
        assert_eq!(rect.y, 25);
        assert_eq!(rect.width, 50);
        assert_eq!(rect.height, 50);

        let rect2 = centered_rect(area, 68, 72);
        assert_eq!(rect2.x, 16);
        assert_eq!(rect2.y, 14);
        assert_eq!(rect2.width, 68);
        assert_eq!(rect2.height, 72);
    }

    #[test]
    fn test_modal_backdrop_areas() {
        let area = Rect::new(0, 0, 100, 100);
        let overlay = Rect::new(20, 20, 60, 60);
        let areas = modal_backdrop_areas(area, overlay);

        // Top
        assert_eq!(areas[0], Rect::new(0, 0, 100, 20));
        // Bottom
        assert_eq!(areas[1], Rect::new(0, 80, 100, 20));
        // Left
        assert_eq!(areas[2], Rect::new(0, 20, 20, 60));
        // Right
        assert_eq!(areas[3], Rect::new(80, 20, 20, 60));
    }
}
