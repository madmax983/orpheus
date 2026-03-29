//! Audio engine for Orpheus.
//!
//! This crate provides the digital signal processing backend for rendering musical
//! patterns into audio streams. It supports real-time playback via CPAL and offline
//! rendering to WAV files.
//!
//! Key components:
//! - [`RenderEngine`]: The central real-time audio synthesizer and sequencer.
//! - [`EngineHandle`]: A thread-safe handle to send [`EngineCommand`]s to the `RenderEngine`.
//! - [`SampleBank`]: A collection of loaded WAV files ready for playback.
//! - [`render_events_to_wav`]: An offline rendering utility for generating static audio files.

mod command;
mod effects;
mod engine;
pub mod graph;
mod offline;
mod routing;
mod sample;
mod sample_bank;
mod sample_manifest;
mod scheduler;
mod synth;
mod voice;

pub use command::{EngineCommand, PatternUpdate, SampleTrigger, new_command_queue};
pub use engine::{EngineError, EngineHandle, RenderEngine, TransportSnapshot, frames_per_cycle};
pub use offline::{
    OfflineRenderError, render_events_to_file, render_events_to_file_with_bank,
    render_events_to_wav, render_routing_snapshot_to_stereo_for_test,
};
pub use routing::{
    BusEffectSpec, BusId, BusView, DelaySpec, ReverbSpec, RoutingError, RoutingSnapshot,
    RoutingSnapshotBuilder, TrackId, TrackSource, TrackView,
};
pub use sample::{DecodedSample, SampleError, load_wav_for_test};
pub use sample_bank::{
    SampleBank, SampleBankError, load_builtin_sample_for_test, load_sample_bank_from_directory,
};
pub use scheduler::Scheduler;
pub use synth::{
    AnalogVoice, AnalogVoiceParams, Gain, LadderFilter, Mix, Noise, OscShape, PhaseAccumulator,
    PulseOsc, SawOsc, SoftSat, TriOsc,
};
pub use voice::VoiceKind;
