//! The dedicated IO thread hosting the transport dispatcher (ADR 0010).
//!
//! [`TransportHandle::spawn`] starts one `orca-io` thread owning a
//! [`TransportDispatcher`]. The host feeds it over an `mpsc` channel and
//! never blocks: scheduling is a channel send, and transport errors flow
//! back over a status channel drained by the TUI tick. Socket and MIDI
//! sends therefore happen only on the IO thread — never on the audio
//! thread (which stays allocation-free and lock-free) and never on the TUI
//! thread's critical path.
//!
//! The worker sleeps until the next queued deadline (or an idle tick),
//! wakes for new commands, and fires everything due. Dropping the handle
//! closes the command channel; the worker then releases any sounding MIDI
//! notes (so devices are not left hanging), discards other pending events,
//! and exits — the drop joins it.

use std::net::SocketAddr;
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender, channel};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use super::dispatch::TransportDispatcher;
use super::midi::{MidiSink, connect_midi_output};
use super::{ScheduledIoEvent, TransportConfig};

/// How long the worker sleeps when nothing is queued.
const IDLE_TIMEOUT: Duration = Duration::from_millis(250);

/// Commands accepted by the IO worker.
pub enum TransportCommand {
    /// Queue events for their deadlines.
    Schedule(Vec<ScheduledIoEvent>),
    /// Attach (or detach, with `None`) the MIDI sink.
    SetMidi(Option<Box<dyn MidiSink>>),
    /// Re-aim the UDP transport.
    SetUdpTarget(SocketAddr),
    /// Re-aim the OSC transport.
    SetOscTarget(SocketAddr),
}

impl std::fmt::Debug for TransportCommand {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Schedule(events) => formatter
                .debug_tuple("Schedule")
                .field(&events.len())
                .finish(),
            Self::SetMidi(sink) => formatter
                .debug_tuple("SetMidi")
                .field(&sink.is_some())
                .finish(),
            Self::SetUdpTarget(target) => {
                formatter.debug_tuple("SetUdpTarget").field(target).finish()
            }
            Self::SetOscTarget(target) => {
                formatter.debug_tuple("SetOscTarget").field(target).finish()
            }
        }
    }
}

/// Host-side handle to the IO worker thread. Owns the command channel and
/// the last-known configuration (for status display); dropping it shuts
/// the worker down and joins it.
#[derive(Debug)]
pub struct TransportHandle {
    commands: Option<Sender<TransportCommand>>,
    status: Receiver<String>,
    worker: Option<JoinHandle<()>>,
    config: TransportConfig,
    midi_port: Option<String>,
}

impl TransportHandle {
    /// Spawns the IO worker with default targets (see
    /// [`super::TransportConfig`]).
    #[must_use]
    pub fn spawn() -> Self {
        Self::spawn_with_config(TransportConfig::default())
    }

    /// Spawns the IO worker with explicit targets.
    ///
    /// # Panics
    ///
    /// Panics if the OS refuses to spawn a thread.
    #[must_use]
    pub fn spawn_with_config(config: TransportConfig) -> Self {
        let (command_sender, command_receiver) = channel();
        let (status_sender, status_receiver) = channel();
        let worker_config = config;
        let worker = std::thread::Builder::new()
            .name("orca-io".to_owned())
            .spawn(move || run_worker(&command_receiver, &status_sender, worker_config))
            .expect("spawn orca-io thread");
        Self {
            commands: Some(command_sender),
            status: status_receiver,
            worker: Some(worker),
            config,
            midi_port: None,
        }
    }

    /// Queues events for dispatch at their deadlines. A dead worker drops
    /// them silently (the process is shutting down).
    pub fn schedule(&self, events: Vec<ScheduledIoEvent>) {
        if events.is_empty() {
            return;
        }
        self.send(TransportCommand::Schedule(events));
    }

    /// Connects the named MIDI output port (exact name, matching the
    /// session's `:midi connect` convention) and hands it to the worker.
    ///
    /// # Errors
    ///
    /// Returns the [`super::TransportError`] display string when the port
    /// cannot be listed or connected.
    pub fn connect_midi(&mut self, port_name: &str) -> Result<String, String> {
        let sink = connect_midi_output(port_name).map_err(|error| error.to_string())?;
        self.send(TransportCommand::SetMidi(Some(Box::new(sink))));
        self.midi_port = Some(port_name.to_owned());
        Ok(format!("orca MIDI output connected to `{port_name}`"))
    }

    /// Detaches the MIDI sink.
    ///
    /// # Errors
    ///
    /// Returns an error when no MIDI output is attached.
    pub fn disconnect_midi(&mut self) -> Result<String, String> {
        let Some(port_name) = self.midi_port.take() else {
            return Err("no orca MIDI output is connected".to_owned());
        };
        self.send(TransportCommand::SetMidi(None));
        Ok(format!("orca MIDI output `{port_name}` disconnected"))
    }

    /// Injects an arbitrary MIDI sink (the test seam), labeled for status
    /// display.
    pub fn set_midi_sink(&mut self, sink: Box<dyn MidiSink>, label: &str) {
        self.send(TransportCommand::SetMidi(Some(sink)));
        self.midi_port = Some(label.to_owned());
    }

    /// Re-aims the UDP transport.
    pub fn set_udp_target(&mut self, target: SocketAddr) {
        self.config.udp_target = target;
        self.send(TransportCommand::SetUdpTarget(target));
    }

    /// Re-aims the OSC transport.
    pub fn set_osc_target(&mut self, target: SocketAddr) {
        self.config.osc_target = target;
        self.send(TransportCommand::SetOscTarget(target));
    }

    /// The last-known configuration.
    #[must_use]
    pub const fn config(&self) -> &TransportConfig {
        &self.config
    }

    /// The connected MIDI port name (or test-sink label), if any.
    #[must_use]
    pub fn midi_port(&self) -> Option<&str> {
        self.midi_port.as_deref()
    }

    /// Drains transport error messages reported by the worker since the
    /// last poll.
    #[must_use]
    pub fn poll_status(&self) -> Vec<String> {
        self.status.try_iter().collect()
    }

    fn send(&self, command: TransportCommand) {
        if let Some(sender) = &self.commands {
            // A send failure means the worker exited (process shutdown);
            // nothing useful can be done with the command.
            let _ = sender.send(command);
        }
    }
}

impl Drop for TransportHandle {
    fn drop(&mut self) {
        // Closing the command channel tells the worker to flush note-offs
        // and exit; join so its sockets and MIDI connection are released
        // before the handle's owner finishes dropping.
        self.commands = None;
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

fn run_worker(
    commands: &Receiver<TransportCommand>,
    status: &Sender<String>,
    config: TransportConfig,
) {
    let mut dispatcher = TransportDispatcher::new(&config);
    loop {
        let timeout = dispatcher.next_due().map_or(IDLE_TIMEOUT, |due| {
            due.saturating_duration_since(Instant::now())
        });
        match commands.recv_timeout(timeout) {
            Ok(command) => apply(&mut dispatcher, command),
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => break,
        }
        for error in dispatcher.run_due(Instant::now()) {
            let _ = status.send(error);
        }
    }
    // Shutdown hygiene: release sounding notes so devices are not left
    // hanging. The status channel is usually gone by now; ignore failures.
    for error in dispatcher.flush_note_offs() {
        let _ = status.send(error);
    }
}

fn apply(dispatcher: &mut TransportDispatcher, command: TransportCommand) {
    match command {
        TransportCommand::Schedule(events) => {
            for event in events {
                dispatcher.schedule(event);
            }
        }
        TransportCommand::SetMidi(sink) => dispatcher.set_midi(sink),
        TransportCommand::SetUdpTarget(target) => dispatcher.set_udp_target(target),
        TransportCommand::SetOscTarget(target) => dispatcher.set_osc_target(target),
    }
}
