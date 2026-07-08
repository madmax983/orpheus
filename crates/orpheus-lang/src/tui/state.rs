use std::net::{Ipv4Addr, SocketAddr};
use std::path::Path;
use std::time::{Duration, Instant};

use orpheus_dsp::EngineHandle;

use crate::orca::transport::{
    ScheduledIoEvent, TransportHandle, cycle_schedule, midi_output_names,
};
use crate::orca::{CycleIoEvent, ORCA_GENERATOR_ID, ORCA_PATTERN_NAME, OrcaPublisher};
use crate::session::{MixerView, ReplSession, TransportView};

pub const STATUS_TOAST_TTL: Duration = Duration::from_secs(3);

pub const COMMAND_HINTS: [(&str, &str); 18] = [
    (":bus", ":bus <new|fx> ..."),
    (":explain", ":explain <binding>"),
    (
        ":export",
        ":export <binding> <path> [cycles] | :export stems [cycles] [--buses]",
    ),
    (":import", ":import stems <directory>"),
    (":mixer", ":mixer"),
    (":open", ":open <path>"),
    (
        ":orca",
        ":orca [midi <list|connect <port>|disconnect> | udp <host:port> | osc <host:port>]",
    ),
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
    pub orca: OrcaPublisher,
    /// Handle to the dedicated `orca-io` transport thread (ADR 0010):
    /// non-note grid IO events (and, with a MIDI port connected, note
    /// events) are scheduled onto it, keeping sockets and MIDI off both
    /// the audio thread and the TUI tick.
    pub orca_io: TransportHandle,
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
            orca: OrcaPublisher::with_default_grid(),
            orca_io: TransportHandle::spawn(),
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
        // `:orca ...` configures the TUI-owned grid transports; everything
        // else goes to the session.
        let result = match command.strip_prefix(":orca") {
            Some(args) if args.is_empty() || args.starts_with(char::is_whitespace) => {
                self.orca_command(args.trim())
            }
            _ => self.session.eval_line(command),
        };
        match result {
            Ok(message) => self.set_status_message(message, false),
            Err(error) => self.set_status_message(error, true),
        }
    }

    /// The `:orca` command family: grid transport configuration (design
    /// doc section 12.4). MIDI ports follow the session's `:midi connect`
    /// exact-name convention; UDP/OSC targets accept `host:port` or a bare
    /// port on `127.0.0.1`.
    fn orca_command(&mut self, args: &str) -> Result<String, String> {
        let tokens: Vec<_> = args.split_whitespace().collect();
        match tokens.as_slice() {
            [] => Ok(self.orca_io_summary()),
            ["udp", target] => {
                let address = parse_socket_target(target)?;
                self.orca_io.set_udp_target(address);
                Ok(format!("orca UDP target set to {address}"))
            }
            ["osc", target] => {
                let address = parse_socket_target(target)?;
                self.orca_io.set_osc_target(address);
                Ok(format!("orca OSC target set to {address}"))
            }
            ["midi", "list"] => {
                let names = midi_output_names().map_err(|error| error.to_string())?;
                if names.is_empty() {
                    Ok("available MIDI output ports: <none>".to_owned())
                } else {
                    Ok(format!("available MIDI output ports: {}", names.join(", ")))
                }
            }
            ["midi", "connect", port @ ..] if !port.is_empty() => {
                self.orca_io.connect_midi(&port.join(" "))
            }
            ["midi", "disconnect"] => self.orca_io.disconnect_midi(),
            _ => Err(orca_usage().to_owned()),
        }
    }

    fn orca_io_summary(&self) -> String {
        let config = self.orca_io.config();
        format!(
            "orca transports: udp -> {}, osc -> {}, midi -> {}",
            config.udp_target,
            config.osc_target,
            self.orca_io.midi_port().unwrap_or("<disconnected>"),
        )
    }

    /// Stamps a materialized cycle's IO events with wall-clock deadlines
    /// and ships them to the transport thread. The batch plays from the
    /// next engine cycle boundary (ADR 0009), so events are anchored there;
    /// while the transport is stopped they fire immediately in grid order.
    fn route_orca_io(&self, io: &[CycleIoEvent]) {
        if io.is_empty() {
            return;
        }
        let snapshot = self.session.transport_snapshot();
        let (cycle_start, frame_duration) = cycle_schedule(
            Instant::now(),
            snapshot.tempo_bpm(),
            snapshot.current_frame(),
            snapshot.current_cycle_start_frame(),
            snapshot.frames_per_cycle(),
            self.orca.frames_per_cycle(),
            snapshot.is_playing(),
        );
        let events = io
            .iter()
            .map(|cycle_event| ScheduledIoEvent {
                fire_at: cycle_start
                    + frame_duration
                        * u32::try_from(cycle_event.frame_in_cycle).unwrap_or(u32::MAX),
                frame_duration,
                io: cycle_event.event.io.clone(),
            })
            .collect();
        self.orca_io.schedule(events);
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

    /// Starts or stops the Orca grid clock (ADR 0009). Starting binds the
    /// grid to its engine generator slot and delivers the first cycle;
    /// stopping delivers an empty cycle so the grid falls silent at the next
    /// engine cycle boundary (voices already sounding ring out their full
    /// note length).
    pub fn toggle_orca_running(&mut self) {
        if self.orca.is_running() {
            self.orca.stop();
            match self
                .session
                .stop_generator_source(ORCA_PATTERN_NAME, ORCA_GENERATOR_ID)
            {
                Ok(()) => self.set_status_message("orca grid stopped", false),
                Err(error) => self.set_status_message(error, true),
            }
        } else {
            self.orca.start();
            let cycle_start = self
                .session
                .transport_snapshot()
                .current_cycle_start_frame();
            match self.orca.poll_io(cycle_start) {
                Ok(Some(cycle)) => {
                    self.route_orca_io(&cycle.io);
                    match self.session.start_generator_source(
                        ORCA_PATTERN_NAME,
                        ORCA_GENERATOR_ID,
                        cycle.audio,
                    ) {
                        Ok(()) => self.set_status_message("orca grid running", false),
                        Err(error) => self.set_status_message(error, true),
                    }
                }
                // The first poll after start always materializes.
                Ok(None) => {}
                Err(error) => {
                    self.set_status_message(format!("orca cycle failed: {error}"), true);
                }
            }
        }
    }

    /// Delivers the next grid cycle to the engine generator slot when the
    /// engine has crossed a cycle boundary since the last poll. Called on
    /// every TUI tick; a no-op while the grid clock is stopped or mid-cycle.
    pub fn poll_orca(&mut self) {
        let cycle_start = self
            .session
            .transport_snapshot()
            .current_cycle_start_frame();
        match self.orca.poll_io(cycle_start) {
            Ok(Some(cycle)) => {
                self.route_orca_io(&cycle.io);
                if let Err(error) = self
                    .session
                    .push_generator_cycle(ORCA_GENERATOR_ID, cycle.audio)
                {
                    self.set_status_message(error, true);
                }
            }
            Ok(None) => {}
            Err(error) => {
                self.set_status_message(format!("orca cycle failed: {error}"), true);
            }
        }
        for message in self.orca_io.poll_status() {
            self.set_status_message(message, true);
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

const fn orca_usage() -> &'static str {
    "usage: :orca [midi <list|connect <port>|disconnect> | udp <host:port> | osc <host:port>]"
}

/// Parses a transport target: `host:port`, or a bare port aimed at
/// `127.0.0.1` (the reference client only ever configures the port).
fn parse_socket_target(raw: &str) -> Result<SocketAddr, String> {
    if let Ok(port) = raw.parse::<u16>() {
        return Ok(SocketAddr::from((Ipv4Addr::LOCALHOST, port)));
    }
    raw.parse::<SocketAddr>()
        .map_err(|_| format!("invalid target `{raw}`: expected <host:port> or <port>"))
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
    fn orca_udp_command_sets_the_target() {
        let mut state = SharedState::new(EngineHandle::stub());
        state.input = ":orca udp 127.0.0.1:9000".to_owned();
        state.submit_line();
        assert_eq!(
            state.orca_io.config().udp_target,
            "127.0.0.1:9000".parse().expect("valid address"),
        );
        let (message, is_error) = state.status_message.clone().expect("status set");
        assert!(!is_error, "unexpected error: {message}");
        assert!(message.contains("127.0.0.1:9000"), "got: {message}");
    }

    #[test]
    fn orca_osc_command_accepts_a_bare_port_on_loopback() {
        let mut state = SharedState::new(EngineHandle::stub());
        state.input = ":orca osc 6010".to_owned();
        state.submit_line();
        assert_eq!(
            state.orca_io.config().osc_target,
            "127.0.0.1:6010".parse().expect("valid address"),
        );
    }

    #[test]
    fn orca_command_defaults_match_the_reference_ports() {
        let state = SharedState::new(EngineHandle::stub());
        assert_eq!(state.orca_io.config().udp_target.port(), 49_161);
        assert_eq!(state.orca_io.config().osc_target.port(), 49_162);
        assert!(state.orca_io.midi_port().is_none());
    }

    #[test]
    fn orca_command_without_args_reports_the_targets() {
        let mut state = SharedState::new(EngineHandle::stub());
        state.input = ":orca".to_owned();
        state.submit_line();
        let (message, is_error) = state.status_message.clone().expect("status set");
        assert!(!is_error, "unexpected error: {message}");
        assert!(message.contains("49161"), "got: {message}");
        assert!(message.contains("49162"), "got: {message}");
        assert!(message.contains("<disconnected>"), "got: {message}");
    }

    #[test]
    fn orca_command_rejects_bad_targets_and_unknown_subcommands() {
        let mut state = SharedState::new(EngineHandle::stub());
        state.input = ":orca udp not-an-address".to_owned();
        state.submit_line();
        let (message, is_error) = state.status_message.clone().expect("status set");
        assert!(is_error);
        assert!(message.contains("invalid target"), "got: {message}");

        state.input = ":orca frobnicate".to_owned();
        state.submit_line();
        let (message, is_error) = state.status_message.clone().expect("status set");
        assert!(is_error);
        assert!(message.starts_with("usage: :orca"), "got: {message}");
    }

    #[test]
    fn orca_prefixed_session_commands_still_reach_the_session() {
        // `:orcasomething` must not be captured by the `:orca` family.
        let mut state = SharedState::new(EngineHandle::stub());
        state.input = ":orcasomething".to_owned();
        state.submit_line();
        let (message, is_error) = state.status_message.clone().expect("status set");
        assert!(is_error);
        assert!(message.contains("unknown REPL command"), "got: {message}");
    }

    #[test]
    fn orca_running_grid_routes_io_events_to_the_transport_thread() {
        use crate::orca::transport::RecordingMidiSink;

        let mut state = SharedState::new(EngineHandle::stub());
        let recording = RecordingMidiSink::new();
        state
            .orca_io
            .set_midi_sink(Box::new(recording.clone()), "test-sink");
        {
            let grid = state.orca.engine_mut().grid_mut();
            // `D1` bangs every frame; `:04c` east of the bang cell emits a
            // note per frame.
            grid.set(1, 0, 'D');
            grid.set(2, 0, '1');
            grid.set(2, 1, ':');
            grid.set(3, 1, '0');
            grid.set(4, 1, '4');
            grid.set(5, 1, 'c');
        }
        state.toggle_orca_running();
        assert!(state.orca.is_running());
        // The stub engine is stopped, so events fire immediately on the IO
        // thread; poll until the recording sink sees the first note-on.
        let deadline = Instant::now() + Duration::from_secs(5);
        while recording.messages().is_empty() && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(10));
        }
        let messages = recording.messages();
        assert!(
            messages.iter().any(|bytes| bytes[0] == 0x90),
            "expected a note-on, got: {messages:?}",
        );
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
