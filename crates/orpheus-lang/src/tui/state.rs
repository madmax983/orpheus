use std::path::Path;
use std::time::{Duration, Instant};

use orpheus_dsp::EngineHandle;

use crate::session::{MixerView, ReplSession, TransportView};

pub const STATUS_TOAST_TTL: Duration = Duration::from_secs(3);

pub const COMMAND_HINTS: [(&str, &str); 14] = [
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

/// Shared application state accessible by all pane plugins via `Rc<RefCell<_>>`.
pub struct SharedState {
    pub session: ReplSession,
    pub transcript: Vec<String>,
    pub history: Vec<String>,
    pub history_index: Option<usize>,
    pub status_message: Option<String>,
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
        let message = self
            .session
            .eval_line(command)
            .unwrap_or_else(|error| error);
        self.set_status_message(message);
    }

    pub fn set_status_message(&mut self, message: impl Into<String>) {
        self.status_message = Some(message.into());
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
        self.set_status_message("transcript cleared");
    }

    pub fn toggle_help(&mut self) {
        self.show_help = !self.show_help;
        self.set_status_message(if self.show_help {
            "help overlay shown"
        } else {
            "help overlay hidden"
        });
    }

    pub fn close_help(&mut self) {
        self.show_help = false;
        self.set_status_message("help overlay hidden");
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

#[cfg(test)]
mod tests {
    use super::*;
    use orpheus_dsp::EngineHandle;

    fn setup_state() -> SharedState {
        SharedState::new(EngineHandle::stub())
    }

    #[test]
    fn test_cursor_movement_left_right() {
        let mut state = setup_state();
        state.input = "hello".to_string();
        state.cursor_index = 2; // "he|llo"

        state.move_cursor_left();
        assert_eq!(state.cursor_index, 1);

        state.move_cursor_right();
        assert_eq!(state.cursor_index, 2);

        state.move_cursor_home();
        assert_eq!(state.cursor_index, 0);

        state.move_cursor_left();
        assert_eq!(state.cursor_index, 0); // clamp

        state.move_cursor_end();
        assert_eq!(state.cursor_index, 5);

        state.move_cursor_right();
        assert_eq!(state.cursor_index, 5); // clamp
    }

    #[test]
    fn test_insert_character() {
        let mut state = setup_state();
        state.insert_character('a');
        assert_eq!(state.input, "a");
        assert_eq!(state.cursor_index, 1);

        state.insert_character('b');
        assert_eq!(state.input, "ab");
        assert_eq!(state.cursor_index, 2);

        state.move_cursor_left();
        state.insert_character('c');
        assert_eq!(state.input, "acb");
        assert_eq!(state.cursor_index, 2);
    }

    #[test]
    fn test_backspace() {
        let mut state = setup_state();
        state.input = "hello".to_string();
        state.cursor_index = 5;

        state.backspace();
        assert_eq!(state.input, "hell");
        assert_eq!(state.cursor_index, 4);

        state.cursor_index = 0;
        state.backspace(); // Should not panic or change
        assert_eq!(state.input, "hell");
        assert_eq!(state.cursor_index, 0);
    }

    #[test]
    fn test_delete() {
        let mut state = setup_state();
        state.input = "hello".to_string();
        state.cursor_index = 0;

        state.delete();
        assert_eq!(state.input, "ello");
        assert_eq!(state.cursor_index, 0);

        state.cursor_index = 4;
        state.delete(); // Should not panic or change
        assert_eq!(state.input, "ello");
        assert_eq!(state.cursor_index, 4);
    }

    #[test]
    fn test_kill_to_end() {
        let mut state = setup_state();
        state.input = "hello world".to_string();
        state.cursor_index = 5;

        state.kill_to_end();
        assert_eq!(state.input, "hello");
        assert_eq!(state.cursor_index, 5);
    }

    #[test]
    fn test_kill_to_start() {
        let mut state = setup_state();
        state.input = "hello world".to_string();
        state.cursor_index = 6;

        state.kill_to_start();
        assert_eq!(state.input, "world");
        assert_eq!(state.cursor_index, 0);
    }

    #[test]
    fn test_word_boundaries() {
        let input = "hello world  rust";
        assert_eq!(previous_word_boundary(input, 0), 0);
        assert_eq!(previous_word_boundary(input, 5), 0); // "hello|" -> "|hello"
        assert_eq!(previous_word_boundary(input, 6), 0); // "hello |" -> "|hello"
        assert_eq!(previous_word_boundary(input, 11), 6); // "hello world|" -> "hello |world"

        assert_eq!(next_word_boundary(input, 0), 5); // "|hello" -> "hello|"
        assert_eq!(next_word_boundary(input, 5), 11); // "hello|" -> "hello world|"
        assert_eq!(next_word_boundary(input, 6), 11); // "hello |" -> "hello world|"
        assert_eq!(next_word_boundary(input, 11), 17); // "hello world|" -> "hello world  rust|"
        assert_eq!(next_word_boundary(input, 17), 17);
    }

    #[test]
    fn test_move_cursor_words() {
        let mut state = setup_state();
        state.input = "hello world rust".to_string();
        state.cursor_index = 0;

        state.move_cursor_next_word();
        assert_eq!(state.cursor_index, 5);

        state.move_cursor_next_word();
        assert_eq!(state.cursor_index, 11);

        state.move_cursor_previous_word();
        assert_eq!(state.cursor_index, 6);
    }

    #[test]
    fn test_delete_previous_word() {
        let mut state = setup_state();
        state.input = "hello world rust".to_string();
        state.cursor_index = 11;

        state.delete_previous_word();
        assert_eq!(state.input, "hello  rust");
        assert_eq!(state.cursor_index, 6);
    }

    #[test]
    fn test_history() {
        let mut state = setup_state();

        state.input = "first".to_string();
        state.submit_line();
        assert_eq!(state.history, vec!["first"]);

        state.input = "second".to_string();
        state.submit_line();
        assert_eq!(state.history, vec!["first", "second"]);

        state.recall_previous_history();
        assert_eq!(state.input, "second");
        assert_eq!(state.history_index, Some(1));

        state.recall_previous_history();
        assert_eq!(state.input, "first");
        assert_eq!(state.history_index, Some(0));

        state.recall_next_history();
        assert_eq!(state.input, "second");
        assert_eq!(state.history_index, Some(1));

        state.recall_next_history();
        assert_eq!(state.input, "");
        assert_eq!(state.history_index, None);
    }

    #[test]
    fn test_status_message() {
        let mut state = setup_state();

        state.set_status_message("hello");
        assert_eq!(state.status_message.as_deref(), Some("hello"));
        assert!(state.status_expires_at.is_some());

        state.clear_status_message();
        assert_eq!(state.status_message, None);
        assert_eq!(state.status_expires_at, None);
    }

    #[test]
    fn test_clear_status_if_expired() {
        let mut state = setup_state();
        state.set_status_message("test");
        let expires_at = state.status_expires_at.unwrap();

        // Not expired
        state.clear_status_if_expired(expires_at - Duration::from_secs(1));
        assert!(state.status_message.is_some());

        // Expired
        state.clear_status_if_expired(expires_at + Duration::from_secs(1));
        assert!(state.status_message.is_none());
    }

    #[test]
    fn test_toggle_help() {
        let mut state = setup_state();
        assert!(!state.show_help);

        state.toggle_help();
        assert!(state.show_help);

        state.close_help();
        assert!(!state.show_help);
    }

    #[test]
    fn test_matching_command() {
        assert_eq!(matching_command(":q"), Some((":quit", ":quit")));
        assert_eq!(matching_command(":p"), Some((":play", ":play")));

        // Ambiguous command -> None
        assert_eq!(matching_command(":"), None);
    }

    #[test]
    fn test_complete_input() {
        let mut state = setup_state();
        state.input = ":q".to_string();
        state.cursor_index = 2;

        state.complete_input();
        assert_eq!(state.input, ":quit");

        state.input = ":t".to_string();
        state.cursor_index = 2;
        state.complete_input(); // Could be :tempo or :track => ambiguous => no complete
        assert_eq!(state.input, ":t");
    }

    #[test]
    fn test_display_input_with_cursor() {
        let mut state = setup_state();
        state.input = "hello".to_string();
        state.cursor_index = 2;
        assert_eq!(state.display_input_with_cursor(), "he|llo");
    }
}
