//! The `tui` module implements the advanced terminal user interface.
//!
//! This module provides the full-screen interactive live-coding environment using
//! `ratatui` and `crossterm`. It visualizes the current evaluated bindings,
//! the active audio transport state, and provides real-time feedback for errors
//! and evaluation events.

use std::cell::Cell;
use std::io;
use std::path::Path;
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
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Borders, Clear, List, ListItem, Paragraph, Wrap};
use ratatui::{Frame, Terminal};

use crate::session::{MixerView, ReplSession, TransportView};

const EVENT_POLL_INTERVAL: Duration = Duration::from_millis(50);
const STATUS_TOAST_TTL: Duration = Duration::from_secs(3);
const MIN_BINDING_LEGEND_ROWS: usize = 6;
const FULL_KEY_LEGEND: &str = "? help   Space toggle(empty)   PgUp/PgDn bindings";
const MEDIUM_KEY_LEGEND: &str = "? help   Space   PgUp/PgDn";
const COMPACT_KEY_LEGEND: &str = "? Space Pg";
const FULL_HELP_FOOTER: &str = "Esc close   ? toggle   Ctrl-C quit";
const MEDIUM_HELP_FOOTER: &str = "Esc close   ?   Ctrl-C";
const COMPACT_HELP_FOOTER: &str = "Esc ? Ctrl-C";
const MIN_HELP_FOOTER: &str = "Esc ?";
const COMMAND_HINTS: [(&str, &str); 14] = [
    (":bus", ":bus <new|fx> ..."),
    (":explain", ":explain <binding>"),
    (
        ":export",
        ":export <binding> <path> [cycles] | :export stems [cycles] [--buses]",
    ),
    (":mixer", ":mixer"),
    (":open", ":open <path>"),
    (":play", ":play"),
    (":quit", ":quit"),
    (":render", ":render <binding> <path> [cycles]"),
    (":roll", ":roll <binding> [cycles] [steps_per_cycle]"),
    (":send", ":send <track> <bus> <level>"),
    (":stats", ":stats <binding> [cycles]"),
    (":stop", ":stop"),
    (":tempo", ":tempo <bpm>"),
    (":track", ":track <new|bind|level|mute> ..."),
];

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
    let mut app = SessionTui::try_new(engine, startup_path, warning);
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

    if app.show_help {
        match key.code {
            KeyCode::Esc => app.close_help(),
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                app.should_quit = true;
            }
            KeyCode::Char('?') if key.modifiers.is_empty() => app.toggle_help(),
            _ => {}
        }
        return;
    }

    match key.code {
        KeyCode::Esc => app.should_quit = true,
        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            app.should_quit = true;
        }
        KeyCode::Char('?') => app.toggle_help(),
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
        KeyCode::PageUp => app.scroll_bindings_up(),
        KeyCode::PageDown => app.scroll_bindings_down(),
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
    let [body, footer] = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(1)])
        .areas(frame.area());
    let [left, right] = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(72), Constraint::Percentage(28)])
        .areas(body);
    let [bindings, repl] = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(58), Constraint::Percentage(42)])
        .areas(left);

    app.record_bindings_height(bindings.height);
    let (bindings_title, binding_items) = app.binding_pane(bindings.height);
    frame.render_widget(
        List::new(binding_items)
            .block(Block::default().title(bindings_title).borders(Borders::ALL)),
        bindings,
    );

    frame.render_widget(
        Paragraph::new(app.repl_text())
            .block(Block::default().title("REPL").borders(Borders::ALL))
            .wrap(Wrap { trim: false }),
        repl,
    );

    frame.render_widget(
        Paragraph::new(app.transport_text())
            .block(Block::default().title("Transport").borders(Borders::ALL))
            .wrap(Wrap { trim: false }),
        right,
    );

    if app.show_help {
        render_help_overlay(frame);
    }

    frame.render_widget(
        Paragraph::new(app.frame_footer_line(footer.width))
            .style(frame_footer_style(app.show_help))
            .wrap(Wrap { trim: false }),
        footer,
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
    status_message: Option<String>,
    status_expires_at: Option<Instant>,
    input: String,
    cursor_index: usize,
    bindings_scroll: usize,
    bindings_last_height: Cell<u16>,
    show_help: bool,
    should_quit: bool,
}

impl SessionTui {
    fn new(engine: EngineHandle) -> Self {
        Self::try_new(engine, None, None)
    }

    fn try_new(engine: EngineHandle, startup_path: Option<&Path>, warning: Option<String>) -> Self {
        let mut transcript = vec![
            "Interactive shell ready.".to_owned(),
            "Press Esc to quit.".to_owned(),
        ];
        if let Some(msg) = warning {
            transcript.push(format!("⚠️ {msg}"));
        }

        let mut app = Self {
            session: ReplSession::with_engine(engine),
            transcript,
            history: Vec::new(),
            history_index: None,
            status_message: None,
            status_expires_at: None,
            input: String::new(),
            cursor_index: 0,
            bindings_scroll: 0,
            bindings_last_height: Cell::new(0),
            show_help: false,
            should_quit: false,
        };
        if let Some(path) = startup_path {
            match app.session.open_file(path) {
                Ok(message) => app.transcript.push(format!("✓ {message}")),
                Err(message) => app.transcript.push(format!("✗ {message}")),
            }
        }
        app
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
            Ok(message) => self.transcript.push(format!("✓ {message}")),
            Err(message) => self.transcript.push(format!("✗ {message}")),
        }
    }

    fn binding_pane(&self, bindings_height: u16) -> (String, Vec<ListItem<'static>>) {
        let items = self.binding_lines(bindings_height);
        let viewport_rows = binding_viewport_rows(bindings_height);
        if items.len() <= viewport_rows {
            return ("Bindings".to_owned(), items);
        }

        let max_scroll = items.len().saturating_sub(viewport_rows);
        let offset = self.bindings_scroll.min(max_scroll);
        let start = offset + 1;
        let end = (offset + viewport_rows).min(items.len());
        (
            format!("Bindings {start}-{end}/{} PgUp/PgDn", items.len()),
            items.into_iter().skip(offset).take(viewport_rows).collect(),
        )
    }

    fn binding_lines(&self, bindings_height: u16) -> Vec<ListItem<'static>> {
        let transport = self.session.transport_view();
        let bindings = self.session.binding_summaries();
        if bindings.is_empty() {
            vec![ListItem::new("No bindings yet")]
        } else {
            let mut items = bindings
                .into_iter()
                .map(|summary| binding_list_item(summary, &transport))
                .collect::<Vec<_>>();
            if should_show_binding_legend(bindings_height, items.len(), &transport) {
                items.push(ListItem::new(""));
                items.push(binding_legend_item(&transport));
            }
            items
        }
    }

    fn record_bindings_height(&self, bindings_height: u16) {
        self.bindings_last_height.set(bindings_height);
    }

    #[cfg(test)]
    fn repl_body(&self) -> String {
        let mut lines = self.transcript.clone();
        let transport = self.session.transport_view();
        let mut transport_line = format!("Transport: {}", format_transport_status(&transport));
        if let Some(pending_pattern_name) = transport.pending_pattern_name() {
            transport_line.push_str(" -> ");
            transport_line.push_str(pending_pattern_name);
        }
        lines.push(transport_line);
        lines.push(format!("> {}", self.display_input_with_cursor()));
        lines.push(self.input_hint());
        lines.join("\n")
    }

    fn repl_text(&self) -> Text<'static> {
        let mut lines = self
            .transcript
            .iter()
            .flat_map(|entry| {
                let style = if entry.starts_with("> ") {
                    Style::default().fg(Color::DarkGray)
                } else if entry.starts_with("✗ ") {
                    Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)
                } else if entry.starts_with("⚠️ ") {
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD)
                } else if entry.starts_with("✓ ") {
                    Style::default().fg(Color::Green)
                } else {
                    Style::default()
                };

                entry
                    .split('\n')
                    .map(move |line| Line::styled(line.to_owned(), style))
            })
            .collect::<Vec<_>>();
        let transport = self.session.transport_view();
        lines.push(transport_status_line("Transport: ", &transport, true));
        lines.push(Line::raw(format!("> {}", self.display_input_with_cursor())));
        lines.push(Line::styled(
            self.input_hint(),
            Style::default().fg(Color::DarkGray),
        ));
        Text::from(lines)
    }

    fn transport_text(&self) -> Text<'static> {
        let transport = self.session.transport_view();
        let mixer = self.session.mixer_view();
        let mut lines = vec![Line::raw(format!(
            "Pattern: {}",
            transport.active_pattern_name().unwrap_or("none")
        ))];
        if let Some(pending_pattern_name) = transport.pending_pattern_name() {
            lines.push(Line::raw(format!("Next: {pending_pattern_name}")));
        }
        lines.push(routing_status_line(&mixer));
        if !mixer.summary().is_empty() && mixer.summary() != "mixer is empty" {
            lines.push(Line::raw("Mixer:"));
            for summary_line in mixer.summary().lines() {
                lines.push(Line::raw(summary_line.to_owned()));
            }
        }
        lines.extend([
            Line::raw("Space: toggle"),
            Line::raw("empty input only"),
            Line::raw("Open: :open <path>"),
            Line::raw("Transport: :play / :stop"),
            Line::raw("Mixer: :track / :bus new|fx / :send / :mixer"),
            Line::raw("Set: :tempo <bpm>"),
            Line::raw("Render: :render <binding> <path> [cycles]"),
            Line::raw(
                "Export: :export <binding> <path> [cycles] | :export stems [cycles] [--buses]",
            ),
            Line::raw("Analyze: :roll <binding>, :stats <binding>, :explain <binding>"),
            Line::raw("Help: ?"),
        ]);
        if let Some(message) = &self.status_message {
            if message.contains("error")
                || message.contains("failed")
                || message.contains("unknown")
                || message.contains("usage:")
            {
                lines.push(Line::styled(
                    format!("Note: ✗ {message}"),
                    Style::default()
                        .fg(Color::LightRed)
                        .add_modifier(Modifier::BOLD),
                ));
            } else {
                lines.push(Line::styled(
                    format!("Note: ✓ {message}"),
                    Style::default().fg(Color::LightGreen),
                ));
            }
        }
        lines.push(Line::raw("Quit: Esc or :quit"));
        Text::from(lines)
    }

    const fn help_overlay_body() -> &'static str {
        "Toggle: ?\nClose: Esc\nTransport: Space toggle, :play, :stop, :tempo <bpm>\nMixer: :track, :bus new|fx, :send, :mixer\nRender: :render <binding> <path> [cycles]\nExport: :export <binding> <path> [cycles] | :export stems [cycles] [--buses]\nAnalyze: :roll <binding> [cycles] [steps_per_cycle], :stats <binding> [cycles], :explain <binding>\nSession: :open <path>, :quit\nBindings: PgUp/PgDn\nInput: Tab complete, Up/Down history\nCursor: Left/Right, Home/End\nDelete: Backspace, Delete, Ctrl-D\nEdit: Ctrl-A/E/K, Ctrl-U/W, Ctrl-L\nWords: Alt-B/F"
    }

    const fn help_overlay_footer() -> &'static str {
        FULL_HELP_FOOTER
    }

    fn frame_footer_line(&self, width: u16) -> Line<'static> {
        if self.show_help {
            help_footer_line(width)
        } else {
            normal_footer_line(width, &self.session.transport_view())
        }
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

    fn scroll_bindings_up(&mut self) {
        self.bindings_scroll = self
            .bindings_scroll
            .saturating_sub(self.binding_scroll_step());
    }

    fn scroll_bindings_down(&mut self) {
        self.bindings_scroll = self
            .bindings_scroll
            .saturating_add(self.binding_scroll_step())
            .min(self.max_binding_scroll());
    }

    fn binding_scroll_step(&self) -> usize {
        binding_viewport_rows(self.bindings_last_height.get())
            .saturating_sub(1)
            .max(1)
    }

    fn max_binding_scroll(&self) -> usize {
        let bindings_height = self.bindings_last_height.get();
        self.binding_lines(bindings_height)
            .len()
            .saturating_sub(binding_viewport_rows(bindings_height))
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

fn render_help_overlay(frame: &mut Frame<'_>) {
    let overlay_area = centered_rect(frame.area(), 68, 60);
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
        Paragraph::new(SessionTui::help_overlay_body())
            .style(Style::default().bg(Color::Black))
            .wrap(Wrap { trim: false }),
        body_area,
    );
    frame.render_widget(
        Paragraph::new(SessionTui::help_overlay_footer())
            .style(help_overlay_footer_style())
            .wrap(Wrap { trim: false }),
        footer_area,
    );
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

fn help_overlay_border_style() -> Style {
    Style::default()
        .fg(Color::Cyan)
        .add_modifier(Modifier::BOLD)
}

fn help_overlay_footer_style() -> Style {
    Style::default().fg(Color::Gray).add_modifier(Modifier::DIM)
}

fn key_legend_style() -> Style {
    Style::default()
        .fg(Color::DarkGray)
        .add_modifier(Modifier::DIM)
}

fn frame_footer_style(show_help: bool) -> Style {
    if show_help {
        help_overlay_footer_style().bg(Color::DarkGray)
    } else {
        key_legend_style()
    }
}

#[derive(Clone, Copy)]
struct FooterCandidate<'a> {
    legend: Option<&'a str>,
    tempo: Option<&'a str>,
    cycle: Option<&'a str>,
}

impl<'a> FooterCandidate<'a> {
    const fn new(legend: Option<&'a str>, tempo: Option<&'a str>, cycle: Option<&'a str>) -> Self {
        Self {
            legend,
            tempo,
            cycle,
        }
    }

    const fn width(&self, state: &str) -> usize {
        let mut width = 0;
        if let Some(legend) = self.legend {
            width += legend.len();
        }
        width = footer_width_with_optional_segment(width, state.len());
        if let Some(tempo) = self.tempo {
            width = footer_width_with_optional_segment(width, tempo.len());
        }
        if let Some(cycle) = self.cycle {
            width = footer_width_with_optional_segment(width, cycle.len());
        }
        width
    }

    fn render(self, state: &str, transport: &TransportView) -> Line<'static> {
        let mut spans = Vec::new();
        if let Some(legend) = self.legend {
            spans.push(Span::styled(legend.to_owned(), key_legend_style()));
        }
        push_footer_segment(
            &mut spans,
            Span::styled(state.to_owned(), transport_status_style(transport)),
        );
        if let Some(tempo) = self.tempo {
            push_footer_segment(
                &mut spans,
                Span::styled(tempo.to_owned(), key_legend_style()),
            );
        }
        if let Some(cycle) = self.cycle {
            push_footer_segment(
                &mut spans,
                Span::styled(cycle.to_owned(), key_legend_style()),
            );
        }
        Line::from(spans)
    }
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

fn normal_footer_line(width: u16, transport: &TransportView) -> Line<'static> {
    let state = format_transport_status(transport).to_owned();
    let tempo_full = format!("{} BPM", format_tempo_bpm(transport.snapshot()));
    let tempo_compact = format_tempo_bpm(transport.snapshot());
    let cycle = format_cycle_position(transport.snapshot());
    let width = usize::from(width);
    let candidates = [
        FooterCandidate::new(
            Some(FULL_KEY_LEGEND),
            Some(tempo_full.as_str()),
            Some(cycle.as_str()),
        ),
        FooterCandidate::new(
            Some(MEDIUM_KEY_LEGEND),
            Some(tempo_full.as_str()),
            Some(cycle.as_str()),
        ),
        FooterCandidate::new(
            Some(COMPACT_KEY_LEGEND),
            Some(tempo_full.as_str()),
            Some(cycle.as_str()),
        ),
        FooterCandidate::new(None, Some(tempo_full.as_str()), Some(cycle.as_str())),
        FooterCandidate::new(None, Some(tempo_compact.as_str()), Some(cycle.as_str())),
        FooterCandidate::new(None, None, Some(cycle.as_str())),
        FooterCandidate::new(None, None, None),
    ];

    for candidate in candidates {
        if candidate.width(&state) <= width {
            return candidate.render(&state, transport);
        }
    }

    Line::styled(state, transport_status_style(transport))
}

const fn footer_width_with_optional_segment(existing_width: usize, segment_width: usize) -> usize {
    if existing_width == 0 {
        segment_width
    } else {
        existing_width + 3 + segment_width
    }
}

fn push_footer_segment(spans: &mut Vec<Span<'static>>, span: Span<'static>) {
    if !spans.is_empty() {
        spans.push(Span::styled(" | ", key_legend_style()));
    }
    spans.push(span);
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum UiTransportState {
    Playing,
    Stopped,
    Syncing,
    Queued,
}

fn transport_state(view: &TransportView) -> UiTransportState {
    if view.pending_pattern_name().is_some() {
        if view.snapshot().has_pending_pattern() && view.snapshot().is_playing() {
            UiTransportState::Syncing
        } else {
            UiTransportState::Queued
        }
    } else if view.snapshot().is_playing() {
        UiTransportState::Playing
    } else {
        UiTransportState::Stopped
    }
}

fn format_transport_status(view: &TransportView) -> &'static str {
    match transport_state(view) {
        UiTransportState::Playing => "playing",
        UiTransportState::Stopped => "stopped",
        UiTransportState::Syncing => "syncing",
        UiTransportState::Queued => "queued",
    }
}

fn transport_status_style(view: &TransportView) -> Style {
    let color = match transport_state(view) {
        UiTransportState::Playing => Color::Green,
        UiTransportState::Stopped => Color::Yellow,
        UiTransportState::Syncing => Color::Cyan,
        UiTransportState::Queued => Color::Blue,
    };
    Style::default().fg(color).add_modifier(Modifier::BOLD)
}

fn transport_status_line(
    prefix: &'static str,
    view: &TransportView,
    include_target: bool,
) -> Line<'static> {
    let mut spans = vec![
        Span::raw(prefix),
        Span::styled(format_transport_status(view), transport_status_style(view)),
    ];
    if include_target {
        if let Some(pending_pattern_name) = view.pending_pattern_name() {
            spans.push(Span::raw(" -> "));
            spans.push(Span::raw(pending_pattern_name.to_owned()));
        }
    }
    Line::from(spans)
}

fn routing_status_line(mixer: &MixerView) -> Line<'static> {
    let status = if mixer.has_pending_routing() {
        Span::styled(
            "pending",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )
    } else {
        Span::styled("live", Style::default().fg(Color::Green))
    };
    Line::from(vec![Span::raw("Routing: "), status])
}

fn binding_list_item(summary: String, transport: &TransportView) -> ListItem<'static> {
    let name = summary
        .split_once(": ")
        .map_or(summary.as_str(), |(name, _)| name);
    if transport.active_pattern_name() == Some(name) {
        return ListItem::new(Line::from(vec![
            Span::styled("[live] ", live_binding_style()),
            Span::raw(summary),
        ]));
    }
    if transport.pending_pattern_name() == Some(name) {
        return ListItem::new(Line::from(vec![
            Span::styled("[next] ", pending_binding_style(transport)),
            Span::raw(summary),
        ]));
    }
    ListItem::new(summary)
}

fn live_binding_style() -> Style {
    Style::default()
        .fg(Color::Green)
        .add_modifier(Modifier::BOLD)
}

fn pending_binding_style(transport: &TransportView) -> Style {
    let color = match transport_state(transport) {
        UiTransportState::Queued => Color::Blue,
        UiTransportState::Playing | UiTransportState::Stopped | UiTransportState::Syncing => {
            Color::Cyan
        }
    };
    Style::default().fg(color).add_modifier(Modifier::BOLD)
}

fn binding_legend_item(transport: &TransportView) -> ListItem<'static> {
    ListItem::new(Line::from(vec![
        Span::raw("Legend: "),
        Span::styled("[live]", live_binding_style()),
        Span::raw(" active  "),
        Span::styled("[next]", pending_binding_style(transport)),
        Span::raw(" pending"),
    ]))
}

fn should_show_binding_legend(
    bindings_height: u16,
    binding_count: usize,
    transport: &TransportView,
) -> bool {
    if transport.active_pattern_name().is_none() && transport.pending_pattern_name().is_none() {
        return false;
    }

    let visible_rows = usize::from(bindings_height.saturating_sub(2));
    visible_rows >= MIN_BINDING_LEGEND_ROWS && visible_rows >= binding_count.saturating_add(2)
}

fn binding_viewport_rows(bindings_height: u16) -> usize {
    usize::from(bindings_height.saturating_sub(2)).max(1)
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
    use std::path::PathBuf;
    use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

    use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
    use orpheus_dsp::EngineHandle;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use ratatui::layout::Rect;
    use ratatui::style::{Color, Modifier};

    use super::{
        SessionTui, buffer_to_string, centered_rect, handle_key_event, render_session_frame,
    };

    fn fixture(name: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join("tests")
            .join("fixtures")
            .join(name)
    }

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
    fn repl_transport_state_uses_distinct_styles() {
        let mut app = SessionTui::new(EngineHandle::stub());

        let playing_buffer = render_buffer_for_test(&app, 80, 24);
        let (playing_line_x, playing_y) =
            find_text_in_buffer(&playing_buffer, "Transport: playing")
                .unwrap_or_else(|| panic!("rendered repl should contain playing status"));
        let playing_x = playing_line_x
            + u16::try_from("Transport: ".len()).unwrap_or_else(|error| {
                panic!("transport prefix length should fit in u16: {error}")
            });
        let playing_cell = &playing_buffer[(playing_x, playing_y)];
        assert_eq!(playing_cell.fg, Color::Green);
        assert!(playing_cell.modifier.contains(Modifier::BOLD));

        handle_key_event(&mut app, press(KeyCode::Char(' ')));
        let _ = app.session.render_test_block_for_tui(1);

        let stopped_buffer = render_buffer_for_test(&app, 80, 24);
        let (stopped_line_x, stopped_y) =
            find_text_in_buffer(&stopped_buffer, "Transport: stopped")
                .unwrap_or_else(|| panic!("rendered repl should contain stopped status"));
        let stopped_x = stopped_line_x
            + u16::try_from("Transport: ".len()).unwrap_or_else(|error| {
                panic!("transport prefix length should fit in u16: {error}")
            });
        let stopped_cell = &stopped_buffer[(stopped_x, stopped_y)];
        assert_eq!(stopped_cell.fg, Color::Yellow);
        assert!(stopped_cell.modifier.contains(Modifier::BOLD));
    }

    #[test]
    fn startup_file_preloads_bindings_and_transport_target() {
        let song = fixture("song.ode");
        let app = SessionTui::try_new(EngineHandle::stub(), Some(song.as_path()), None);

        assert_eq!(
            app.session.binding_summaries(),
            vec![
                "drums: Pattern<Sample>".to_owned(),
                "song: Pattern<Sample>".to_owned()
            ]
        );
        assert_eq!(
            app.session.transport_view().pending_pattern_name(),
            Some("song")
        );

        let frame = render_frame_for_test(&app, 80, 24);
        assert!(frame.contains("song: Pattern<Sample>"));
        assert!(frame.contains("drums: Pattern<Sample>"));
    }

    #[test]
    fn transport_shows_syncing_while_pattern_swap_is_pending() {
        let mut app = SessionTui::new(EngineHandle::stub());
        app.input = "drums = bd sn".to_owned();
        app.submit_line();
        let _ = app.session.render_test_block_for_tui(256);

        app.input = "backbeat = sn cp".to_owned();
        app.submit_line();
        let _ = app.session.render_test_block_for_tui(1);

        let frame = render_frame_for_test(&app, 80, 24);
        assert!(frame.contains("Transport: syncing -> backbeat"));
        assert!(frame.contains("Next: backbeat"));
        assert!(frame.contains("Pattern: drums"));
    }

    #[test]
    fn transport_shows_queued_target_before_engine_observes_pattern_swap() {
        let mut app = SessionTui::new(EngineHandle::stub());
        app.input = "drums = bd sn".to_owned();
        app.submit_line();
        let _ = app.session.render_test_block_for_tui(256);

        app.input = "backbeat = sn cp".to_owned();
        app.submit_line();

        let frame = render_frame_for_test(&app, 80, 24);
        assert!(frame.contains("Transport: queued -> backbeat"));
        assert!(frame.contains("Next: backbeat"));
        assert!(frame.contains("Pattern: drums"));
    }

    #[test]
    fn transport_pane_omits_footer_redundant_metrics() {
        let app = SessionTui::new(EngineHandle::stub());

        let frame = render_frame_for_test(&app, 80, 24);
        assert!(frame.contains("Pattern: none"));
        assert!(!frame.contains("Status: "));
        assert!(!frame.contains("Tempo: "));
        assert!(!frame.contains("Cycle: "));
    }

    #[test]
    fn transport_pane_shows_active_track_assignment_summary() {
        let mut app = SessionTui::new(EngineHandle::stub());
        app.input = "groove = bd sn".to_owned();
        app.submit_line();
        app.input = ":track new drums".to_owned();
        app.submit_line();
        app.input = ":track bind drums groove".to_owned();
        app.submit_line();

        let frame = render_frame_for_test(&app, 80, 24);
        assert!(frame.contains("Mixer:"));
        assert!(frame.contains("drums"));
    }

    #[test]
    fn transport_pane_shows_pending_routing_state() {
        let mut app = SessionTui::new(EngineHandle::stub());
        app.input = "groove = bd sn".to_owned();
        app.submit_line();
        app.input = ":track new drums".to_owned();
        app.submit_line();
        app.input = ":track bind drums groove".to_owned();
        app.submit_line();
        let _ = app.session.render_test_block_for_tui(1);

        let frame = render_frame_for_test(&app, 80, 24);
        assert!(frame.contains("Routing: pending"));
    }

    #[test]
    fn transport_pane_shows_bus_sends_compactly() {
        let mut app = SessionTui::new(EngineHandle::stub());
        app.input = "groove = bd sn".to_owned();
        app.submit_line();
        app.input = ":track new drums".to_owned();
        app.submit_line();
        app.input = ":track bind drums groove".to_owned();
        app.submit_line();
        app.input = ":bus new verb".to_owned();
        app.submit_line();
        app.input = ":send drums verb 0.35".to_owned();
        app.submit_line();

        let frame = render_frame_for_test(&app, 100, 24);
        assert!(frame.contains("drums"));
        assert!(frame.contains("verb @ 0.35"));
        assert!(frame.contains("Mixer Buses"));
    }

    #[test]
    fn transport_pane_shows_hosted_bus_effect_summary() {
        let mut app = SessionTui::new(EngineHandle::stub());
        app.input = ":bus new dub".to_owned();
        app.submit_line();
        app.input = ":bus fx dub delay time=3/16 feedback=0.45 wet=1.0".to_owned();
        app.submit_line();

        let frame = render_frame_for_test(&app, 160, 40);
        assert!(frame.contains("Mixer Buses"));
        assert!(frame.contains("delay(3/16"));
    }

    #[test]
    fn transport_pane_shows_hosted_reverb_bus_effect_summary() {
        let mut app = SessionTui::new(EngineHandle::stub());
        app.input = ":bus new verb".to_owned();
        app.submit_line();
        app.input = ":bus fx verb reverb size=0.75 damp=0.35 wet=1.0".to_owned();
        app.submit_line();

        let frame = render_frame_for_test(&app, 160, 40);
        assert!(frame.contains("Mixer Buses"));
        assert!(frame.contains("reverb(size=0.75"));
        assert!(frame.contains("damp=0.35"));
        assert!(frame.contains("wet=1.00)"));
    }

    #[test]
    fn transport_pane_shows_pending_routing_after_bus_fx_change() {
        let mut app = SessionTui::new(EngineHandle::stub());
        app.input = ":bus new dub".to_owned();
        app.submit_line();
        let _ = app.session.render_test_block_for_tui(1);
        app.input = ":bus fx dub delay time=3/16 feedback=0.45 wet=1.0".to_owned();
        app.submit_line();
        let _ = app.session.render_test_block_for_tui(1);

        let frame = render_frame_for_test(&app, 80, 24);
        assert!(frame.contains("Routing: pending"));
    }

    #[test]
    fn bindings_pane_marks_live_and_next_patterns() {
        let mut app = SessionTui::new(EngineHandle::stub());
        app.input = "drums = bd sn".to_owned();
        app.submit_line();
        let _ = app.session.render_test_block_for_tui(256);
        app.input = "warp = fast(2)".to_owned();
        app.submit_line();
        app.input = "backbeat = sn cp".to_owned();
        app.submit_line();

        let frame = render_frame_for_test(&app, 120, 24);
        assert!(frame.contains("[live] drums: Pattern<Sample>"));
        assert!(frame.contains("warp: Function(Pattern<t1>) -> Pattern<t1>"));
        assert!(frame.contains("[next] backbeat: Pattern<Sample>"));
    }

    #[test]
    fn bindings_pane_promotes_next_pattern_after_cycle_boundary() {
        let mut app = SessionTui::new(EngineHandle::stub());
        app.input = "drums = bd sn".to_owned();
        app.submit_line();
        let _ = app.session.render_test_block_for_tui(256);
        app.input = "backbeat = sn cp".to_owned();
        app.submit_line();
        let _ = app
            .session
            .render_test_block_for_tui(app.session.frames_until_boundary_for_tui());

        let frame = render_frame_for_test(&app, 120, 24);
        assert!(frame.contains("[live] backbeat: Pattern<Sample>"));
        assert!(!frame.contains("[next] backbeat: Pattern<Sample>"));
        assert!(!frame.contains("[live] drums: Pattern<Sample>"));
    }

    #[test]
    fn bindings_pane_shows_marker_legend_when_pattern_markers_are_present() {
        let mut app = SessionTui::new(EngineHandle::stub());
        app.input = "drums = bd sn".to_owned();
        app.submit_line();
        let _ = app.session.render_test_block_for_tui(256);
        app.input = "backbeat = sn cp".to_owned();
        app.submit_line();

        let frame = render_frame_for_test(&app, 120, 24);
        assert!(frame.contains("Legend: [live] active  [next] pending"));
    }

    #[test]
    fn bindings_pane_hides_marker_legend_without_pattern_markers() {
        let mut app = SessionTui::new(EngineHandle::stub());
        app.input = "warp = fast(2)".to_owned();
        app.submit_line();

        let frame = render_frame_for_test(&app, 120, 24);
        assert!(!frame.contains("Legend: [live] active  [next] pending"));
    }

    #[test]
    fn bindings_pane_hides_marker_legend_on_short_terminal_heights() {
        let mut app = SessionTui::new(EngineHandle::stub());
        app.input = "drums = bd sn".to_owned();
        app.submit_line();
        let _ = app.session.render_test_block_for_tui(256);
        app.input = "backbeat = sn cp".to_owned();
        app.submit_line();

        let frame = render_frame_for_test(&app, 120, 10);
        assert!(frame.contains("[live] drums: Pattern<Sample>"));
        assert!(frame.contains("[next] backbeat: Pattern<Sample>"));
        assert!(!frame.contains("Legend: [live] active  [next] pending"));
    }

    #[test]
    fn page_down_and_page_up_scroll_long_bindings_pane() {
        let mut app = SessionTui::new(EngineHandle::stub());
        for index in 0..12 {
            app.input = format!("b{index:02} = fast(2)");
            app.submit_line();
        }

        let initial_frame = render_frame_for_test(&app, 120, 12);
        assert!(initial_frame.contains("b00: Function(Pattern<t1>) -> Pattern<t1>"));
        assert!(!initial_frame.contains("b11: Function(Pattern<t1>) -> Pattern<t1>"));

        handle_key_event(&mut app, press(KeyCode::PageDown));

        let scrolled_frame = render_frame_for_test(&app, 120, 12);
        assert!(!scrolled_frame.contains("b00: Function(Pattern<t1>) -> Pattern<t1>"));
        assert!(scrolled_frame.contains("b06: Function(Pattern<t1>) -> Pattern<t1>"));
        assert!(scrolled_frame.contains("Bindings 4-7/12"));

        handle_key_event(&mut app, press(KeyCode::PageUp));

        let restored_frame = render_frame_for_test(&app, 120, 12);
        assert!(restored_frame.contains("b00: Function(Pattern<t1>) -> Pattern<t1>"));
        assert!(restored_frame.contains("Bindings 1-4/12"));
    }

    #[test]
    fn syncing_transport_state_uses_cyan_bold_styles() {
        let mut app = SessionTui::new(EngineHandle::stub());
        app.input = "drums = bd sn".to_owned();
        app.submit_line();
        let _ = app.session.render_test_block_for_tui(256);

        app.input = "backbeat = sn cp".to_owned();
        app.submit_line();
        let _ = app.session.render_test_block_for_tui(1);

        let repl_buffer = render_buffer_for_test(&app, 80, 24);
        let (repl_line_x, repl_y) = find_text_in_buffer(&repl_buffer, "Transport: syncing")
            .unwrap_or_else(|| panic!("rendered repl should contain syncing status"));
        let repl_x = repl_line_x
            + u16::try_from("Transport: ".len()).unwrap_or_else(|error| {
                panic!("transport prefix length should fit in u16: {error}")
            });
        let repl_cell = &repl_buffer[(repl_x, repl_y)];
        assert_eq!(repl_cell.fg, Color::Cyan);
        assert!(repl_cell.modifier.contains(Modifier::BOLD));

        let footer_buffer = render_buffer_for_test(&app, 80, 24);
        let footer_line = buffer_line(&footer_buffer, 23);
        assert!(footer_line.contains("syncing"));
        let syncing_x = footer_line
            .find("syncing")
            .and_then(|x| u16::try_from(x).ok())
            .unwrap_or_else(|| panic!("footer should contain syncing status"));
        let syncing_cell = &footer_buffer[(syncing_x, 23)];
        assert_eq!(syncing_cell.fg, Color::Cyan);
        assert!(syncing_cell.modifier.contains(Modifier::BOLD));
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
    fn tab_completes_track_command_prefix() {
        let mut app = SessionTui::new(EngineHandle::stub());
        app.input = ":tr".to_owned();
        app.cursor_index = app.input.len();

        handle_key_event(&mut app, press(KeyCode::Tab));

        assert_eq!(app.input, ":track ");
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
    fn repl_body_shows_updated_bus_completion_hint() {
        let mut app = SessionTui::new(EngineHandle::stub());
        app.input = ":bu".to_owned();
        app.cursor_index = app.input.len();

        assert!(app.repl_body().contains("Hint: Tab -> :bus <new|fx> ..."));
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
        assert!(app.repl_body().contains("Transport: stopped"));

        handle_key_event(&mut app, press(KeyCode::Char(' ')));
        let _ = app.session.render_test_block_for_tui(1);
        assert_eq!(app.status_message.as_deref(), Some("transport playing"));
        assert!(app.session.transport_snapshot().is_playing());
        assert!(app.repl_body().contains("Transport: playing"));
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
        let overlay_frame = render_frame_for_test(&app, 80, 30);
        assert!(overlay_frame.contains("Help"));
        assert!(overlay_frame.contains("Space"));
        assert!(overlay_frame.contains(":play"));
        assert!(overlay_frame.contains(":stop"));
        assert!(overlay_frame.contains(":tempo <bpm>"));
        assert!(overlay_frame.contains(":open <path>"));
        assert!(overlay_frame.contains(":render <binding>"));
        assert!(overlay_frame.contains(":export <binding>"));
        assert!(overlay_frame.contains(":explain <binding>"));
        assert!(overlay_frame.contains(":track"));
        assert!(overlay_frame.contains(":mixer"));
        assert!(overlay_frame.contains("Bindings: PgUp/PgDn"));
        assert!(overlay_frame.contains("Ctrl-A/E/K"));
        assert!(!app.should_quit);

        handle_key_event(&mut app, press(KeyCode::Esc));

        assert!(!app.should_quit);
        assert_eq!(app.status_message.as_deref(), Some("help overlay hidden"));
        let normal_frame = render_frame_for_test(&app, 80, 28);
        assert!(!normal_frame.contains("Toggle: ?"));
        assert!(!normal_frame.contains("Words: Alt-B/F"));
        assert!(normal_frame.contains("? help"));
    }

    #[test]
    fn help_overlay_is_modal_and_swallows_editing_and_scroll_keys() {
        let mut app = SessionTui::new(EngineHandle::stub());
        for index in 0..12 {
            app.input = format!("b{index:02} = fast(2)");
            app.submit_line();
        }
        let bindings_before = app.session.binding_summaries();
        let transcript_before = app.transcript.clone();
        app.input = "bd".to_owned();
        app.cursor_index = app.input.len();

        handle_key_event(&mut app, press(KeyCode::Char('?')));
        handle_key_event(&mut app, press(KeyCode::PageDown));
        handle_key_event(&mut app, press(KeyCode::Char('x')));
        handle_key_event(&mut app, press(KeyCode::Backspace));
        handle_key_event(&mut app, press(KeyCode::Enter));

        assert!(app.show_help);
        assert_eq!(app.input, "bd");
        assert_eq!(app.cursor_index, 2);
        assert_eq!(app.bindings_scroll, 0);
        assert_eq!(app.transcript, transcript_before);
        assert_eq!(app.session.binding_summaries(), bindings_before);
        assert!(!app.should_quit);
    }

    #[test]
    fn help_overlay_allows_question_mark_toggle_and_ctrl_c_quit() {
        let mut app = SessionTui::new(EngineHandle::stub());

        handle_key_event(&mut app, press(KeyCode::Char('?')));
        assert!(app.show_help);

        handle_key_event(&mut app, press(KeyCode::Char('?')));
        assert!(!app.show_help);
        assert_eq!(app.status_message.as_deref(), Some("help overlay hidden"));

        handle_key_event(&mut app, press(KeyCode::Char('?')));
        handle_key_event(&mut app, ctrl(KeyCode::Char('c')));
        assert!(app.should_quit);
    }

    #[test]
    fn help_overlay_renders_modal_backdrop_and_accented_border() {
        let mut app = SessionTui::new(EngineHandle::stub());
        handle_key_event(&mut app, press(KeyCode::Char('?')));

        let buffer = render_buffer_for_test(&app, 80, 24);
        let overlay_area = centered_rect(Rect::new(0, 0, 80, 24), 68, 60);

        let backdrop_cell = &buffer[(0, 0)];
        assert_eq!(backdrop_cell.bg, Color::DarkGray);

        let border_cell = &buffer[(overlay_area.x, overlay_area.y)];
        assert_eq!(border_cell.fg, Color::Cyan);
        assert!(border_cell.modifier.contains(Modifier::BOLD));
    }

    #[test]
    fn help_overlay_shows_close_footer_controls() {
        let mut app = SessionTui::new(EngineHandle::stub());
        handle_key_event(&mut app, press(KeyCode::Char('?')));

        let overlay_frame = render_frame_for_test(&app, 80, 24);
        assert!(overlay_frame.contains("Esc close"));
        assert!(overlay_frame.contains("? toggle"));
        assert!(overlay_frame.contains("Ctrl-C quit"));
    }

    #[test]
    fn main_frame_shows_live_key_legend() {
        let app = SessionTui::new(EngineHandle::stub());

        let frame = render_frame_for_test(&app, 80, 24);
        assert!(frame.contains("? help"));
        assert!(frame.contains("Space toggle"));
        assert!(frame.contains("PgUp/PgDn bindings"));
    }

    #[test]
    fn help_open_footer_switches_to_modal_controls() {
        let mut app = SessionTui::new(EngineHandle::stub());
        handle_key_event(&mut app, press(KeyCode::Char('?')));

        let buffer = render_buffer_for_test(&app, 80, 24);
        let footer_line = buffer_line(&buffer, 23);
        assert!(footer_line.contains("Esc close"));
        assert!(footer_line.contains("? toggle"));
        assert!(footer_line.contains("Ctrl-C quit"));
        assert!(!footer_line.contains("Space toggle"));
    }

    #[test]
    fn main_footer_shows_live_transport_state() {
        let app = SessionTui::new(EngineHandle::stub());

        let buffer = render_buffer_for_test(&app, 80, 24);
        let footer_line = buffer_line(&buffer, 23);
        assert!(footer_line.contains("? help"));
        assert!(footer_line.contains("Space toggle"));
        assert!(footer_line.contains("playing"));
        assert!(footer_line.contains("120 BPM"));
        assert!(footer_line.contains("0.000"));

        let playing_x = footer_line
            .find("playing")
            .and_then(|x| u16::try_from(x).ok())
            .unwrap_or_else(|| panic!("footer should contain playing status"));
        let playing_cell = &buffer[(playing_x, 23)];
        assert_eq!(playing_cell.fg, Color::Green);
        assert!(playing_cell.modifier.contains(Modifier::BOLD));
    }

    #[test]
    fn stopped_transport_updates_footer_state_accent() {
        let mut app = SessionTui::new(EngineHandle::stub());
        app.input = "drums = bd sn".to_owned();
        app.submit_line();
        let _ = app.session.render_test_block_for_tui(256);
        handle_key_event(&mut app, press(KeyCode::Char(' ')));
        let _ = app.session.render_test_block_for_tui(1);

        let buffer = render_buffer_for_test(&app, 80, 24);
        let footer_line = buffer_line(&buffer, 23);
        assert!(footer_line.contains("stopped"));

        let stopped_x = footer_line
            .find("stopped")
            .and_then(|x| u16::try_from(x).ok())
            .unwrap_or_else(|| panic!("footer should contain stopped status"));
        let stopped_cell = &buffer[(stopped_x, 23)];
        assert_eq!(stopped_cell.fg, Color::Yellow);
        assert!(stopped_cell.modifier.contains(Modifier::BOLD));
    }

    #[test]
    fn tempo_command_updates_footer_tempo_readout() {
        let mut app = SessionTui::new(EngineHandle::stub());
        app.input = ":tempo 90".to_owned();
        app.submit_line();
        let _ = app.session.render_test_block_for_tui(1);

        let buffer = render_buffer_for_test(&app, 80, 24);
        let footer_line = buffer_line(&buffer, 23);
        assert!(footer_line.contains("90 BPM"));
        assert!(footer_line.contains("playing"));
    }

    #[test]
    fn render_progress_updates_footer_cycle_readout() {
        let mut app = SessionTui::new(EngineHandle::stub());
        app.input = "drums = bd sn".to_owned();
        app.submit_line();
        let frames_per_cycle = app.session.transport_snapshot().frames_per_cycle();
        let _ = app.session.render_test_block_for_tui(frames_per_cycle / 2);

        let buffer = render_buffer_for_test(&app, 80, 24);
        let footer_line = buffer_line(&buffer, 23);
        assert!(footer_line.contains("0.500"));
        assert!(footer_line.contains("playing"));
    }

    #[test]
    fn narrow_footer_uses_compact_legend_but_keeps_transport_metrics() {
        let app = SessionTui::new(EngineHandle::stub());

        let buffer = render_buffer_for_test(&app, 48, 24);
        let footer_line = buffer_line(&buffer, 23);
        assert!(footer_line.contains("? Space Pg"));
        assert!(footer_line.contains("playing"));
        assert!(footer_line.contains("120 BPM"));
        assert!(footer_line.contains("0.000"));
        assert!(!footer_line.contains("toggle(empty)"));
    }

    #[test]
    fn narrow_help_footer_compacts_close_controls() {
        let mut app = SessionTui::new(EngineHandle::stub());
        handle_key_event(&mut app, press(KeyCode::Char('?')));

        let buffer = render_buffer_for_test(&app, 16, 24);
        let footer_line = buffer_line(&buffer, 23);
        assert!(footer_line.contains("Esc ? Ctrl-C"));
        assert!(!footer_line.contains("toggle"));
        assert!(!footer_line.contains("close"));
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
    fn transport_reports_last_loaded_pattern() {
        let mut app = SessionTui::new(EngineHandle::stub());
        app.input = "drums = bd sn".to_owned();
        app.submit_line();
        let frames_per_cycle = app.session.transport_snapshot().frames_per_cycle();
        let _ = app.session.render_test_block_for_tui(frames_per_cycle / 2);

        let frame = render_frame_for_test(&app, 80, 24);
        assert!(frame.contains("Pattern: drums"));
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

        let frame = render_frame_for_test(&app, 80, 28);
        assert!(frame.contains("Pattern: drums"));
        assert!(frame.contains("Space"));
        assert!(frame.contains("empty input"));
        assert!(frame.contains(":open <path>"));
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

    fn render_buffer_for_test(
        app: &SessionTui,
        width: u16,
        height: u16,
    ) -> ratatui::buffer::Buffer {
        let backend = TestBackend::new(width, height);
        let mut terminal = Terminal::new(backend)
            .unwrap_or_else(|error| panic!("test backend should create a terminal: {error}"));
        terminal
            .draw(|frame| render_session_frame(frame, app))
            .unwrap_or_else(|error| panic!("test backend should render one frame: {error}"));
        terminal.backend().buffer().clone()
    }

    fn find_text_in_buffer(buffer: &ratatui::buffer::Buffer, needle: &str) -> Option<(u16, u16)> {
        for y in 0..buffer.area.height {
            let line = buffer_line(buffer, y);
            if let Some(x) = line.find(needle) {
                let x = u16::try_from(x)
                    .unwrap_or_else(|error| panic!("needle offset should fit in u16: {error}"));
                return Some((x, y));
            }
        }
        None
    }

    fn buffer_line(buffer: &ratatui::buffer::Buffer, y: u16) -> String {
        let mut line = String::new();
        for x in 0..buffer.area.width {
            line.push_str(buffer[(x, y)].symbol());
        }
        line
    }
}
