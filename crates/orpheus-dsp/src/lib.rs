//! Audio engine for Orpheus.

mod command;
mod engine;
mod offline;
mod sample;
mod sample_bank;
mod sample_manifest;
mod scheduler;
mod voice;

pub use command::{EngineCommand, PatternUpdate, SampleTrigger, new_command_queue};
pub use engine::{EngineError, EngineHandle, RenderEngine, TransportSnapshot};
pub use offline::{
    OfflineRenderError, render_events_to_file, render_events_to_file_with_bank,
    render_events_to_wav,
};
pub use sample::{DecodedSample, SampleError, load_wav_for_test};
pub use sample_bank::{
    SampleBank, SampleBankError, load_builtin_sample_for_test, load_sample_bank_from_directory,
};
pub use sample_manifest::load_sample_manifest;
pub use scheduler::Scheduler;
pub use voice::VoiceKind;
