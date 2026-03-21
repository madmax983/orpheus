//! The `offline` module provides non-real-time audio rendering.
//!
//! This module allows evaluated patterns to be rendered directly to audio files (like `.wav`)
//! as fast as the CPU allows, bypassing the real-time system audio callbacks. This is useful
//! for exporting bounces, offline testing, and generating static assets.

use std::fs;
use std::path::Path;

use flacenc::bitsink::ByteSink;
use flacenc::component::BitRepr;
use flacenc::config::Encoder;
use flacenc::error::Verify;
use flacenc::source::MemSource;
use orpheus_pattern::Event;
use thiserror::Error;

use crate::SampleTrigger;
use crate::engine::{DEFAULT_SAMPLE_RATE, DEFAULT_TEMPO_BPM, EngineError, frames_per_cycle};
use crate::sample_bank::SampleBank;
use crate::scheduler::Scheduler;
use crate::voice::{ActiveVoice, VoiceKind};

const OFFLINE_CHANNELS: u16 = 2;
const MAX_ACTIVE_VOICES: usize = 32;
const PCM_BITS_PER_SAMPLE: u8 = 16;
const FLAC_BLOCK_SIZE: usize = 1024;

/// Errors raised while rendering an offline export.
#[derive(Debug, Error)]
pub enum OfflineRenderError {
    #[error("offline rendering requires at least one cycle")]
    InvalidCycleCount,
    #[error(transparent)]
    Engine(#[from] EngineError),
    #[error("unsupported render format `{0}`")]
    UnsupportedFormat(Box<str>),
    #[error("failed to write audio file `{path}`: {message}")]
    Io { path: Box<str>, message: Box<str> },
    #[error("failed to write wav file `{path}`: {message}")]
    WavIo { path: Box<str>, message: Box<str> },
    #[error("failed to verify FLAC encoder config: {0}")]
    FlacConfig(Box<str>),
    #[error("failed to encode FLAC output: {0}")]
    FlacEncode(Box<str>),
    #[error("unknown sample token `{0}`")]
    UnknownSampleToken(Box<str>),
}

/// Renders explicit-time sample token events to a deterministic stereo audio
/// file, selecting the sink from the target file extension.
///
/// Supported extensions:
/// - `.wav`
/// - `.flac`
///
/// # Errors
///
/// Returns [`OfflineRenderError`] if event scheduling fails or if the output
/// file cannot be written.
pub fn render_events_to_file(
    path: impl AsRef<Path>,
    events: &[Event<SampleTrigger>],
    cycle_count: u64,
) -> Result<(), OfflineRenderError> {
    let builtin_bank = SampleBank::load_builtin();
    render_events_to_file_with_bank(path, events, cycle_count, &builtin_bank)
}

/// Renders explicit-time sample token events to a deterministic stereo audio
/// file using the supplied sample bank overrides.
///
/// # Errors
///
/// Returns [`OfflineRenderError`] if event scheduling fails or if the output
/// file cannot be written.
pub fn render_events_to_file_with_bank(
    path: impl AsRef<Path>,
    events: &[Event<SampleTrigger>],
    cycle_count: u64,
    sample_bank: &SampleBank,
) -> Result<(), OfflineRenderError> {
    let path = path.as_ref();
    let rendered = render_events_to_pcm(events, cycle_count, sample_bank)?;
    match extension(path).as_deref() {
        Some("wav") => write_wav(path, &rendered),
        Some("flac") => write_flac(path, &rendered),
        Some(other) => Err(OfflineRenderError::UnsupportedFormat(other.into())),
        None => Err(OfflineRenderError::UnsupportedFormat("".into())),
    }
}

/// Renders explicit-time sample token events to a deterministic stereo WAV.
///
/// # Errors
///
/// Returns [`OfflineRenderError`] if event scheduling fails or the output file
/// cannot be written.
pub fn render_events_to_wav(
    path: impl AsRef<Path>,
    events: &[Event<SampleTrigger>],
    cycle_count: u64,
) -> Result<(), OfflineRenderError> {
    let builtin_bank = SampleBank::load_builtin();
    let rendered = render_events_to_pcm(events, cycle_count, &builtin_bank)?;
    write_wav(path.as_ref(), &rendered)
}

fn render_events_to_pcm(
    events: &[Event<SampleTrigger>],
    cycle_count: u64,
    sample_bank: &SampleBank,
) -> Result<Vec<i32>, OfflineRenderError> {
    if cycle_count == 0 {
        return Err(OfflineRenderError::InvalidCycleCount);
    }

    let frames_per_cycle = frames_per_cycle(DEFAULT_SAMPLE_RATE, DEFAULT_TEMPO_BPM)?;
    let total_frames = frames_per_cycle
        .checked_mul(cycle_count)
        .ok_or(EngineError::FrameOverflow)?;
    let total_samples = total_frames
        .checked_mul(u64::from(OFFLINE_CHANNELS))
        .ok_or(EngineError::FrameOverflow)?;
    let total_samples = usize::try_from(total_samples).map_err(|_| EngineError::FrameOverflow)?;
    let mut scheduler = Scheduler::default();
    scheduler.schedule_cycle_events(
        0,
        frames_per_cycle,
        events.iter().map(|event| Event {
            whole: event.whole.clone(),
            part: event.part.clone(),
            value: &event.value,
        }),
    )?;

    let mut active_voices = vec![None; MAX_ACTIVE_VOICES];
    let mut rendered = Vec::with_capacity(total_samples);

    for frame in 0..total_frames {
        while let Some(trigger) = scheduler.pop_due(frame) {
            activate_voice(
                &mut active_voices,
                sample_bank,
                &trigger.trigger,
                trigger.fallback_voice,
                DEFAULT_SAMPLE_RATE,
            )?;
        }

        let (left, right) = mix_voices(&mut active_voices);
        rendered.push(i32::from(float_to_pcm16(left)));
        rendered.push(i32::from(float_to_pcm16(right)));
    }

    Ok(rendered)
}

fn write_wav(path: &Path, samples: &[i32]) -> Result<(), OfflineRenderError> {
    let path_string = path.display().to_string();
    let spec = hound::WavSpec {
        channels: OFFLINE_CHANNELS,
        sample_rate: DEFAULT_SAMPLE_RATE,
        bits_per_sample: u16::from(PCM_BITS_PER_SAMPLE),
        sample_format: hound::SampleFormat::Int,
    };
    let mut writer =
        hound::WavWriter::create(path, spec).map_err(|source| OfflineRenderError::WavIo {
            path: path_string.clone().into_boxed_str(),
            message: match &source {
                hound::Error::IoError(io_err) => match io_err.kind() {
                    std::io::ErrorKind::NotFound => "file not found".into(),
                    std::io::ErrorKind::PermissionDenied => "permission denied".into(),
                    _ => io_err.to_string().into_boxed_str(),
                },
                _ => source.to_string().into_boxed_str(),
            },
        })?;

    for sample in samples {
        writer
            .write_sample(i16::try_from(*sample).unwrap_or_else(|_| {
                panic!("rendered PCM sample {sample} did not fit into 16-bit output")
            }))
            .map_err(|source| OfflineRenderError::WavIo {
                path: path_string.clone().into_boxed_str(),
                message: match &source {
                    hound::Error::IoError(io_err) => match io_err.kind() {
                        std::io::ErrorKind::NotFound => "file not found".into(),
                        std::io::ErrorKind::PermissionDenied => "permission denied".into(),
                        _ => io_err.to_string().into_boxed_str(),
                    },
                    _ => source.to_string().into_boxed_str(),
                },
            })?;
    }

    writer
        .finalize()
        .map_err(|source| OfflineRenderError::WavIo {
            path: path_string.into_boxed_str(),
            message: match &source {
                hound::Error::IoError(io_err) => match io_err.kind() {
                    std::io::ErrorKind::NotFound => "file not found".into(),
                    std::io::ErrorKind::PermissionDenied => "permission denied".into(),
                    _ => io_err.to_string().into_boxed_str(),
                },
                _ => source.to_string().into_boxed_str(),
            },
        })?;
    Ok(())
}

fn write_flac(path: &Path, samples: &[i32]) -> Result<(), OfflineRenderError> {
    let encoder = Encoder::default()
        .into_verified()
        .map_err(|(_config, error)| {
            OfflineRenderError::FlacConfig(error.to_string().into_boxed_str())
        })?;
    let source = MemSource::from_samples(
        samples,
        usize::from(OFFLINE_CHANNELS),
        usize::from(PCM_BITS_PER_SAMPLE),
        usize::try_from(DEFAULT_SAMPLE_RATE)
            .unwrap_or_else(|_| panic!("sample rate should fit into usize")),
    );
    let stream = flacenc::encode_with_fixed_block_size(&encoder, source, FLAC_BLOCK_SIZE)
        .map_err(|error| OfflineRenderError::FlacEncode(error.to_string().into_boxed_str()))?;
    let mut sink = ByteSink::new();
    stream
        .write(&mut sink)
        .map_err(|error| OfflineRenderError::FlacEncode(error.to_string().into_boxed_str()))?;
    fs::write(path, sink.as_slice()).map_err(|source| OfflineRenderError::Io {
        path: path.display().to_string().into_boxed_str(),
        message: match source.kind() {
            std::io::ErrorKind::NotFound => "file not found".into(),
            std::io::ErrorKind::PermissionDenied => "permission denied".into(),
            _ => source.to_string().into_boxed_str(),
        },
    })?;
    Ok(())
}

fn activate_voice(
    active_voices: &mut [Option<ActiveVoice>],
    sample_bank: &SampleBank,
    trigger: &SampleTrigger,
    fallback_voice: Option<VoiceKind>,
    sample_rate: u32,
) -> Result<(), OfflineRenderError> {
    if let Some(slot) = active_voices.iter_mut().find(|slot| slot.is_none()) {
        *slot = Some(
            if let Some((sample, resolved_trigger)) = sample_bank.resolve_trigger(trigger) {
                ActiveVoice::from_sample(sample, sample_rate, &resolved_trigger)
            } else if let Some(voice) = fallback_voice {
                ActiveVoice::new_with_pan(voice, sample_rate, trigger.pan())
            } else {
                return Err(OfflineRenderError::UnknownSampleToken(
                    trigger.token().into(),
                ));
            },
        );
    }

    Ok(())
}

fn mix_voices(active_voices: &mut [Option<ActiveVoice>]) -> (f32, f32) {
    let mut left = 0.0_f32;
    let mut right = 0.0_f32;
    for slot in active_voices {
        if let Some(voice) = slot.as_mut() {
            if let Some((voice_left, voice_right)) = voice.next_stereo_frame() {
                left += voice_left;
                right += voice_right;
            } else {
                *slot = None;
            }
        }
    }
    (left.clamp(-1.0, 1.0), right.clamp(-1.0, 1.0))
}

#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn float_to_pcm16(sample: f32) -> i16 {
    let scaled = (sample.clamp(-1.0, 1.0) * f32::from(i16::MAX)).round();
    scaled as i16
}

fn extension(path: &Path) -> Option<String> {
    path.extension()
        .and_then(|extension| extension.to_str())
        .map(str::to_ascii_lowercase)
}
