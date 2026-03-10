use std::io;
use std::time::{Duration, Instant};

use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use orpheus_dsp::EngineHandle;
use ratatui::backend::{Backend, CrosstermBackend, TestBackend};
use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::widgets::{Block, Borders, Clear, List, ListItem, Paragraph, Wrap};
use ratatui::{Frame, Terminal};

use crate::repl::ReplSession;

const EVENT_POLL_INTERVAL: Duration = Duration::from_millis(50);
const STATUS_TOAST_TTL: Duration = Duration::from_secs(3);
const COMMAND_HINTS: [(&str, &str); 5] = [
    (":play", ":play"),
    (":quit", ":quit"),
    (":render", ":render <binding> <path> [cycles]"),
    (":stop", ":stop"),
    (":tempo", ":tempo <bpm>"),
];

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
        app.clear_status_if_expired(Instant::now());
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
        KeyCode::Esc if app.show_help => app.close_help(),
        KeyCode::Esc => app.should_quit = true,
        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            app.should_quit = true;
        }
        KeyCode::Char('?') => app.toggle_help(),
        KeyCode::Char(' ') if key.modifiers.is_empty() && app.show_help => {}
        KeyCode::Char(' ') if key.modifiers.is_empty() && app.input.is_empty() => {
            app.toggle_transport_hotkey();
        }
        KeyCode::Char('a') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            app.move_cursor_home();
        }
        KeyCode::Char('d') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            app.delete();
        }
        KeyCode::Char('e') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            app.move_cursor_end();
        }
        KeyCode::Char('k') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            app.kill_to_end();
        }
        KeyCode::Char('l') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            app.clear_transcript();
        }
        KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            app.kill_to_start();
        }
        KeyCode::Char('w') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            app.delete_previous_word();
        }
        KeyCode::Char('b') if key.modifiers.contains(KeyModifiers::ALT) => {
            app.move_cursor_previous_word();
        }
        KeyCode::Char('f') if key.modifiers.contains(KeyModifiers::ALT) => {
            app.move_cursor_next_word();
        }
        KeyCode::Left => app.move_cursor_left(),
        KeyCode::Right => app.move_cursor_right(),
        KeyCode::Home => app.move_cursor_home(),
        KeyCode::End => app.move_cursor_end(),
        KeyCode::Up => app.recall_previous_history(),
        KeyCode::Down => app.recall_next_history(),
        KeyCode::Tab => app.complete_input(),
        KeyCode::Backspace => app.backspace(),
        KeyCode::Delete => app.delete(),
        KeyCode::Enter => app.submit_line(),
        KeyCode::Char(character) => app.insert_character(character),
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
        Paragraph::new(app.transport_body())
            .block(Block::default().title("Transport").borders(Borders::ALL))
            .wrap(Wrap { trim: false }),
        right,
    );

    if app.show_help {
        let overlay_area = centered_rect(frame.area(), 68, 60);
        frame.render_widget(Clear, overlay_area);
        frame.render_widget(
            Paragraph::new(SessionTui::help_overlay_body())
                .block(Block::default().title("Help").borders(Borders::ALL))
                .wrap(Wrap { trim: false }),
            overlay_area,
        );
    }
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
    history: Vec<String>,
    history_index: Option<usize>,
    status_message: Option<String>,
    status_expires_at: Option<Instant>,
    input: String,
    cursor_index: usize,
    show_help: bool,
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
            history: Vec::new(),
            history_index: None,
            status_message: None,
            status_expires_at: None,
            input: String::new(),
            cursor_index: 0,
            show_help: false,
            should_quit: false,
        }
    }

    fn submit_line(&mut self) {
        let line = self.input.trim().to_owned();
        self.input.clear();
        self.cursor_index = 0;
        self.history_index = None;

        if line.is_empty() {
            return;
        }

        self.history.push(line.clone());
        if line == ":quit" {
            self.should_quit = true;
            return;
        }

        if line.starts_with(':') {
            let message = self
                .session
                .eval_line(&line)
                .unwrap_or_else(|message| message);
            self.set_status_message(message);
            return;
        }

        self.clear_status_message();
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
        lines.push(format!("> {}", self.display_input_with_cursor()));
        lines.push(self.input_hint());
        lines.join("\n")
    }

    fn transport_body(&self) -> String {
        let transport = self.session.transport_snapshot();
        let mut lines = vec![
            format!("Status: {}", format_transport_status(&transport)),
            format!("Tempo: {} BPM", format_tempo_bpm(&transport)),
            format!("Cycle: {}", format_cycle_position(&transport)),
            format!(
                "Pattern: {}",
                self.session.last_loaded_pattern_name().unwrap_or("none")
            ),
            "Space: toggle".to_owned(),
            "empty input only".to_owned(),
            "Transport: :play / :stop".to_owned(),
            "Set: :tempo <bpm>".to_owned(),
            "Export: :render <binding> <path> [cycles]".to_owned(),
            "Help: ?".to_owned(),
        ];
        if let Some(message) = &self.status_message {
            lines.push(format!("Note: {message}"));
        }
        lines.push("Quit: Esc or :quit".to_owned());
        lines.join("\n")
    }

    const fn help_overlay_body() -> &'static str {
        "Toggle: ?\nClose: Esc\nTransport: Space toggle, :play, :stop, :tempo <bpm>\nExport: :render <binding> <path> [cycles]\nSession: :quit\nInput: Tab complete, Up/Down history\nCursor: Left/Right, Home/End\nDelete: Backspace, Delete, Ctrl-D\nEdit: Ctrl-A/E/K, Ctrl-U/W, Ctrl-L\nWords: Alt-B/F"
    }

    fn complete_input(&mut self) {
        if !self.input.starts_with(':') {
            return;
        }

        let Some((command, usage)) = matching_command(&self.input) else {
            return;
        };
        self.input = if usage == command {
            command.to_owned()
        } else {
            format!("{command} ")
        };
        self.cursor_index = self.input.len();
        self.history_index = None;
    }

    fn recall_previous_history(&mut self) {
        let Some(next_index) = self.history_index.map_or_else(
            || self.history.len().checked_sub(1),
            |index| index.checked_sub(1),
        ) else {
            return;
        };

        self.history_index = Some(next_index);
        self.input = self.history[next_index].clone();
        self.cursor_index = self.input.len();
    }

    fn recall_next_history(&mut self) {
        let Some(current) = self.history_index else {
            return;
        };

        let next = current + 1;
        if next >= self.history.len() {
            self.history_index = None;
            self.input.clear();
            self.cursor_index = 0;
            return;
        }

        self.history_index = Some(next);
        self.input = self.history[next].clone();
        self.cursor_index = self.input.len();
    }

    fn input_hint(&self) -> String {
        if self.input.is_empty() {
            return "Hint: Tab completes commands. Up/Down recalls history.".to_owned();
        }

        if let Some((_, usage)) = matching_command(&self.input) {
            return format!("Hint: Tab -> {usage}");
        }

        "Hint: Tab completes commands. Up/Down recalls history.".to_owned()
    }

    fn insert_character(&mut self, character: char) {
        self.input.insert(self.cursor_index, character);
        self.cursor_index += character.len_utf8();
        self.history_index = None;
    }

    fn backspace(&mut self) {
        if self.cursor_index == 0 {
            return;
        }

        let previous = previous_char_boundary(&self.input, self.cursor_index);
        self.input.replace_range(previous..self.cursor_index, "");
        self.cursor_index = previous;
        self.history_index = None;
    }

    fn move_cursor_left(&mut self) {
        self.cursor_index = previous_char_boundary(&self.input, self.cursor_index);
    }

    fn move_cursor_right(&mut self) {
        self.cursor_index = next_char_boundary(&self.input, self.cursor_index);
    }

    fn move_cursor_previous_word(&mut self) {
        self.cursor_index = previous_word_boundary(&self.input, self.cursor_index);
    }

    fn move_cursor_next_word(&mut self) {
        self.cursor_index = next_word_boundary(&self.input, self.cursor_index);
    }

    const fn move_cursor_home(&mut self) {
        self.cursor_index = 0;
    }

    fn move_cursor_end(&mut self) {
        self.cursor_index = self.input.len();
    }

    fn delete(&mut self) {
        if self.cursor_index >= self.input.len() {
            return;
        }

        let next = next_char_boundary(&self.input, self.cursor_index);
        self.input.replace_range(self.cursor_index..next, "");
        self.history_index = None;
    }

    fn kill_to_end(&mut self) {
        if self.cursor_index >= self.input.len() {
            return;
        }

        self.input.truncate(self.cursor_index);
        self.history_index = None;
    }

    fn kill_to_start(&mut self) {
        if self.cursor_index == 0 {
            return;
        }

        self.input.replace_range(..self.cursor_index, "");
        self.cursor_index = 0;
        self.history_index = None;
    }

    fn delete_previous_word(&mut self) {
        if self.cursor_index == 0 {
            return;
        }

        let start = previous_word_boundary(&self.input, self.cursor_index);
        self.input.replace_range(start..self.cursor_index, "");
        self.cursor_index = start;
        self.history_index = None;
    }

    fn clear_transcript(&mut self) {
        self.transcript.clear();
        self.set_status_message("transcript cleared");
    }

    fn toggle_help(&mut self) {
        self.show_help = !self.show_help;
        self.set_status_message(if self.show_help {
            "help overlay shown"
        } else {
            "help overlay hidden"
        });
    }

    fn close_help(&mut self) {
        self.show_help = false;
        self.set_status_message("help overlay hidden");
    }

    fn toggle_transport_hotkey(&mut self) {
        let command = if self.session.transport_snapshot().is_playing() {
            ":stop"
        } else {
            ":play"
        };
        let message = self
            .session
            .eval_line(command)
            .unwrap_or_else(|error| error);
        self.set_status_message(message);
    }

    fn set_status_message(&mut self, message: impl Into<String>) {
        self.status_message = Some(message.into());
        self.status_expires_at = Some(Instant::now() + STATUS_TOAST_TTL);
    }

    fn clear_status_message(&mut self) {
        self.status_message = None;
        self.status_expires_at = None;
    }

    fn clear_status_if_expired(&mut self, now: Instant) {
        if self
            .status_expires_at
            .is_some_and(|expires_at| now >= expires_at)
        {
            self.clear_status_message();
        }
    }

    fn display_input_with_cursor(&self) -> String {
        let (left, right) = self.input.split_at(self.cursor_index);
        format!("{left}|{right}")
    }
}

fn matching_command(prefix: &str) -> Option<(&'static str, &'static str)> {
    let mut matches = COMMAND_HINTS
        .into_iter()
        .filter(|(command, _)| command.starts_with(prefix));
    let command_match = matches.next()?;
    if matches.next().is_some() {
        None
    } else {
        Some(command_match)
    }
}

fn previous_char_boundary(input: &str, index: usize) -> usize {
    if index == 0 {
        return 0;
    }

    let mut cursor = index - 1;
    while !input.is_char_boundary(cursor) {
        cursor -= 1;
    }
    cursor
}

fn next_char_boundary(input: &str, index: usize) -> usize {
    if index >= input.len() {
        return input.len();
    }

    let mut cursor = index + 1;
    while cursor < input.len() && !input.is_char_boundary(cursor) {
        cursor += 1;
    }
    cursor
}

fn previous_word_boundary(input: &str, index: usize) -> usize {
    let mut cursor = index;

    while cursor > 0 {
        let previous = previous_char_boundary(input, cursor);
        let Some(character) = input[..cursor].chars().next_back() else {
            break;
        };
        if !character.is_whitespace() {
            break;
        }
        cursor = previous;
    }

    while cursor > 0 {
        let previous = previous_char_boundary(input, cursor);
        let Some(character) = input[..cursor].chars().next_back() else {
            break;
        };
        if character.is_whitespace() {
            break;
        }
        cursor = previous;
    }

    cursor
}

fn next_word_boundary(input: &str, index: usize) -> usize {
    let mut cursor = index;

    while cursor < input.len() {
        let Some(character) = input[cursor..].chars().next() else {
            break;
        };
        if !character.is_whitespace() {
            break;
        }
        cursor = next_char_boundary(input, cursor);
    }

    while cursor < input.len() {
        let Some(character) = input[cursor..].chars().next() else {
            break;
        };
        if character.is_whitespace() {
            break;
        }
        cursor = next_char_boundary(input, cursor);
    }

    cursor
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

fn format_cycle_position(snapshot: &orpheus_dsp::TransportSnapshot) -> String {
    let frames_per_cycle = snapshot.frames_per_cycle();
    if frames_per_cycle == 0 {
        return "0.000".to_owned();
    }

    let cycle_index = snapshot.current_cycle_start_frame() / frames_per_cycle;
    let cycle_offset = snapshot
        .current_frame()
        .saturating_sub(snapshot.current_cycle_start_frame());
    let progress_millis = cycle_offset.saturating_mul(1000) / frames_per_cycle;
    format!("{cycle_index}.{progress_millis:03}")
}

fn format_tempo_bpm(snapshot: &orpheus_dsp::TransportSnapshot) -> String {
    let tempo_bpm = snapshot.tempo_bpm();
    if tempo_bpm.fract().abs() < f32::EPSILON {
        format!("{tempo_bpm:.0}")
    } else {
        format!("{tempo_bpm:.1}")
    }
}

const fn format_transport_status(snapshot: &orpheus_dsp::TransportSnapshot) -> &'static str {
    if snapshot.is_playing() {
        "playing"
    } else {
        "stopped"
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

#[cfg(test)]
mod tests {
    use std::fs;
    use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

    use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
    use orpheus_dsp::EngineHandle;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    use super::{SessionTui, buffer_to_string, handle_key_event, render_session_frame};

    fn press(code: KeyCode) -> KeyEvent {
        KeyEvent {
            code,
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Press,
            state: crossterm::event::KeyEventState::NONE,
        }
    }

    fn ctrl(code: KeyCode) -> KeyEvent {
        KeyEvent {
            code,
            modifiers: KeyModifiers::CONTROL,
            kind: KeyEventKind::Press,
            state: crossterm::event::KeyEventState::NONE,
        }
    }

    fn alt(code: KeyCode) -> KeyEvent {
        KeyEvent {
            code,
            modifiers: KeyModifiers::ALT,
            kind: KeyEventKind::Press,
            state: crossterm::event::KeyEventState::NONE,
        }
    }

    #[test]
    fn tab_completes_render_command_prefix() {
        let mut app = SessionTui::new(EngineHandle::stub());
        app.input = ":ren".to_owned();
        app.cursor_index = app.input.len();

        handle_key_event(&mut app, press(KeyCode::Tab));

        assert_eq!(app.input, ":render ");
    }

    #[test]
    fn up_and_down_walk_input_history() {
        let mut app = SessionTui::new(EngineHandle::stub());
        app.input = "drums = bd sn".to_owned();
        app.submit_line();
        app.input = "song = drums".to_owned();
        app.submit_line();

        handle_key_event(&mut app, press(KeyCode::Up));
        assert_eq!(app.input, "song = drums");

        handle_key_event(&mut app, press(KeyCode::Up));
        assert_eq!(app.input, "drums = bd sn");

        handle_key_event(&mut app, press(KeyCode::Down));
        assert_eq!(app.input, "song = drums");

        handle_key_event(&mut app, press(KeyCode::Down));
        assert!(app.input.is_empty());
    }

    #[test]
    fn repl_body_shows_completion_hint_for_partial_command() {
        let mut app = SessionTui::new(EngineHandle::stub());
        app.input = ":ren".to_owned();
        app.cursor_index = app.input.len();

        assert!(
            app.repl_body()
                .contains("Hint: Tab -> :render <binding> <path> [cycles]")
        );
    }

    #[test]
    fn left_right_and_backspace_edit_at_cursor() {
        let mut app = SessionTui::new(EngineHandle::stub());
        for code in [
            KeyCode::Char('d'),
            KeyCode::Char('r'),
            KeyCode::Char('m'),
            KeyCode::Char('s'),
            KeyCode::Left,
            KeyCode::Left,
            KeyCode::Char('u'),
        ] {
            handle_key_event(&mut app, press(code));
        }

        assert_eq!(app.input, "drums");

        handle_key_event(&mut app, press(KeyCode::Right));
        handle_key_event(&mut app, press(KeyCode::Backspace));

        assert_eq!(app.input, "drus");
    }

    #[test]
    fn repl_body_marks_the_cursor_position_inside_input() {
        let mut app = SessionTui::new(EngineHandle::stub());
        for code in [
            KeyCode::Char('d'),
            KeyCode::Char('r'),
            KeyCode::Char('u'),
            KeyCode::Char('m'),
            KeyCode::Char('s'),
            KeyCode::Left,
            KeyCode::Left,
        ] {
            handle_key_event(&mut app, press(code));
        }

        assert!(app.repl_body().contains("> dru|ms"));
    }

    #[test]
    fn home_and_end_move_cursor_for_insertion() {
        let mut app = SessionTui::new(EngineHandle::stub());
        for code in [
            KeyCode::Char('d'),
            KeyCode::Char('r'),
            KeyCode::Char('u'),
            KeyCode::Char('m'),
            KeyCode::Char('s'),
            KeyCode::Home,
            KeyCode::Char('!'),
            KeyCode::End,
            KeyCode::Char('.'),
        ] {
            handle_key_event(&mut app, press(code));
        }

        assert_eq!(app.input, "!drums.");
    }

    #[test]
    fn delete_removes_character_after_cursor() {
        let mut app = SessionTui::new(EngineHandle::stub());
        for code in [
            KeyCode::Char('d'),
            KeyCode::Char('r'),
            KeyCode::Char('u'),
            KeyCode::Char('m'),
            KeyCode::Char('s'),
            KeyCode::Left,
            KeyCode::Left,
            KeyCode::Delete,
        ] {
            handle_key_event(&mut app, press(code));
        }

        assert_eq!(app.input, "drus");
    }

    #[test]
    fn ctrl_a_and_ctrl_e_move_cursor_to_line_edges() {
        let mut app = SessionTui::new(EngineHandle::stub());
        for code in [
            KeyCode::Char('d'),
            KeyCode::Char('r'),
            KeyCode::Char('u'),
            KeyCode::Char('m'),
            KeyCode::Char('s'),
        ] {
            handle_key_event(&mut app, press(code));
        }

        handle_key_event(&mut app, ctrl(KeyCode::Char('a')));
        handle_key_event(&mut app, press(KeyCode::Char('!')));
        handle_key_event(&mut app, ctrl(KeyCode::Char('e')));
        handle_key_event(&mut app, press(KeyCode::Char('.')));

        assert_eq!(app.input, "!drums.");
    }

    #[test]
    fn ctrl_k_deletes_from_cursor_to_line_end() {
        let mut app = SessionTui::new(EngineHandle::stub());
        for code in [
            KeyCode::Char('d'),
            KeyCode::Char('r'),
            KeyCode::Char('u'),
            KeyCode::Char('m'),
            KeyCode::Char('s'),
            KeyCode::Left,
            KeyCode::Left,
        ] {
            handle_key_event(&mut app, press(code));
        }

        handle_key_event(&mut app, ctrl(KeyCode::Char('k')));

        assert_eq!(app.input, "dru");
    }

    #[test]
    fn ctrl_u_deletes_from_cursor_back_to_start() {
        let mut app = SessionTui::new(EngineHandle::stub());
        for code in [
            KeyCode::Char('d'),
            KeyCode::Char('r'),
            KeyCode::Char('u'),
            KeyCode::Char('m'),
            KeyCode::Char('s'),
            KeyCode::Left,
            KeyCode::Left,
        ] {
            handle_key_event(&mut app, press(code));
        }

        handle_key_event(&mut app, ctrl(KeyCode::Char('u')));

        assert_eq!(app.input, "ms");
        assert_eq!(app.cursor_index, 0);
    }

    #[test]
    fn ctrl_w_skips_trailing_whitespace_and_deletes_previous_word() {
        let mut app = SessionTui::new(EngineHandle::stub());
        for code in [
            KeyCode::Char('b'),
            KeyCode::Char('d'),
            KeyCode::Char(' '),
            KeyCode::Char('s'),
            KeyCode::Char('n'),
            KeyCode::Char(' '),
            KeyCode::Char(' '),
            KeyCode::Char(' '),
        ] {
            handle_key_event(&mut app, press(code));
        }

        handle_key_event(&mut app, ctrl(KeyCode::Char('w')));

        assert_eq!(app.input, "bd ");
        assert_eq!(app.cursor_index, app.input.len());
    }

    #[test]
    fn ctrl_d_deletes_character_after_cursor() {
        let mut app = SessionTui::new(EngineHandle::stub());
        for code in [
            KeyCode::Char('d'),
            KeyCode::Char('r'),
            KeyCode::Char('u'),
            KeyCode::Char('m'),
            KeyCode::Char('s'),
            KeyCode::Left,
            KeyCode::Left,
        ] {
            handle_key_event(&mut app, press(code));
        }

        handle_key_event(&mut app, ctrl(KeyCode::Char('d')));

        assert_eq!(app.input, "drus");
    }

    #[test]
    fn alt_b_moves_cursor_to_start_of_previous_word() {
        let mut app = SessionTui::new(EngineHandle::stub());
        for code in [
            KeyCode::Char('b'),
            KeyCode::Char('d'),
            KeyCode::Char(' '),
            KeyCode::Char('s'),
            KeyCode::Char('n'),
            KeyCode::Char(' '),
            KeyCode::Char('c'),
            KeyCode::Char('p'),
        ] {
            handle_key_event(&mut app, press(code));
        }

        handle_key_event(&mut app, alt(KeyCode::Char('b')));
        handle_key_event(&mut app, alt(KeyCode::Char('b')));
        handle_key_event(&mut app, press(KeyCode::Char('!')));

        assert_eq!(app.input, "bd !sn cp");
        assert_eq!(app.cursor_index, 4);
    }

    #[test]
    fn alt_f_moves_cursor_to_end_of_next_word() {
        let mut app = SessionTui::new(EngineHandle::stub());
        for code in [
            KeyCode::Char('b'),
            KeyCode::Char('d'),
            KeyCode::Char(' '),
            KeyCode::Char('s'),
            KeyCode::Char('n'),
            KeyCode::Char(' '),
            KeyCode::Char('c'),
            KeyCode::Char('p'),
        ] {
            handle_key_event(&mut app, press(code));
        }

        handle_key_event(&mut app, press(KeyCode::Home));
        handle_key_event(&mut app, alt(KeyCode::Char('f')));
        handle_key_event(&mut app, press(KeyCode::Char('!')));

        assert_eq!(app.input, "bd! sn cp");
        assert_eq!(app.cursor_index, 3);
    }

    #[test]
    fn ctrl_l_clears_transcript_but_keeps_bindings() {
        let mut app = SessionTui::new(EngineHandle::stub());
        app.input = "drums = bd sn".to_owned();
        app.submit_line();
        app.input = "song = drums".to_owned();
        app.submit_line();
        let bindings_before = app.session.binding_summaries();
        app.input = "warp".to_owned();
        app.cursor_index = app.input.len();

        handle_key_event(&mut app, ctrl(KeyCode::Char('l')));

        let repl_body = app.repl_body();
        assert!(!repl_body.contains("> drums = bd sn"));
        assert!(!repl_body.contains("> song = drums"));
        assert!(repl_body.contains("> warp|"));
        assert_eq!(app.session.binding_summaries(), bindings_before);
    }

    #[test]
    fn space_toggles_transport_when_input_is_empty() {
        let mut app = SessionTui::new(EngineHandle::stub());
        app.input = "drums = bd sn".to_owned();
        app.submit_line();
        let _ = app.session.render_test_block_for_tui(256);

        handle_key_event(&mut app, press(KeyCode::Char(' ')));
        let _ = app.session.render_test_block_for_tui(1);
        assert_eq!(app.status_message.as_deref(), Some("transport stopped"));
        assert!(!app.session.transport_snapshot().is_playing());

        handle_key_event(&mut app, press(KeyCode::Char(' ')));
        let _ = app.session.render_test_block_for_tui(1);
        assert_eq!(app.status_message.as_deref(), Some("transport playing"));
        assert!(app.session.transport_snapshot().is_playing());
    }

    #[test]
    fn space_still_inserts_a_character_while_editing_input() {
        let mut app = SessionTui::new(EngineHandle::stub());
        app.input = "bd".to_owned();
        app.cursor_index = app.input.len();

        handle_key_event(&mut app, press(KeyCode::Char(' ')));

        assert_eq!(app.input, "bd ");
        assert_eq!(app.cursor_index, app.input.len());
        assert!(app.session.transport_snapshot().is_playing());
    }

    #[test]
    fn question_mark_toggles_help_overlay_and_escape_closes_it_first() {
        let mut app = SessionTui::new(EngineHandle::stub());

        handle_key_event(&mut app, press(KeyCode::Char('?')));
        assert_eq!(app.status_message.as_deref(), Some("help overlay shown"));
        let overlay_frame = render_frame_for_test(&app, 80, 24);
        assert!(overlay_frame.contains("Help"));
        assert!(overlay_frame.contains("Space"));
        assert!(overlay_frame.contains(":play"));
        assert!(overlay_frame.contains(":stop"));
        assert!(overlay_frame.contains(":tempo <bpm>"));
        assert!(overlay_frame.contains(":render <binding>"));
        assert!(overlay_frame.contains("Ctrl-A/E/K"));
        assert!(overlay_frame.contains("Alt-B/F"));
        assert!(!app.should_quit);

        handle_key_event(&mut app, press(KeyCode::Esc));

        assert!(!app.should_quit);
        assert_eq!(app.status_message.as_deref(), Some("help overlay hidden"));
        let normal_frame = render_frame_for_test(&app, 80, 24);
        assert!(!normal_frame.contains("Toggle: ?"));
        assert!(!normal_frame.contains("Words: Alt-B/F"));
        assert!(normal_frame.contains("Help: ?"));
    }

    #[test]
    fn render_command_uses_status_toast_instead_of_transcript() {
        let mut app = SessionTui::new(EngineHandle::stub());
        app.input = "drums = bd sn".to_owned();
        app.submit_line();
        let transcript_before = app.transcript.clone();
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_else(|error| panic!("system clock should be after unix epoch: {error}"))
            .as_nanos();
        let output_path = std::env::temp_dir().join(format!("orpheus-tui-toast-{unique}.wav"));
        let expected_message = format!(
            "rendered `drums` to `{}` (1 cycle(s))",
            output_path.display()
        );
        app.input = format!(":render drums {}", output_path.display());

        app.submit_line();

        assert_eq!(app.transcript, transcript_before);
        assert_eq!(
            app.status_message.as_deref(),
            Some(expected_message.as_str())
        );
        assert!(output_path.exists());
        let _ = fs::remove_file(output_path);
    }

    #[test]
    fn status_toast_expires_after_ttl() {
        let mut app = SessionTui::new(EngineHandle::stub());
        app.toggle_help();
        assert_eq!(app.status_message.as_deref(), Some("help overlay shown"));

        app.status_expires_at = Some(
            Instant::now()
                .checked_sub(Duration::from_millis(1))
                .unwrap_or_else(|| panic!("subtracting one millisecond from now should succeed")),
        );
        app.clear_status_if_expired(Instant::now());

        assert!(app.status_message.is_none());
        assert!(app.status_expires_at.is_none());
        let frame = render_frame_for_test(&app, 80, 24);
        assert!(!frame.contains("Note:"));
    }

    #[test]
    fn transport_reports_cycle_progress_and_last_loaded_pattern() {
        let mut app = SessionTui::new(EngineHandle::stub());
        app.input = "drums = bd sn".to_owned();
        app.submit_line();
        let frames_per_cycle = app.session.transport_snapshot().frames_per_cycle();
        let _ = app.session.render_test_block_for_tui(frames_per_cycle / 2);

        let frame = render_frame_for_test(&app, 80, 24);
        assert!(frame.contains("Pattern: drums"));
        assert!(frame.contains("Cycle: 0.500"));
    }

    #[test]
    fn transport_reports_live_engine_tempo() {
        let mut app = SessionTui::new(EngineHandle::stub());
        app.input = ":tempo 90".to_owned();
        app.submit_line();
        let _ = app.session.render_test_block_for_tui(1);

        let frame = render_frame_for_test(&app, 80, 24);
        assert!(frame.contains("Tempo: 90 BPM"));
    }

    #[test]
    fn transport_reports_stopped_state_after_stop_command() {
        let mut app = SessionTui::new(EngineHandle::stub());
        app.input = "drums = bd sn".to_owned();
        app.submit_line();
        let _ = app.session.render_test_block_for_tui(256);
        app.input = ":stop".to_owned();
        app.submit_line();
        let _ = app.session.render_test_block_for_tui(1);

        let frame = render_frame_for_test(&app, 80, 24);
        assert!(frame.contains("Status: stopped"));
        assert!(frame.contains("Space"));
        assert!(frame.contains("empty input"));
        assert!(frame.contains(":play"));
        assert!(frame.contains(":stop"));
    }

    fn render_frame_for_test(app: &SessionTui, width: u16, height: u16) -> String {
        let backend = TestBackend::new(width, height);
        let mut terminal = Terminal::new(backend)
            .unwrap_or_else(|error| panic!("test backend should create a terminal: {error}"));
        terminal
            .draw(|frame| render_session_frame(frame, app))
            .unwrap_or_else(|error| panic!("test backend should render one frame: {error}"));
        buffer_to_string(terminal.backend().buffer())
    }
}
