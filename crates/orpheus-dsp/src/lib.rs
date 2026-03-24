//! Audio engine for Orpheus.

mod command;
mod effects;
mod engine;
mod offline;
mod routing;
mod sample;
mod sample_bank;
mod sample_manifest;
mod scheduler;
mod voice;

pub use command::{EngineCommand, PatternUpdate, SampleTrigger};
pub use engine::{EngineError, EngineHandle, RenderEngine, TransportSnapshot};
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
pub use voice::VoiceKind;
