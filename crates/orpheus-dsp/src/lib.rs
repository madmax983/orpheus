//! Audio engine for Orpheus.

mod command;
mod engine;
mod offline;
mod sample;
mod sample_bank;
mod scheduler;
mod voice;

pub use command::{EngineCommand, PatternUpdate};
pub use engine::{EngineError, EngineHandle, RenderEngine, TransportSnapshot};
pub use offline::{
    OfflineRenderError, render_events_to_file, render_events_to_file_with_bank,
    render_events_to_wav,
};
pub use sample::{DecodedSample, SampleError, load_wav_for_test};
pub use sample_bank::{
    SampleBank, SampleBankError, load_builtin_sample_for_test, load_sample_bank_from_directory,
};
pub use scheduler::Scheduler;
pub use voice::VoiceKind;
