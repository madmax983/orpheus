use std::path::Path;
use std::time::{Duration, Instant};

use orpheus_dsp::EngineHandle;

use crate::session::{MixerView, ReplSession, TransportView};

pub const STATUS_TOAST_TTL: Duration = Duration::from_secs(3);

pub const COMMAND_HINTS: [(&str, &str); 17] = [
    (":bus", ":bus <new|fx> ..."),
    (":explain", ":explain <binding>"),
    (
        ":export",
        ":export <binding> <path> [cycles] | :export stems [cycles] [--buses]",
    ),
    (":import", ":import stems <directory>"),
    (":mixer", ":mixer"),
    (":open", ":open <path>"),
    (":play", ":play"),
    (":quit", ":quit"),
    (":redo", ":redo"),
    (":render", ":render <binding> <path> [cycles]"),
    (":roll", ":roll <binding> [cycles] [steps_per_cycle]"),
    (":send", ":send <track> <bus> <level>"),
    (":stats", ":stats <binding> [cycles]"),
    (":stop", ":stop"),
    (":tempo", ":tempo <bpm>"),
    (":track", ":track <new|bind|level|mute> ..."),
    (":undo", ":undo"),
];

/// Shared application state accessible by all pane plugins via `Rc<RefCell<_>>`.
pub struct SharedState {
    pub session: ReplSession,
    pub transcript: Vec<String>,
    pub history: Vec<String>,
    pub history_index: Option<usize>,
    pub status_message: Option<(String, bool)>,
    pub status_expires_at: Option<Instant>,
    pub input: String,
    pub cursor_index: usize,
    pub show_help: bool,
    pub should_quit: bool,
}

impl SharedState {
    pub fn new(engine: EngineHandle) -> Self {
        Self::with_startup(engine, None, None)
    }

    pub fn with_startup(
        engine: EngineHandle,
        startup_path: Option<&Path>,
        warning: Option<String>,
    ) -> Self {
        let mut transcript = vec![
            "Interactive shell ready.".to_owned(),
            "Press Esc to quit (layout mode).".to_owned(),
        ];
        if let Some(msg) = warning {
            transcript.push(format!("\u{26a0}\u{fe0f} {msg}"));
        }
        let mut state = Self {
            session: ReplSession::with_engine(engine),
            transcript,
            history: Vec::new(),
            history_index: None,
            status_message: None,
            status_expires_at: None,
            input: String::new(),
            cursor_index: 0,
            show_help: false,
            should_quit: false,
        };
        if let Some(path) = startup_path {
            match state.session.open_file(path) {
                Ok(message) => state.transcript.push(format!("\u{2713} {message}")),
                Err(message) => state.transcript.push(format!("\u{2717} {message}")),
            }
        }
        state
    }

    pub fn tick(&mut self) {
        let warnings = self.session.poll_background_tasks();
        for warning in warnings {
            self.transcript.push(format!("\u{26a0}\u{fe0f} {warning}"));
        }
    }

    pub fn submit_line(&mut self) {
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
            self.run_status_command(&line);
            return;
        }

        self.clear_status_message();
        self.transcript.push(format!("> {line}"));
        match self.session.eval_line(&line) {
            Ok(message) => self.transcript.push(format!("\u{2713} {message}")),
            Err(message) => self.transcript.push(format!("\u{2717} {message}")),
        }
    }

    pub fn toggle_transport_hotkey(&mut self) {
        let command = if self.session.transport_snapshot().is_playing() {
            ":stop"
        } else {
            ":play"
        };
        self.run_status_command(command);
    }

    pub fn undo_session_change(&mut self) {
        self.run_status_command(":undo");
    }

    pub fn redo_session_change(&mut self) {
        self.run_status_command(":redo");
    }

    fn run_status_command(&mut self, command: &str) {
        match self.session.eval_line(command) {
            Ok(message) => self.set_status_message(message, false),
            Err(error) => self.set_status_message(error, true),
        }
    }

    pub fn set_status_message(&mut self, message: impl Into<String>, is_error: bool) {
        self.status_message = Some((message.into(), is_error));
        self.status_expires_at = Some(Instant::now() + STATUS_TOAST_TTL);
    }

    pub fn clear_status_message(&mut self) {
        self.status_message = None;
        self.status_expires_at = None;
    }

    pub fn clear_status_if_expired(&mut self, now: Instant) {
        if self
            .status_expires_at
            .is_some_and(|expires_at| now >= expires_at)
        {
            self.clear_status_message();
        }
    }

    pub fn transport_view(&self) -> TransportView {
        self.session.transport_view()
    }

    pub fn mixer_view(&self) -> MixerView {
        self.session.mixer_view()
    }

    // --- Input editing ---

    pub fn insert_character(&mut self, character: char) {
        self.input.insert(self.cursor_index, character);
        self.cursor_index += character.len_utf8();
        self.history_index = None;
    }

    pub fn backspace(&mut self) {
        if self.cursor_index == 0 {
            return;
        }
        let previous = previous_char_boundary(&self.input, self.cursor_index);
        self.input.replace_range(previous..self.cursor_index, "");
        self.cursor_index = previous;
        self.history_index = None;
    }

    pub fn delete(&mut self) {
        if self.cursor_index >= self.input.len() {
            return;
        }
        let next = next_char_boundary(&self.input, self.cursor_index);
        self.input.replace_range(self.cursor_index..next, "");
        self.history_index = None;
    }

    pub fn move_cursor_left(&mut self) {
        self.cursor_index = previous_char_boundary(&self.input, self.cursor_index);
    }

    pub fn move_cursor_right(&mut self) {
        self.cursor_index = next_char_boundary(&self.input, self.cursor_index);
    }

    pub fn move_cursor_previous_word(&mut self) {
        self.cursor_index = previous_word_boundary(&self.input, self.cursor_index);
    }

    pub fn move_cursor_next_word(&mut self) {
        self.cursor_index = next_word_boundary(&self.input, self.cursor_index);
    }

    pub const fn move_cursor_home(&mut self) {
        self.cursor_index = 0;
    }

    pub const fn move_cursor_end(&mut self) {
        self.cursor_index = self.input.len();
    }

    pub fn kill_to_end(&mut self) {
        if self.cursor_index >= self.input.len() {
            return;
        }
        self.input.truncate(self.cursor_index);
        self.history_index = None;
    }

    pub fn kill_to_start(&mut self) {
        if self.cursor_index == 0 {
            return;
        }
        self.input.replace_range(..self.cursor_index, "");
        self.cursor_index = 0;
        self.history_index = None;
    }

    pub fn delete_previous_word(&mut self) {
        if self.cursor_index == 0 {
            return;
        }
        let start = previous_word_boundary(&self.input, self.cursor_index);
        self.input.replace_range(start..self.cursor_index, "");
        self.cursor_index = start;
        self.history_index = None;
    }

    pub fn clear_transcript(&mut self) {
        self.transcript.clear();
        self.set_status_message("transcript cleared", false);
    }

    pub fn toggle_help(&mut self) {
        self.show_help = !self.show_help;
        let message = if self.show_help {
            "help overlay shown"
        } else {
            "help overlay hidden"
        };
        self.set_status_message(message, false);
    }

    pub fn close_help(&mut self) {
        self.show_help = false;
        self.set_status_message("help overlay hidden", false);
    }

    pub fn recall_previous_history(&mut self) {
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

    pub fn recall_next_history(&mut self) {
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

    pub fn complete_input(&mut self) {
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

    pub fn display_input_with_cursor(&self) -> String {
        let (left, right) = self.input.split_at(self.cursor_index);
        format!("{left}|{right}")
    }

    pub fn input_hint(&self) -> String {
        if self.input.is_empty() {
            return "Hint: Tab completes commands. Up/Down recalls history.".to_owned();
        }
        if let Some((_, usage)) = matching_command(&self.input) {
            return format!("Hint: Tab -> {usage}");
        }
        "Hint: Tab completes commands. Up/Down recalls history.".to_owned()
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

const fn previous_char_boundary(input: &str, index: usize) -> usize {
    if index == 0 {
        return 0;
    }
    let mut cursor = index - 1;
    while !input.is_char_boundary(cursor) {
        cursor -= 1;
    }
    cursor
}

const fn next_char_boundary(input: &str, index: usize) -> usize {
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
    let mut cursor = index.min(input.len());
    while !input.is_char_boundary(cursor) && cursor > 0 {
        cursor -= 1;
    }
    while cursor > 0 {
        let previous = previous_char_boundary(input, cursor);
        let Some(character) = input.get(..cursor).and_then(|s| s.chars().next_back()) else {
            break;
        };
        if !character.is_whitespace() {
            break;
        }
        cursor = previous;
    }
    while cursor > 0 {
        let previous = previous_char_boundary(input, cursor);
        let Some(character) = input.get(..cursor).and_then(|s| s.chars().next_back()) else {
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
    let mut cursor = index.min(input.len());
    while !input.is_char_boundary(cursor) && cursor < input.len() {
        cursor += 1;
    }
    while cursor < input.len() {
        let Some(character) = input.get(cursor..).and_then(|s| s.chars().next()) else {
            break;
        };
        if !character.is_whitespace() {
            break;
        }
        cursor = next_char_boundary(input, cursor);
    }
    while cursor < input.len() {
        let Some(character) = input.get(cursor..).and_then(|s| s.chars().next()) else {
            break;
        };
        if character.is_whitespace() {
            break;
        }
        cursor = next_char_boundary(input, cursor);
    }
    cursor
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_previous_char_boundary() {
        let text = "a🚀c";
        assert_eq!(previous_char_boundary(text, 6), 5);
        assert_eq!(previous_char_boundary(text, 5), 1);
        assert_eq!(previous_char_boundary(text, 1), 0);
        assert_eq!(previous_char_boundary(text, 0), 0);
    }

    #[test]
    fn test_next_char_boundary() {
        let text = "a🚀c";
        assert_eq!(next_char_boundary(text, 0), 1);
        assert_eq!(next_char_boundary(text, 1), 5);
        assert_eq!(next_char_boundary(text, 5), 6);
        assert_eq!(next_char_boundary(text, 6), 6);
    }

    #[test]
    fn test_previous_word_boundary() {
        let text = "hello  world";
        assert_eq!(previous_word_boundary(text, 12), 7);
        assert_eq!(previous_word_boundary(text, 7), 0);
        assert_eq!(previous_word_boundary(text, 0), 0);
    }

    #[test]
    fn test_next_word_boundary() {
        let text = "hello  world";
        assert_eq!(next_word_boundary(text, 0), 5);
        assert_eq!(next_word_boundary(text, 5), 12);
        assert_eq!(next_word_boundary(text, 12), 12);
    }

    /// 👺 Havoc: Test that out-of-bounds or non-char boundary inputs do not panic
    #[test]
    fn test_havoc_word_boundary_out_of_bounds() {
        let text = "🚀 def";
        assert_eq!(previous_word_boundary(text, 100), 5);
        assert_eq!(next_word_boundary(text, 100), 8);
        assert_eq!(previous_word_boundary(text, 2), 0);
        assert_eq!(next_word_boundary(text, 2), 8);
    }

    #[test]
    fn test_shared_state_history_tracking() {
        let engine = EngineHandle::stub();
        let mut state = SharedState::new(engine);

        assert!(state.history.is_empty());

        state.input = "test command".to_string();
        state.recall_previous_history();
        assert_eq!(state.input, "test command");

        state.submit_line();
        assert_eq!(state.history.len(), 1);
        assert_eq!(state.history[0], "test command");
        assert_eq!(state.input, "");

        state.input = "another".to_string();
        state.submit_line();
        assert_eq!(state.history.len(), 2);

        state.recall_previous_history();
        assert_eq!(state.input, "another");
        assert_eq!(state.history_index, Some(1));

        state.recall_previous_history();
        assert_eq!(state.input, "test command");
        assert_eq!(state.history_index, Some(0));

        state.recall_previous_history();
        assert_eq!(state.input, "test command");
        assert_eq!(state.history_index, Some(0));

        state.recall_next_history();
        assert_eq!(state.input, "another");
        assert_eq!(state.history_index, Some(1));

        state.recall_next_history();
        assert_eq!(state.input, "");
        assert_eq!(state.history_index, None);
    }

    #[test]
    fn test_shared_state_cursor_movement() {
        let engine = EngineHandle::stub();
        let mut state = SharedState::new(engine);

        state.input = "hello world".to_string();
        state.cursor_index = 11;

        state.move_cursor_left();
        assert_eq!(state.cursor_index, 10);

        state.move_cursor_home();
        assert_eq!(state.cursor_index, 0);

        state.move_cursor_right();
        assert_eq!(state.cursor_index, 1);

        state.move_cursor_end();
        assert_eq!(state.cursor_index, 11);

        state.move_cursor_previous_word();
        assert_eq!(state.cursor_index, 6);

        state.move_cursor_previous_word();
        assert_eq!(state.cursor_index, 0);

        state.move_cursor_next_word();
        assert_eq!(state.cursor_index, 5);

        state.move_cursor_next_word();
        assert_eq!(state.cursor_index, 11);
    }

    #[test]
    fn test_shared_state_editing() {
        let engine = EngineHandle::stub();
        let mut state = SharedState::new(engine);

        state.input = "hello".to_string();
        state.cursor_index = 5;
        state.move_cursor_left();
        state.backspace();
        assert_eq!(state.input, "helo");
        assert_eq!(state.cursor_index, 3);

        state.move_cursor_home();
        state.delete();
        assert_eq!(state.input, "elo");
        assert_eq!(state.cursor_index, 0);

        state.insert_character('y');
        assert_eq!(state.input, "yelo");
        assert_eq!(state.cursor_index, 1);
    }

    #[test]
    fn test_status_messages() {
        let engine = EngineHandle::stub();
        let mut state = SharedState::new(engine);

        assert!(state.status_message.is_none());

        state.set_status_message("hello".to_string(), false);
        assert_eq!(state.status_message, Some(("hello".to_string(), false)));
        assert!(state.status_expires_at.is_some());

        state.clear_status_message();
        assert!(state.status_message.is_none());
        assert!(state.status_expires_at.is_none());
    }

    #[test]
    fn test_toggle_help() {
        let engine = EngineHandle::stub();
        let mut state = SharedState::new(engine);

        assert!(!state.show_help);
        state.toggle_help();
        assert!(state.show_help);
        state.toggle_help();
        assert!(!state.show_help);
    }

    #[test]
    fn test_havoc_previous_word_boundary_oob() {
        let text = "abc def";
        previous_word_boundary(text, 100);
    }

    #[test]
    fn test_havoc_next_word_boundary_oob() {
        let text = "abc def";
        next_word_boundary(text, 100);
    }

    #[test]
    fn test_havoc_next_word_boundary_emoji() {
        let text = "🚀 def";
        next_word_boundary(text, 1);
    }

    #[test]
    fn test_havoc_previous_word_boundary_emoji() {
        let text = "🚀 def";
        previous_word_boundary(text, 1);
    }
}
