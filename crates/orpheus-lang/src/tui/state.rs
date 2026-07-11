use std::net::{Ipv4Addr, SocketAddr};
use std::path::Path;
use std::time::{Duration, Instant};

use orpheus_dsp::EngineHandle;

use crate::orca::transport::{
    DEFAULT_UDP_LISTEN, ScheduledIoEvent, TransportHandle, UdpCommandListener, cycle_schedule,
    midi_output_names,
};
use crate::orca::{
    CommandOutcome, CycleIoEvent, ORCA_GENERATOR_ID, ORCA_PATTERN_NAME, OrcaCommand, OrcaPublisher,
    adjusted_frame, parse_command,
};
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
        ":orca [midi <list|connect <port>|disconnect|clock on|off> | udp <host:port> | osc <host:port> | listen [on|off|<host:port>]]",
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
    /// The optional UDP command listener (`:orca listen`, reference input
    /// port 49160): received datagrams run through the same `$` command
    /// interpreter as grid commands. Off by default.
    pub orca_listener: Option<UdpCommandListener>,
    pub transcript: Vec<String>,
    pub history: Vec<String>,
    pub history_index: Option<usize>,
    pub status_message: Option<(String, bool)>,
    pub status_expires_at: Option<Instant>,
    pub input: String,
    pub cursor_index: usize,
    pub show_help: bool,
    /// Whether the `:`-triggered command palette (fuzzy finder) is open.
    pub palette_open: bool,
    /// The current filter text typed into the command palette.
    pub palette_query: String,
    /// Index of the highlighted row among the palette's filtered matches.
    pub palette_selected: usize,
    pub should_quit: bool,
}

impl SharedState {
    /// Creates a new, default interactive session state wrapping the given audio engine.
    ///
    /// This method configures the standard Ratatui UI with default themes, empty
    /// history, and no loaded file path. It starts the internal `OrcaPublisher`
    /// and initializes the `ReplSession`.
    ///
    /// # Examples
    ///
    /// ```
    /// use orpheus_dsp::{AudioEngine, AudioEngineOptions};
    /// use orpheus_lang::tui::state::SharedState;
    ///
    /// let (engine, handle) = AudioEngine::new(AudioEngineOptions::default());
    /// let state = SharedState::new(handle);
    /// assert_eq!(state.input_hint(), "type :help for commands");
    /// ```
    pub fn new(engine: EngineHandle) -> Self {
        Self::with_startup(engine, None, None)
    }

    /// Creates an interactive session state that immediately evaluates a startup file.
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
            orca_listener: None,
            transcript,
            history: Vec::new(),
            history_index: None,
            status_message: None,
            status_expires_at: None,
            input: String::new(),
            cursor_index: 0,
            show_help: false,
            palette_open: false,
            palette_query: String::new(),
            palette_selected: 0,
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
            ["midi", "clock", "on"] => {
                self.orca_io.set_clock_enabled(true);
                if self.orca.is_running() {
                    let frame_duration = self.grid_frame_duration();
                    self.orca_io.start_clock(frame_duration);
                    Ok("orca MIDI clock on (ticking)".to_owned())
                } else {
                    Ok("orca MIDI clock on; ticks start with the grid".to_owned())
                }
            }
            ["midi", "clock", "off"] => {
                self.orca_io.set_clock_enabled(false);
                self.orca_io.stop_clock();
                Ok("orca MIDI clock off".to_owned())
            }
            ["midi", "connect", port @ ..] if !port.is_empty() => {
                self.orca_io.connect_midi(&port.join(" "))
            }
            ["midi", "disconnect"] => self.orca_io.disconnect_midi(),
            ["listen"] | ["listen", "on"] => self.start_orca_listener(DEFAULT_UDP_LISTEN),
            ["listen", "off"] => {
                if self.orca_listener.take().is_some() {
                    Ok("orca UDP listener stopped".to_owned())
                } else {
                    Err("no orca UDP listener is running".to_owned())
                }
            }
            ["listen", target] => {
                let address = parse_socket_target(target)?;
                self.start_orca_listener(address)
            }
            _ => Err(orca_usage().to_owned()),
        }
    }

    /// Binds (or rebinds) the UDP command listener, surfacing bind
    /// failures as status errors — never a panic.
    fn start_orca_listener(&mut self, address: SocketAddr) -> Result<String, String> {
        // Drop any previous listener first so rebinding the same port works.
        self.orca_listener = None;
        let listener = UdpCommandListener::bind(address)
            .map_err(|error| format!("orca UDP listener failed to bind {address}: {error}"))?;
        let bound = listener.local_addr();
        self.orca_listener = Some(listener);
        Ok(format!("orca UDP listener on {bound}"))
    }

    fn orca_io_summary(&self) -> String {
        let config = self.orca_io.config();
        format!(
            "orca transports: udp -> {}, osc -> {}, midi -> {}, clock {}, listen {}",
            config.udp_target,
            config.osc_target,
            self.orca_io.midi_port().unwrap_or("<disconnected>"),
            if self.orca_io.clock_enabled() {
                "on"
            } else {
                "off"
            },
            self.orca_listener.as_ref().map_or_else(
                || "off".to_owned(),
                |listener| listener.local_addr().to_string()
            ),
        )
    }

    /// The wall-clock duration of one grid frame at the current transport
    /// tempo (the MIDI clock tick anchor).
    fn grid_frame_duration(&self) -> Duration {
        let snapshot = self.session.transport_snapshot();
        let (_, frame_duration) = cycle_schedule(
            Instant::now(),
            snapshot.tempo_bpm(),
            snapshot.current_frame(),
            snapshot.current_cycle_start_frame(),
            snapshot.frames_per_cycle(),
            self.orca.frames_per_cycle(),
            snapshot.is_playing(),
        );
        frame_duration
    }

    /// Interprets one `$`/UDP command string (design doc section 13.1):
    /// supported commands apply, recognized-but-divergent and unknown
    /// commands surface as status-line notes and change nothing.
    pub fn apply_orca_command(&mut self, raw: &str) {
        match parse_command(raw) {
            CommandOutcome::Apply(command) => self.run_orca_command(command),
            CommandOutcome::Divergent(name) => self.set_status_message(
                format!("orca command `{name}` has no Orpheus mapping (ignored)"),
                false,
            ),
            CommandOutcome::NoOp(name) => self.set_status_message(
                format!("orca command `{name}` ignored (missing or invalid value)"),
                false,
            ),
            CommandOutcome::Unknown(name) => {
                self.set_status_message(format!("orca: unknown command `{name}`"), true);
            }
        }
    }

    fn run_orca_command(&mut self, command: OrcaCommand) {
        match command {
            OrcaCommand::Bpm(bpm) => {
                // Grid-driven tempo reaches the global transport (ADR
                // 0011), through the same `:tempo` path the status bar
                // uses; the value was already clamped to 60-300.
                match self.session.eval_line(&format!(":tempo {bpm}")) {
                    Ok(message) => self.set_status_message(format!("orca: {message}"), false),
                    Err(error) => self.set_status_message(error, true),
                }
                if self.orca_io.clock_enabled() {
                    self.orca_io
                        .set_clock_frame_duration(self.grid_frame_duration());
                }
            }
            OrcaCommand::Frame(frame) => {
                self.orca.engine_mut().set_frame(frame);
                self.set_status_message(format!("orca frame set to {frame}"), false);
            }
            OrcaCommand::Rewind(by) => self.move_orca_frame(-by),
            OrcaCommand::Skip(by) => self.move_orca_frame(by),
            OrcaCommand::Play => {
                if !self.orca.is_running() {
                    self.toggle_orca_running();
                }
            }
            OrcaCommand::Stop => {
                if self.orca.is_running() {
                    self.toggle_orca_running();
                }
            }
        }
    }

    fn move_orca_frame(&mut self, delta: i64) {
        let engine = self.orca.engine_mut();
        let frame = adjusted_frame(engine.frame(), delta);
        engine.set_frame(frame);
        self.set_status_message(format!("orca frame set to {frame}"), false);
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
        // Tempo follow: retune the MIDI clock tick period each cycle from
        // the same frame duration the events were scheduled under.
        if self.orca_io.clock_enabled() {
            self.orca_io.set_clock_frame_duration(frame_duration);
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

    /// Starts or stops the Orca grid clock (ADR 0009). Starting binds the
    /// grid to its engine generator slot and delivers the first cycle;
    /// stopping delivers an empty cycle so the grid falls silent at the next
    /// engine cycle boundary (voices already sounding ring out their full
    /// note length).
    pub fn toggle_orca_running(&mut self) {
        if self.orca.is_running() {
            self.orca.stop();
            // MIDI clock stops with the grid (0xFC), like the reference's
            // `clock.stop()` when `isClock` is set.
            if self.orca_io.clock_enabled() {
                self.orca_io.stop_clock();
            }
            match self
                .session
                .stop_generator_source(ORCA_PATTERN_NAME, ORCA_GENERATOR_ID)
            {
                Ok(()) => self.set_status_message("orca grid stopped", false),
                Err(error) => self.set_status_message(error, true),
            }
        } else {
            self.orca.start();
            // MIDI clock starts with the grid (0xFA, then ticks).
            if self.orca_io.clock_enabled() {
                let frame_duration = self.grid_frame_duration();
                self.orca_io.start_clock(frame_duration);
            }
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
        // `$` commands fired at their deadlines on the IO thread, and any
        // datagrams the UDP listener received, run through the shared
        // command interpreter here on the TUI side — never the audio
        // thread (design doc section 13.1).
        for command in self.orca_io.poll_commands() {
            self.apply_orca_command(&command);
        }
        let received = self
            .orca_listener
            .as_ref()
            .map(UdpCommandListener::poll)
            .unwrap_or_default();
        for command in received {
            self.apply_orca_command(&command);
        }
    }

    /// Advances every live multi-cycle arrangement to the engine's current
    /// cycle when a boundary was crossed since the last tick (issue #1446), the
    /// live twin of the offline master-render fix. Runs alongside
    /// [`Self::poll_orca`] on the control thread.
    pub fn poll_arrangements(&mut self) {
        if let Err(error) = self.session.poll_arrangements() {
            self.set_status_message(error, true);
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

    // --- Command palette (`:` fuzzy finder) ---

    /// Opens the command palette, resetting the query and selection.
    pub fn open_palette(&mut self) {
        self.palette_open = true;
        self.palette_query.clear();
        self.palette_selected = 0;
    }

    /// Closes the command palette, leaving the input line untouched.
    pub const fn close_palette(&mut self) {
        self.palette_open = false;
    }

    /// The commands currently matching the palette query (all when empty).
    #[must_use]
    pub fn palette_matches(&self) -> Vec<(&'static str, &'static str)> {
        filter_commands(&self.palette_query, &COMMAND_HINTS)
    }

    /// Appends a character to the palette query and resets the selection to the
    /// top of the (re-filtered) list.
    pub fn palette_insert_char(&mut self, character: char) {
        self.palette_query.push(character);
        self.palette_selected = 0;
    }

    /// Removes the last character from the palette query and resets selection.
    pub fn palette_backspace(&mut self) {
        self.palette_query.pop();
        self.palette_selected = 0;
    }

    /// Moves the palette highlight down one row, wrapping to the top.
    pub fn palette_move_selection_down(&mut self) {
        let len = self.palette_matches().len();
        if len == 0 {
            self.palette_selected = 0;
            return;
        }
        let current = self.palette_selected.min(len - 1);
        self.palette_selected = (current + 1) % len;
    }

    /// Moves the palette highlight up one row, wrapping to the bottom.
    pub fn palette_move_selection_up(&mut self) {
        let len = self.palette_matches().len();
        if len == 0 {
            self.palette_selected = 0;
            return;
        }
        let current = self.palette_selected.min(len - 1);
        self.palette_selected = (current + len - 1) % len;
    }

    /// Inserts the highlighted command into the REPL input line (so the user can
    /// complete any arguments) and closes the palette. Returns `false` when
    /// there is no match to select.
    pub fn palette_select(&mut self) -> bool {
        let matches = self.palette_matches();
        let Some(&(command, _)) =
            matches.get(self.palette_selected.min(matches.len().saturating_sub(1)))
        else {
            return false;
        };
        self.input = command.to_owned();
        self.cursor_index = self.input.len();
        self.history_index = None;
        self.close_palette();
        true
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
    "usage: :orca [midi <list|connect <port>|disconnect|clock on|off> | udp <host:port> | osc <host:port> | listen [on|off|<host:port>]]"
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

/// Filters command entries by a case-insensitive substring match against both
/// the command name and its hint text. An empty (or whitespace-only) query
/// returns every entry, preserving the input order.
///
/// Pure and allocation-transparent so it can be unit-tested directly, decoupled
/// from any palette rendering or key handling.
fn filter_commands<'a>(query: &str, entries: &[(&'a str, &'a str)]) -> Vec<(&'a str, &'a str)> {
    let needle = query.trim().to_lowercase();
    if needle.is_empty() {
        return entries.to_vec();
    }
    entries
        .iter()
        .filter(|(name, hint)| {
            name.to_lowercase().contains(&needle) || hint.to_lowercase().contains(&needle)
        })
        .copied()
        .collect()
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
    fn orca_bpm_command_reaches_the_session_tempo_path() {
        let mut state = SharedState::new(EngineHandle::stub());
        state.apply_orca_command("bpm:140");
        let (message, is_error) = state.status_message.clone().expect("status set");
        assert!(!is_error, "unexpected error: {message}");
        // The session's own `:tempo` confirmation is the seam session
        // tests assert on.
        assert!(message.contains("tempo set to 140 BPM"), "got: {message}");
    }

    #[test]
    fn orca_bpm_command_clamps_to_the_reference_range() {
        let mut state = SharedState::new(EngineHandle::stub());
        state.apply_orca_command("bpm:20");
        let (message, _) = state.status_message.clone().expect("status set");
        assert!(message.contains("tempo set to 60 BPM"), "got: {message}");
        state.apply_orca_command("apm:9999");
        let (message, _) = state.status_message.clone().expect("status set");
        assert!(message.contains("tempo set to 300 BPM"), "got: {message}");
    }

    #[test]
    fn orca_frame_rewind_and_skip_commands_move_the_grid_frame() {
        let mut state = SharedState::new(EngineHandle::stub());
        state.apply_orca_command("frame:12");
        assert_eq!(state.orca.engine().frame(), 12);
        state.apply_orca_command("rewind:4");
        assert_eq!(state.orca.engine().frame(), 8);
        state.apply_orca_command("skip:3");
        assert_eq!(state.orca.engine().frame(), 11);
        // Rewinding past zero clamps like the reference's setFrame.
        state.apply_orca_command("rewind:100");
        assert_eq!(state.orca.engine().frame(), 0);
    }

    #[test]
    fn orca_play_and_stop_commands_drive_the_grid_clock() {
        let mut state = SharedState::new(EngineHandle::stub());
        assert!(!state.orca.is_running());
        state.apply_orca_command("play");
        assert!(state.orca.is_running());
        // A second play is a no-op, like the reference's early return.
        state.apply_orca_command("play");
        assert!(state.orca.is_running());
        state.apply_orca_command("stop");
        assert!(!state.orca.is_running());
    }

    #[test]
    fn orca_unknown_and_divergent_commands_no_op_with_a_status_note() {
        let mut state = SharedState::new(EngineHandle::stub());
        let frame = state.orca.engine().frame();
        state.apply_orca_command("frobnicate:9");
        let (message, is_error) = state.status_message.clone().expect("status set");
        assert!(is_error);
        assert!(
            message.contains("unknown command `frobnicate`"),
            "got: {message}"
        );

        state.apply_orca_command("write:E;2;3");
        let (message, is_error) = state.status_message.clone().expect("status set");
        assert!(!is_error);
        assert!(
            message.contains("`write` has no Orpheus mapping"),
            "got: {message}"
        );
        assert_eq!(state.orca.engine().frame(), frame, "nothing changed");
        assert!(!state.orca.is_running(), "nothing changed");
    }

    #[test]
    fn orca_midi_clock_command_toggles_the_config_flag() {
        let mut state = SharedState::new(EngineHandle::stub());
        assert!(!state.orca_io.clock_enabled(), "clock defaults off");

        state.input = ":orca midi clock on".to_owned();
        state.submit_line();
        let (message, is_error) = state.status_message.clone().expect("status set");
        assert!(!is_error, "unexpected error: {message}");
        assert!(message.contains("MIDI clock on"), "got: {message}");
        assert!(state.orca_io.clock_enabled());

        state.input = ":orca midi clock off".to_owned();
        state.submit_line();
        assert!(!state.orca_io.clock_enabled());

        state.input = ":orca midi clock sideways".to_owned();
        state.submit_line();
        let (message, is_error) = state.status_message.clone().expect("status set");
        assert!(is_error);
        assert!(message.starts_with("usage: :orca"), "got: {message}");
    }

    #[test]
    fn orca_summary_reports_clock_and_listener_state() {
        let mut state = SharedState::new(EngineHandle::stub());
        state.input = ":orca".to_owned();
        state.submit_line();
        let (message, _) = state.status_message.clone().expect("status set");
        assert!(message.contains("clock off"), "got: {message}");
        assert!(message.contains("listen off"), "got: {message}");
    }

    #[test]
    fn orca_listen_command_binds_and_routes_datagrams_to_the_interpreter() {
        use std::net::UdpSocket;

        let mut state = SharedState::new(EngineHandle::stub());
        state.input = ":orca listen 127.0.0.1:0".to_owned();
        state.submit_line();
        let (message, is_error) = state.status_message.clone().expect("status set");
        assert!(!is_error, "unexpected error: {message}");
        let address = state
            .orca_listener
            .as_ref()
            .expect("listener running")
            .local_addr();
        assert!(message.contains(&address.to_string()), "got: {message}");

        let sender = UdpSocket::bind("127.0.0.1:0").expect("bind sender");
        sender.send_to(b"frame:7", address).expect("send datagram");
        let deadline = Instant::now() + Duration::from_secs(5);
        while state.orca.engine().frame() != 7 && Instant::now() < deadline {
            state.poll_orca();
            std::thread::sleep(Duration::from_millis(10));
        }
        assert_eq!(state.orca.engine().frame(), 7, "datagram command applied");

        state.input = ":orca listen off".to_owned();
        state.submit_line();
        assert!(state.orca_listener.is_none());
    }

    #[test]
    fn orca_listen_bind_failure_surfaces_on_the_status_line() {
        use std::net::UdpSocket;

        let holder = UdpSocket::bind("127.0.0.1:0").expect("bind holder");
        let address = holder.local_addr().expect("local addr");
        let mut state = SharedState::new(EngineHandle::stub());
        state.input = format!(":orca listen {address}");
        state.submit_line();
        let (message, is_error) = state.status_message.clone().expect("status set");
        assert!(is_error);
        assert!(message.contains("failed to bind"), "got: {message}");
        assert!(state.orca_listener.is_none());
    }

    #[test]
    fn orca_grid_dollar_command_round_trips_through_the_io_thread() {
        let mut state = SharedState::new(EngineHandle::stub());
        {
            let grid = state.orca.engine_mut().grid_mut();
            // `D` bangs below every frame; `$` east of the bang cell reads
            // `bpm:90` and emits it as a command.
            grid.set(1, 0, 'D');
            grid.set(1, 1, '.');
            grid.set(2, 1, '$');
            grid.set(3, 1, 'b');
            grid.set(4, 1, 'p');
            grid.set(5, 1, 'm');
            grid.set(6, 1, ':');
            grid.set(7, 1, '9');
            grid.set(8, 1, '0');
        }
        state.toggle_orca_running();
        assert!(state.orca.is_running());
        // The stub transport is stopped, so the command fires immediately
        // on the IO thread; poll until it comes back and applies.
        let deadline = Instant::now() + Duration::from_secs(5);
        let mut applied = false;
        while !applied && Instant::now() < deadline {
            state.poll_orca();
            applied = state
                .status_message
                .as_ref()
                .is_some_and(|(message, _)| message.contains("tempo set to 90 BPM"));
            std::thread::sleep(Duration::from_millis(10));
        }
        assert!(applied, "grid $bpm:90 reached the session tempo path");
    }

    #[test]
    fn filter_commands_empty_query_returns_every_entry_in_order() {
        let all = filter_commands("", &COMMAND_HINTS);
        assert_eq!(all.len(), COMMAND_HINTS.len());
        assert_eq!(all[0], COMMAND_HINTS[0]);
        // Whitespace-only queries behave like an empty query.
        assert_eq!(
            filter_commands("   ", &COMMAND_HINTS).len(),
            COMMAND_HINTS.len()
        );
    }

    #[test]
    fn filter_commands_matches_name_case_insensitively() {
        let matches = filter_commands("TEMPO", &COMMAND_HINTS);
        assert!(matches.iter().any(|(name, _)| *name == ":tempo"));
        assert!(
            matches.iter().all(|(name, hint)| {
                name.to_lowercase().contains("tempo") || hint.to_lowercase().contains("tempo")
            }),
            "every match should contain the needle, got: {matches:?}"
        );
    }

    #[test]
    fn filter_commands_matches_hint_text_not_just_the_name() {
        // "bpm" only appears in the `:tempo <bpm>` hint, not in any command name.
        let matches = filter_commands("bpm", &COMMAND_HINTS);
        assert!(
            matches.iter().any(|(name, _)| *name == ":tempo"),
            "hint-text match should surface :tempo, got: {matches:?}"
        );
    }

    #[test]
    fn filter_commands_unmatched_query_returns_empty() {
        assert!(filter_commands("zzznotacommand", &COMMAND_HINTS).is_empty());
    }

    #[test]
    fn palette_open_reset_and_select_prefills_the_input_line() {
        let mut state = SharedState::new(EngineHandle::stub());
        assert!(!state.palette_open);

        state.open_palette();
        assert!(state.palette_open);
        assert!(state.palette_query.is_empty());
        assert_eq!(state.palette_selected, 0);

        for character in "play".chars() {
            state.palette_insert_char(character);
        }
        assert_eq!(state.palette_query, "play");
        let matches = state.palette_matches();
        let index = matches
            .iter()
            .position(|(name, _)| *name == ":play")
            .expect(":play should match the query");
        state.palette_selected = index;

        assert!(state.palette_select());
        assert_eq!(state.input, ":play");
        assert_eq!(state.cursor_index, state.input.len());
        assert!(!state.palette_open, "selecting closes the palette");
    }

    #[test]
    fn palette_backspace_edits_the_query_and_resets_selection() {
        let mut state = SharedState::new(EngineHandle::stub());
        state.open_palette();
        for character in "play".chars() {
            state.palette_insert_char(character);
        }
        state.palette_selected = 3;
        state.palette_backspace();
        assert_eq!(state.palette_query, "pla");
        assert_eq!(state.palette_selected, 0);
    }

    #[test]
    fn palette_selection_wraps_around_the_match_list() {
        let mut state = SharedState::new(EngineHandle::stub());
        state.open_palette();
        let len = state.palette_matches().len();
        assert!(len > 1);

        // Up from the top wraps to the bottom.
        state.palette_move_selection_up();
        assert_eq!(state.palette_selected, len - 1);

        // Down from the bottom wraps back to the top.
        state.palette_move_selection_down();
        assert_eq!(state.palette_selected, 0);

        state.palette_move_selection_down();
        assert_eq!(state.palette_selected, 1);
    }

    #[test]
    fn palette_select_with_no_matches_is_a_noop() {
        let mut state = SharedState::new(EngineHandle::stub());
        state.open_palette();
        for character in "zzznope".chars() {
            state.palette_insert_char(character);
        }
        assert!(state.palette_matches().is_empty());
        assert!(!state.palette_select());
        assert!(state.input.is_empty());
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
