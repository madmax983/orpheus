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
const COMMAND_HINTS: [(&str, &str); 2] = [
    (":quit", ":quit"),
    (":render", ":render <binding> <path> [cycles]"),
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
        KeyCode::Char('a') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            app.move_cursor_home();
        }
        KeyCode::Char('e') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            app.move_cursor_end();
        }
        KeyCode::Char('k') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            app.kill_to_end();
        }
        KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            app.kill_to_start();
        }
        KeyCode::Char('w') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            app.delete_previous_word();
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
    history: Vec<String>,
    history_index: Option<usize>,
    input: String,
    cursor_index: usize,
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
            input: String::new(),
            cursor_index: 0,
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
        lines.push(format!("> {}", self.display_input_with_cursor()));
        lines.push(self.input_hint());
        lines.join("\n")
    }

    fn transport_body() -> String {
        format!(
            "Status: live shell\nTempo: {DEFAULT_TEMPO_BPM} BPM\nAudio: cycle-locked\nExport: :render <binding> <path> [cycles]\nInput: Tab=complete, Up/Down=history, Left/Right=move, Home/End/Delete, Ctrl-A/E/K\nBackkill: Ctrl-U/W\nQuit: Esc or :quit"
        )
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
    use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
    use orpheus_dsp::EngineHandle;

    use super::{SessionTui, handle_key_event};

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
            KeyCode::Char('?'),
        ] {
            handle_key_event(&mut app, press(code));
        }

        assert_eq!(app.input, "!drums?");
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
        handle_key_event(&mut app, press(KeyCode::Char('?')));

        assert_eq!(app.input, "!drums?");
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
}
