use std::path::Path;

use orpheus_pattern::Event;
use thiserror::Error;

use crate::engine::{DEFAULT_SAMPLE_RATE, DEFAULT_TEMPO_BPM, EngineError, frames_per_cycle};
use crate::sample_bank::SampleBank;
use crate::scheduler::Scheduler;
use crate::voice::ActiveVoice;

const OFFLINE_CHANNELS: u16 = 2;
const MAX_ACTIVE_VOICES: usize = 32;

/// Errors raised while rendering an offline WAV export.
#[derive(Debug, Error)]
pub enum OfflineRenderError {
    #[error("offline rendering requires at least one cycle")]
    InvalidCycleCount,
    #[error(transparent)]
    Engine(#[from] EngineError),
    #[error("failed to write wav file `{path}`: {source}")]
    Io {
        path: Box<str>,
        #[source]
        source: hound::Error,
    },
}

/// Renders explicit-time sample token events to a deterministic stereo WAV.
///
/// # Errors
///
/// Returns [`OfflineRenderError`] if event scheduling fails or the output file
/// cannot be written.
pub fn render_events_to_wav(
    path: impl AsRef<Path>,
    events: &[Event<Box<str>>],
    cycle_count: u64,
) -> Result<(), OfflineRenderError> {
    if cycle_count == 0 {
        return Err(OfflineRenderError::InvalidCycleCount);
    }

    let path = path.as_ref();
    let path_string = path.display().to_string();
    let spec = hound::WavSpec {
        channels: OFFLINE_CHANNELS,
        sample_rate: DEFAULT_SAMPLE_RATE,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut writer =
        hound::WavWriter::create(path, spec).map_err(|source| OfflineRenderError::Io {
            path: path_string.clone().into_boxed_str(),
            source,
        })?;

    let frames_per_cycle = frames_per_cycle(DEFAULT_SAMPLE_RATE, DEFAULT_TEMPO_BPM)?;
    let total_frames = frames_per_cycle
        .checked_mul(cycle_count)
        .ok_or(EngineError::FrameOverflow)?;
    let mut scheduler = Scheduler::default();
    scheduler.schedule_cycle_events(
        0,
        frames_per_cycle,
        events.iter().map(|event| Event {
            whole: event.whole.clone(),
            part: event.part.clone(),
            value: event.value.as_ref(),
        }),
    )?;

    let sample_bank = SampleBank::load_builtin();
    let mut active_voices = vec![None; MAX_ACTIVE_VOICES];

    for frame in 0..total_frames {
        while let Some(trigger) = scheduler.pop_due(frame) {
            activate_voice(
                &mut active_voices,
                &sample_bank,
                trigger.voice,
                DEFAULT_SAMPLE_RATE,
            );
        }

        let sample = mix_voices(&mut active_voices);
        let pcm = float_to_pcm16(sample);
        writer
            .write_sample(pcm)
            .map_err(|source| OfflineRenderError::Io {
                path: path_string.clone().into_boxed_str(),
                source,
            })?;
        writer
            .write_sample(pcm)
            .map_err(|source| OfflineRenderError::Io {
                path: path_string.clone().into_boxed_str(),
                source,
            })?;
    }

    writer.finalize().map_err(|source| OfflineRenderError::Io {
        path: path_string.into_boxed_str(),
        source,
    })?;
    Ok(())
}

fn activate_voice(
    active_voices: &mut [Option<ActiveVoice>],
    sample_bank: &SampleBank,
    voice: crate::VoiceKind,
    sample_rate: u32,
) {
    if let Some(slot) = active_voices.iter_mut().find(|slot| slot.is_none()) {
        *slot = Some(sample_bank.get(voice).map_or_else(
            || ActiveVoice::new(voice, sample_rate),
            |sample| ActiveVoice::from_sample(sample, sample_rate),
        ));
    }
}

fn mix_voices(active_voices: &mut [Option<ActiveVoice>]) -> f32 {
    let mut mixed = 0.0_f32;
    for slot in active_voices {
        if let Some(voice) = slot.as_mut() {
            if let Some(sample) = voice.next_sample() {
                mixed += sample;
            } else {
                *slot = None;
            }
        }
    }
    mixed.clamp(-1.0, 1.0)
}

#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn float_to_pcm16(sample: f32) -> i16 {
    let scaled = (sample.clamp(-1.0, 1.0) * f32::from(i16::MAX)).round();
    scaled as i16
}
