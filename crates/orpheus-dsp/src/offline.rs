//! The `offline` module provides non-real-time audio rendering.
//!
//! This module allows evaluated patterns to be rendered directly to audio files (like `.wav`)
//! as fast as the CPU allows, bypassing the real-time system audio callbacks. This is useful
//! for exporting bounces, offline testing, and generating static assets.

use std::fs;
use std::path::{Path, PathBuf};

use flacenc::bitsink::ByteSink;
use flacenc::component::BitRepr;
use flacenc::config::Encoder;
use flacenc::error::Verify;
use flacenc::source::MemSource;
use orpheus_pattern::Event;
use thiserror::Error;

use crate::SampleTrigger;
use crate::effects::BusEffectState;
use crate::engine::{DEFAULT_SAMPLE_RATE, DEFAULT_TEMPO_BPM, EngineError, frames_per_cycle};
use crate::routing::{RoutingSnapshot, TrackId, TrackSource};
use crate::sample_bank::SampleBank;
use crate::scheduler::{ScheduledTrigger, Scheduler};
use crate::voice::ActiveVoice;

const OFFLINE_CHANNELS: u16 = 2;
const MAX_ACTIVE_VOICES: usize = 32;
const PCM_BITS_PER_SAMPLE: u8 = 16;
const FLAC_BLOCK_SIZE: usize = 1024;

/// Errors raised while rendering an offline export.
#[derive(Debug, Error)]
pub enum OfflineRenderError {
    /// The renderer was requested to render fewer than 1 cycle.
    #[error("offline rendering requires at least one cycle")]
    InvalidCycleCount,
    /// An underlying error was returned by the audio engine during offline execution.
    #[error(transparent)]
    Engine(#[from] EngineError),
    /// The requested output file extension is not supported for offline rendering.
    #[error("unsupported render format `{0}`")]
    UnsupportedFormat(Box<str>),
    /// A general filesystem or IO error occurred during rendering.
    #[error("failed to write audio file `{path}`: {message}")]
    Io {
        /// The path where rendering failed.
        path: Box<str>,
        /// The IO error message.
        message: Box<str>,
    },
    /// A specific error occurred within the WAV encoding process.
    #[error("failed to write wav file `{path}`: {message}")]
    WavIo {
        /// The path where the WAV write failed.
        path: Box<str>,
        /// The WAV encoder error message.
        message: Box<str>,
    },
    /// The FLAC encoder encountered an invalid configuration.
    #[error("failed to verify FLAC encoder config: {0}")]
    FlacConfig(Box<str>),
    /// An error occurred while streaming frames into the FLAC encoder.
    #[error("failed to encode FLAC output: {0}")]
    FlacEncode(Box<str>),
    /// The offline renderer could not resolve a requested sample name.
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

/// Renders a validated routing snapshot into an interleaved stereo `f32` buffer
/// for deterministic tests.
///
/// This is intentionally test-oriented instead of user-facing export API: it
/// exercises the shared routing/effect path without baking a second file format
/// surface into the crate.
///
/// # Errors
///
/// Returns [`OfflineRenderError`] if scheduling or sample resolution fails.
#[doc(hidden)]
pub fn render_routing_snapshot_to_stereo_for_test(
    snapshot: &RoutingSnapshot,
    cycle_count: u64,
    tempo_bpm: f32,
    sample_bank: &SampleBank,
) -> Result<Vec<f32>, OfflineRenderError> {
    if cycle_count == 0 {
        return Err(OfflineRenderError::InvalidCycleCount);
    }

    let frames_per_cycle = frames_per_cycle(DEFAULT_SAMPLE_RATE, tempo_bpm)?;
    let total_frames = frames_per_cycle
        .checked_mul(cycle_count)
        .ok_or(EngineError::FrameOverflow)?;
    let total_frames_usize =
        usize::try_from(total_frames).map_err(|_| EngineError::FrameOverflow)?;
    let mut scheduler = Scheduler::default();
    schedule_snapshot_cycles(snapshot, cycle_count, frames_per_cycle, &mut scheduler)?;

    // Stack-allocated fixed-size array to avoid heap allocations during initialization,
    // ensuring deterministic memory usage for offline audio rendering.
    let mut active_voices: [Option<ActiveVoice>; MAX_ACTIVE_VOICES] = std::array::from_fn(|_| None);
    let mut track_mix_buffer = vec![(0.0_f32, 0.0_f32); snapshot.tracks().len()];
    let mut bus_mix_buffer = vec![(0.0_f32, 0.0_f32); snapshot.buses().len()];
    let mut bus_effect_states = snapshot
        .buses()
        .iter()
        .map(|bus| {
            bus.effect()
                .map(|effect| BusEffectState::from_spec(effect, frames_per_cycle))
                .transpose()
        })
        .collect::<Result<Vec<_>, _>>()?;
    let mut rendered = Vec::with_capacity(total_frames_usize * usize::from(OFFLINE_CHANNELS));

    for frame in 0..total_frames {
        activate_due_snapshot_voices(
            frame,
            frames_per_cycle,
            &mut scheduler,
            &mut active_voices,
            sample_bank,
        )?;
        let (master_left, master_right) = mix_snapshot_frame(
            snapshot,
            &mut active_voices,
            &mut track_mix_buffer,
            &mut bus_mix_buffer,
            &mut bus_effect_states,
        );

        rendered.push(master_left.clamp(-1.0, 1.0));
        rendered.push(master_right.clamp(-1.0, 1.0));
    }

    Ok(rendered)
}

/// Offline-renders a routing snapshot to per-track stem WAV files.
///
/// Track stems are written for bound, unmuted tracks. When `include_buses` is
/// true, bus stems are also written as post-effect stereo files.
///
/// Returns the file paths that were written.
///
/// # Errors
///
/// Returns [`OfflineRenderError`] if scheduling, rendering, sample resolution, or
/// file I/O fails.
///
/// # Panics
///
/// Panics if a track ID, a bus ID, or the sample rate cannot fit into a `usize`.
pub fn render_routing_snapshot_to_stem_wavs(
    snapshot: &RoutingSnapshot,
    cycle_count: u64,
    tempo_bpm: f32,
    sample_bank: &SampleBank,
    output_dir: impl AsRef<Path>,
    include_buses: bool,
) -> Result<Vec<PathBuf>, OfflineRenderError> {
    if cycle_count == 0 {
        return Err(OfflineRenderError::InvalidCycleCount);
    }

    let output_dir = output_dir.as_ref();
    fs::create_dir_all(output_dir).map_err(|source| OfflineRenderError::Io {
        path: output_dir.display().to_string().into_boxed_str(),
        message: source.to_string().into_boxed_str(),
    })?;

    let frames_per_cycle = frames_per_cycle(DEFAULT_SAMPLE_RATE, tempo_bpm)?;
    let total_frames = frames_per_cycle
        .checked_mul(cycle_count)
        .ok_or(EngineError::FrameOverflow)?;
    let total_frames_usize =
        usize::try_from(total_frames).map_err(|_| EngineError::FrameOverflow)?;
    let mut scheduler = Scheduler::default();
    schedule_snapshot_cycles(snapshot, cycle_count, frames_per_cycle, &mut scheduler)?;

    // Stack-allocated fixed-size array to avoid heap allocations during initialization,
    // ensuring deterministic memory usage for offline audio rendering.
    let mut active_voices: [Option<ActiveVoice>; MAX_ACTIVE_VOICES] = std::array::from_fn(|_| None);
    let mut track_mix_buffer = vec![(0.0_f32, 0.0_f32); snapshot.tracks().len()];
    let mut bus_mix_buffer = vec![(0.0_f32, 0.0_f32); snapshot.buses().len()];
    let mut track_stem_frame = vec![(0.0_f32, 0.0_f32); snapshot.tracks().len()];
    let mut bus_stem_frame = vec![(0.0_f32, 0.0_f32); snapshot.buses().len()];
    let mut bus_effect_states = snapshot
        .buses()
        .iter()
        .map(|bus| {
            bus.effect()
                .map(|effect| BusEffectState::from_spec(effect, frames_per_cycle))
                .transpose()
        })
        .collect::<Result<Vec<_>, _>>()?;

    let mut track_stems = snapshot
        .tracks()
        .iter()
        .map(|_| Vec::with_capacity(total_frames_usize * usize::from(OFFLINE_CHANNELS)))
        .collect::<Vec<_>>();
    let mut bus_stems = snapshot
        .buses()
        .iter()
        .map(|_| Vec::with_capacity(total_frames_usize * usize::from(OFFLINE_CHANNELS)))
        .collect::<Vec<_>>();

    for frame in 0..total_frames {
        activate_due_snapshot_voices(
            frame,
            frames_per_cycle,
            &mut scheduler,
            &mut active_voices,
            sample_bank,
        )?;
        mix_snapshot_frame_with_stems(
            snapshot,
            &mut active_voices,
            &mut track_mix_buffer,
            &mut bus_mix_buffer,
            &mut bus_effect_states,
            &mut track_stem_frame,
            &mut bus_stem_frame,
        );

        for (index, buffer) in track_stems.iter_mut().enumerate() {
            let (left, right) = track_stem_frame[index];
            buffer.push(i32::from(float_to_pcm16(left)));
            buffer.push(i32::from(float_to_pcm16(right)));
        }
        for (index, buffer) in bus_stems.iter_mut().enumerate() {
            let (left, right) = bus_stem_frame[index];
            buffer.push(i32::from(float_to_pcm16(left)));
            buffer.push(i32::from(float_to_pcm16(right)));
        }
    }

    let mut written_paths = Vec::new();
    for track in snapshot.tracks() {
        if track.source().is_unbound() || track.muted() {
            continue;
        }
        let track_index =
            usize::try_from(track.id().get()).unwrap_or_else(|_| panic!("track id should fit"));
        let stem_name = format!("{}.wav", sanitize_stem_name(track.name()));
        let stem_path = output_dir.join(stem_name);
        write_wav(&stem_path, &track_stems[track_index])?;
        written_paths.push(stem_path);
    }

    if include_buses {
        for bus in snapshot.buses() {
            if !bus.routes_to_master() {
                continue;
            }
            let bus_index =
                usize::try_from(bus.id().get()).unwrap_or_else(|_| panic!("bus id should fit"));
            let stem_name = format!("{}_bus.wav", sanitize_stem_name(bus.name()));
            let stem_path = output_dir.join(stem_name);
            write_wav(&stem_path, &bus_stems[bus_index])?;
            written_paths.push(stem_path);
        }
    }

    Ok(written_paths)
}

fn schedule_snapshot_cycles(
    snapshot: &RoutingSnapshot,
    cycle_count: u64,
    frames_per_cycle: u64,
    scheduler: &mut Scheduler,
) -> Result<(), OfflineRenderError> {
    for cycle in 0..cycle_count {
        let cycle_start = cycle
            .checked_mul(frames_per_cycle)
            .ok_or(EngineError::FrameOverflow)?;
        for track in snapshot.tracks() {
            if let TrackSource::SamplePattern(events) = track.source() {
                scheduler.schedule_cycle_events(
                    track.id(),
                    cycle_start,
                    frames_per_cycle,
                    events.iter().map(|event| Event {
                        whole: event.whole.clone(),
                        part: event.part.clone(),
                        value: &event.value,
                    }),
                )?;
            }
        }
    }

    Ok(())
}

fn activate_due_snapshot_voices(
    frame: u64,
    frames_per_cycle: u64,
    scheduler: &mut Scheduler,
    active_voices: &mut [Option<ActiveVoice>],
    sample_bank: &SampleBank,
) -> Result<(), OfflineRenderError> {
    while let Some(trigger) = scheduler.pop_due(frame) {
        activate_voice(
            active_voices,
            sample_bank,
            &trigger,
            DEFAULT_SAMPLE_RATE,
            frames_per_cycle,
        )?;
    }

    Ok(())
}

fn mix_snapshot_frame(
    snapshot: &RoutingSnapshot,
    active_voices: &mut [Option<ActiveVoice>],
    track_mix_buffer: &mut [(f32, f32)],
    bus_mix_buffer: &mut [(f32, f32)],
    bus_effect_states: &mut [Option<BusEffectState>],
) -> (f32, f32) {
    track_mix_buffer.fill((0.0, 0.0));
    bus_mix_buffer.fill((0.0, 0.0));
    mix_offline_voices_into_tracks(active_voices, track_mix_buffer);

    let mut master_left = 0.0_f32;
    let mut master_right = 0.0_f32;

    for track in snapshot.tracks() {
        let track_index = usize::try_from(track.id().get())
            .unwrap_or_else(|_| panic!("track id did not fit in usize"));
        let (track_left, track_right) = track_mix_buffer[track_index];
        let track_left = track_left * track.level();
        let track_right = track_right * track.level();

        if track.muted() {
            continue;
        }

        if track.routes_to_master() {
            master_left += track_left;
            master_right += track_right;
        }

        for send in track.sends() {
            let bus_index = usize::try_from(send.bus_id().get())
                .unwrap_or_else(|_| panic!("bus id did not fit in usize"));
            let (bus_left, bus_right) = &mut bus_mix_buffer[bus_index];
            *bus_left += track_left * send.level();
            *bus_right += track_right * send.level();
        }
    }

    for bus in snapshot.buses() {
        if !bus.routes_to_master() {
            continue;
        }

        let bus_index = usize::try_from(bus.id().get())
            .unwrap_or_else(|_| panic!("bus id did not fit in usize"));
        let (bus_left, bus_right) = bus_mix_buffer[bus_index];
        if let Some(effect) = bus_effect_states[bus_index].as_mut() {
            let (wet_left, wet_right) = effect.process_frame(bus_left, bus_right);
            master_left += wet_left;
            master_right += wet_right;
        } else {
            master_left += bus_left;
            master_right += bus_right;
        }
    }

    (master_left, master_right)
}

fn mix_snapshot_frame_with_stems(
    snapshot: &RoutingSnapshot,
    active_voices: &mut [Option<ActiveVoice>],
    track_mix_buffer: &mut [(f32, f32)],
    bus_mix_buffer: &mut [(f32, f32)],
    bus_effect_states: &mut [Option<BusEffectState>],
    track_stem_frame: &mut [(f32, f32)],
    bus_stem_frame: &mut [(f32, f32)],
) {
    track_mix_buffer.fill((0.0, 0.0));
    bus_mix_buffer.fill((0.0, 0.0));
    track_stem_frame.fill((0.0, 0.0));
    bus_stem_frame.fill((0.0, 0.0));
    mix_offline_voices_into_tracks(active_voices, track_mix_buffer);

    for track in snapshot.tracks() {
        let track_index = usize::try_from(track.id().get())
            .unwrap_or_else(|_| panic!("track id did not fit in usize"));
        let (track_left, track_right) = track_mix_buffer[track_index];
        let track_left = track_left * track.level();
        let track_right = track_right * track.level();

        if track.muted() {
            continue;
        }
        track_stem_frame[track_index] = (track_left, track_right);

        for send in track.sends() {
            let bus_index = usize::try_from(send.bus_id().get())
                .unwrap_or_else(|_| panic!("bus id did not fit in usize"));
            let (bus_left, bus_right) = &mut bus_mix_buffer[bus_index];
            *bus_left += track_left * send.level();
            *bus_right += track_right * send.level();
        }
    }

    for bus in snapshot.buses() {
        if !bus.routes_to_master() {
            continue;
        }

        let bus_index = usize::try_from(bus.id().get())
            .unwrap_or_else(|_| panic!("bus id did not fit in usize"));
        let (bus_left, bus_right) = bus_mix_buffer[bus_index];
        if let Some(effect) = bus_effect_states[bus_index].as_mut() {
            bus_stem_frame[bus_index] = effect.process_frame(bus_left, bus_right);
        } else {
            bus_stem_frame[bus_index] = (bus_left, bus_right);
        }
    }
}

fn mix_offline_voices_into_tracks(
    active_voices: &mut [Option<ActiveVoice>],
    track_mix_buffer: &mut [(f32, f32)],
) {
    for slot in active_voices {
        if let Some(voice) = slot.as_mut() {
            if let Some((left, right)) = voice.next_stereo_frame() {
                let track_index = usize::try_from(voice.track_id().get())
                    .unwrap_or_else(|_| panic!("track id did not fit in usize"));
                let (track_left, track_right) = &mut track_mix_buffer[track_index];
                *track_left += left;
                *track_right += right;
            } else {
                *slot = None;
            }
        }
    }
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
        TrackId::new(0),
        0,
        frames_per_cycle,
        events.iter().map(|event| Event {
            whole: event.whole.clone(),
            part: event.part.clone(),
            value: &event.value,
        }),
    )?;

    // Stack-allocated fixed-size array to avoid heap allocations during initialization,
    // ensuring deterministic memory usage for offline audio rendering.
    let mut active_voices: [Option<ActiveVoice>; MAX_ACTIVE_VOICES] = std::array::from_fn(|_| None);
    let mut rendered = Vec::with_capacity(total_samples);

    for frame in 0..total_frames {
        while let Some(trigger) = scheduler.pop_due(frame) {
            activate_voice(
                &mut active_voices,
                sample_bank,
                &trigger,
                DEFAULT_SAMPLE_RATE,
                frames_per_cycle,
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
    scheduled_trigger: &ScheduledTrigger,
    sample_rate: u32,
    frames_per_cycle: u64,
) -> Result<(), OfflineRenderError> {
    if let Some(slot) = active_voices.iter_mut().find(|slot| slot.is_none()) {
        *slot = Some(
            if let Some((sample, resolved_trigger)) =
                sample_bank.resolve_trigger(&scheduled_trigger.trigger)
            {
                ActiveVoice::from_sample(
                    scheduled_trigger.track_id,
                    sample,
                    sample_rate,
                    frames_per_cycle,
                    &resolved_trigger,
                )
            } else if let Some(voice) = scheduled_trigger.fallback_voice {
                ActiveVoice::from_trigger(
                    scheduled_trigger.track_id,
                    voice,
                    sample_rate,
                    frames_per_cycle,
                    &scheduled_trigger.trigger,
                    scheduled_trigger.duration_frames,
                )
            } else {
                return Err(OfflineRenderError::UnknownSampleToken(
                    scheduled_trigger.trigger.token().into(),
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

fn sanitize_stem_name(name: &str) -> String {
    let mut sanitized = String::with_capacity(name.len());
    for ch in name.chars() {
        if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
            sanitized.push(ch);
        } else {
            sanitized.push('_');
        }
    }
    let trimmed = sanitized.trim_matches('_');
    if trimmed.is_empty() {
        "stem".to_owned()
    } else {
        trimmed.to_owned()
    }
}
