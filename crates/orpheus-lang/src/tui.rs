use std::io;
use std::time::Duration;

use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use orpheus_dsp::EngineHandle;
use ratatui::backend::{Backend, CrosstermBackend, TestBackend};
use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::widgets::{Block, Borders, List, ListItem, Paragraph, Wrap};
use ratatui::{Frame, Terminal};

use crate::repl::ReplSession;

const EVENT_POLL_INTERVAL: Duration = Duration::from_millis(50);
const DEFAULT_TEMPO_BPM: u32 = 120;

/// Runs the interactive ratatui session shell with the provided audio engine.
///
/// # Errors
///
/// Returns any terminal initialization, draw, input polling, or terminal
/// restoration failure encountered while the shell is active.
pub fn run_with_engine(engine: EngineHandle) -> io::Result<()> {
    let _terminal_guard = TerminalGuard::enter()?;
    let backend = CrosstermBackend::new(io::stdout());
    let mut terminal = Terminal::new(backend)?;
    let mut app = SessionTui::new(engine);
    let result = run_event_loop(&mut terminal, &mut app);
    terminal.show_cursor()?;
    result
}

#[must_use]
/// Renders the first TUI frame against a test backend and returns its text.
///
/// # Panics
///
/// Panics if the in-memory ratatui test backend fails to initialize or render a
/// single frame.
pub fn render_initial_frame_for_test(width: u16, height: u16) -> String {
    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend)
        .unwrap_or_else(|error| panic!("test backend should create a terminal: {error}"));
    let app = SessionTui::new(EngineHandle::stub());
    terminal
        .draw(|frame| render_session_frame(frame, &app))
        .unwrap_or_else(|error| panic!("test backend should render one frame: {error}"));
    buffer_to_string(terminal.backend().buffer())
}

fn run_event_loop<B>(terminal: &mut Terminal<B>, app: &mut SessionTui) -> io::Result<()>
where
    B: Backend,
{
    while !app.should_quit {
        terminal.draw(|frame| render_session_frame(frame, app))?;

        if !event::poll(EVENT_POLL_INTERVAL)? {
            continue;
        }

        if let Event::Key(key) = event::read()? {
            handle_key_event(app, key);
        }
    }

    Ok(())
}

fn handle_key_event(app: &mut SessionTui, key: KeyEvent) {
    if key.kind != KeyEventKind::Press {
        return;
    }

    match key.code {
        KeyCode::Esc => app.should_quit = true,
        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            app.should_quit = true;
        }
        KeyCode::Backspace => {
            app.input.pop();
        }
        KeyCode::Enter => app.submit_line(),
        KeyCode::Char(character) => app.input.push(character),
        _ => {}
    }
}

fn render_session_frame(frame: &mut Frame<'_>, app: &SessionTui) {
    let [left, right] = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(72), Constraint::Percentage(28)])
        .areas(frame.area());
    let [bindings, repl] = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(58), Constraint::Percentage(42)])
        .areas(left);

    let binding_items = app.binding_lines();
    frame.render_widget(
        List::new(binding_items).block(Block::default().title("Bindings").borders(Borders::ALL)),
        bindings,
    );

    frame.render_widget(
        Paragraph::new(app.repl_body())
            .block(Block::default().title("REPL").borders(Borders::ALL))
            .wrap(Wrap { trim: false }),
        repl,
    );

    frame.render_widget(
        Paragraph::new(SessionTui::transport_body())
            .block(Block::default().title("Transport").borders(Borders::ALL))
            .wrap(Wrap { trim: false }),
        right,
    );
}

fn buffer_to_string(buffer: &Buffer) -> String {
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

struct SessionTui {
    session: ReplSession,
    transcript: Vec<String>,
    input: String,
    should_quit: bool,
}

impl SessionTui {
    fn new(engine: EngineHandle) -> Self {
        Self {
            session: ReplSession::with_engine(engine),
            transcript: vec![
                "Interactive shell ready.".to_owned(),
                "Press Esc to quit.".to_owned(),
            ],
            input: String::new(),
            should_quit: false,
        }
    }

    fn submit_line(&mut self) {
        let line = self.input.trim().to_owned();
        self.input.clear();

        if line.is_empty() {
            return;
        }
        if line == ":quit" {
            self.transcript.push("> :quit".to_owned());
            self.should_quit = true;
            return;
        }

        self.transcript.push(format!("> {line}"));
        match self.session.eval_line(&line) {
            Ok(message) => self.transcript.push(message),
            Err(message) => self.transcript.push(format!("! {message}")),
        }
    }

    fn binding_lines(&self) -> Vec<ListItem<'static>> {
        let bindings = self.session.binding_summaries();
        if bindings.is_empty() {
            vec![ListItem::new("No bindings yet")]
        } else {
            bindings.into_iter().map(ListItem::new).collect()
        }
    }

    fn repl_body(&self) -> String {
        let mut lines = self.transcript.clone();
        lines.push(format!("> {}", self.input));
        lines.join("\n")
    }

    fn transport_body() -> String {
        format!(
            "Status: live shell\nTempo: {DEFAULT_TEMPO_BPM} BPM\nAudio: cycle-locked\nExport: :render <binding> <path> [cycles]\nQuit: Esc or :quit"
        )
    }
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
