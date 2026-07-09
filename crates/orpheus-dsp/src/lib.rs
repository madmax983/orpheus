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
mod graph;
mod graph_voice;
mod offline;
mod pedal;
mod plugin_host;
mod routing;
mod sample;
mod sample_bank;
mod sample_manifest;
mod scheduler;
mod synth;
mod transient;
mod voice;

pub use command::{
    EngineCommand, GeneratorCycle, PatternUpdate, PedalProgram, SampleTrigger, new_command_queue,
};
pub use engine::{
    EngineError, EngineHandle, MAX_GENERATORS, RenderEngine, TransportSnapshot, frames_per_cycle,
};
pub use graph::*;
pub use graph_voice::{
    DEFAULT_GRAPH_VOICE_POLYPHONY, DEFAULT_PARAM_RAMP_SECONDS, DEFAULT_VOICE_PARAM_VALUE,
    GraphVoice, GraphVoiceBank, GraphVoiceProgram, GraphVoiceSpec, GraphVoiceSpecError,
    MAX_GRAPH_VOICE_POLYPHONY, MAX_PARAM_RAMP_SECONDS, MAX_VOICE_DELAY_SECONDS,
    MAX_VOICE_RELEASE_TAIL_SECONDS, MODULATED_VOICE_DELAY_MAX_SECONDS, ShelfMode, StealPolicy,
    SvfMode, VOICE_PARAM_COUNT, VOICE_STEAL_RAMP_SECONDS, VoiceNodeSpec, VoiceSignalRef,
    builtin_graph_voice_programs, graph_note_voice_params,
};
pub use offline::{
    GeneratorCycleSpec, OfflineRenderError, render_events_to_file, render_events_to_file_with_bank,
    render_events_to_wav, render_routing_snapshot_to_stem_wavs,
    render_routing_snapshot_to_stereo_for_test,
};
pub use pedal::{
    ClipModel, FilterMode, NodeRef, PEDAL_CONTROL_INTERVAL_SAMPLES, PedalGraphProgram,
    PedalInstance, PedalNode, PedalNodeKind, PedalStage, PreampModel, SignalKind, ToneModel,
};
pub use plugin_host::{
    PluginBufferCapacities, PluginDescriptor, PluginFormat, PluginHostError, PluginNote,
    PluginParameterLane, PluginProcessor, PluginTrackSource,
};
pub use routing::{
    BusEffectSpec, BusId, BusView, DelaySpec, GeneratorId, ReverbSpec, RoutingError,
    RoutingSnapshot, RoutingSnapshotBuilder, TrackId, TrackSource, TrackView,
};
pub use sample::{DecodedSample, SampleError, load_wav_for_test};
pub use sample_bank::{
    PlaybackSample, SampleBank, SampleBankError, SampleLibraryReload, SampleLibraryScanError,
    SampleLibraryWatcher, SampleLibraryWatcherConfig, load_builtin_sample_for_test,
    load_sample_bank_from_directory,
};
pub use scheduler::{ScheduledTrigger, Scheduler};
pub use synth::{
    AnalogVoice, AnalogVoiceParams, Gain, LadderFilter, Mix, Noise, OscShape, PhaseAccumulator,
    PulseOsc, SawOsc, SoftSat, TriOsc,
};
pub use voice::{DEFAULT_ANALOG_BASE_FREQUENCY_HZ, VoiceKind};
