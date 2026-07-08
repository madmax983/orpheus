//! Graph-backed engine voices (ADR 0004 follow-up, ADR 0009).
//!
//! This module wires the Faust-style graph combinator system into the render
//! engine's voice lifecycle. A [`GraphVoiceProgram`] describes a playable
//! stereo voice as a compiled graph [`Processor`] with the fixed interface
//!
//! ```text
//! inputs:  [gate, freq_hz, gain, pan]   (params as signals, ADR 0004)
//! outputs: [left, right]
//! ```
//!
//! Allocation discipline: programs are compiled into a fixed pool of
//! [`GraphVoice`]s at engine construction time (off the audio thread) and
//! warmed with [`GraphVoice::prepare`] so lazy-but-once scratch growth also
//! happens off-thread. Triggering and rendering a pooled voice performs no
//! heap allocation; finished voices are reset in place and reused instead of
//! being dropped on the audio thread.

use crate::SampleTrigger;
use crate::graph::{
    Node, Processor, adsr, bind, gain_node, pan, par, passthrough, seq, sine, wire,
};
use crate::routing::TrackId;

/// Pooled voices per program: how many simultaneous notes one graph program
/// can sound before further triggers are dropped.
const GRAPH_VOICE_POLYPHONY: usize = 8;

/// Output trim applied to graph voices, matching the analog-voice headroom
/// convention in `voice.rs`.
const GRAPH_OUTPUT_TRIM: f32 = 0.35;

/// A named, buildable graph voice program.
///
/// The builder function compiles a graph with the fixed voice interface:
/// 4 inputs (gate, freq\_hz, gain, pan) and 2 outputs (left, right).
#[derive(Clone, Copy, Debug)]
pub struct GraphVoiceProgram {
    token: &'static str,
    release_seconds: f32,
    build: fn(f32) -> Processor,
}

impl GraphVoiceProgram {
    /// The pattern-token this program is selected by (e.g. `"gsine"`).
    #[must_use]
    pub const fn token(&self) -> &'static str {
        self.token
    }

    /// How long the voice keeps sounding after its gate falls, in seconds.
    #[must_use]
    pub const fn release_seconds(&self) -> f32 {
        self.release_seconds
    }

    /// Compiles the program into an unprepared [`GraphVoice`].
    ///
    /// Construction may allocate; call it off the audio thread and follow up
    /// with [`GraphVoice::prepare`] before real-time use.
    #[must_use]
    pub fn build_voice(&self, sample_rate_hz: f32) -> GraphVoice {
        GraphVoice {
            processor: (self.build)(sample_rate_hz),
        }
    }

    /// The release tail length in frames at `sample_rate_hz`.
    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        clippy::cast_precision_loss
    )]
    fn release_frames(&self, sample_rate_hz: f32) -> u32 {
        let frames = (self.release_seconds * sample_rate_hz).ceil();
        if frames.is_finite() && frames > 0.0 {
            frames.min(u32::MAX as f32) as u32
        } else {
            1
        }
    }
}

/// The built-in graph voice programs available to the engine.
///
/// Currently one program:
///
/// - `gsine` — the gated sine voice from the graph integration suite:
///   sine carrier multiplied by a gate-driven ADSR envelope, scaled by the
///   trigger gain, and placed in the stereo field by an equal-power panner.
#[must_use]
pub const fn builtin_graph_voice_programs() -> &'static [GraphVoiceProgram] {
    const PROGRAMS: [GraphVoiceProgram; 1] = [GraphVoiceProgram {
        token: "gsine",
        release_seconds: GSINE_RELEASE_SECONDS,
        build: build_gsine,
    }];
    &PROGRAMS
}

const GSINE_ATTACK_SECONDS: f32 = 0.001;
const GSINE_DECAY_SECONDS: f32 = 0.01;
const GSINE_SUSTAIN_LEVEL: f32 = 0.7;
const GSINE_RELEASE_SECONDS: f32 = 0.02;

/// Builds the `gsine` voice graph: `[gate, freq, gain, pan] -> [L, R]`.
fn build_gsine(sample_rate_hz: f32) -> Processor {
    let envelope = bind(
        adsr(sample_rate_hz),
        &[
            (1, GSINE_ATTACK_SECONDS),
            (2, GSINE_DECAY_SECONDS),
            (3, GSINE_SUSTAIN_LEVEL),
            (4, GSINE_RELEASE_SECONDS),
        ],
    )
    .unwrap_or_else(|error| panic!("gsine envelope bindings must be valid: {error}"));

    // [gate, freq, gain, pan] -> [freq, gate, gain, pan]
    let reorder = wire(&[1, 0, 2, 3]);
    // -> [osc, level, gain, pan]
    let sources = par(sine(sample_rate_hz), par(envelope, passthrough(2)));
    // -> [osc * level, gain, pan]
    let enveloped = par(gain_node(), passthrough(2));
    // -> [audio * gain, pan]
    let levelled = par(gain_node(), passthrough(1));
    // -> [left, right]
    let stereo = pan();

    let graph = seq(
        reorder,
        seq(
            sources,
            seq(
                enveloped,
                seq(levelled, stereo)
                    .unwrap_or_else(|error| panic!("gsine pan stage must compose: {error}")),
            )
            .unwrap_or_else(|error| panic!("gsine gain stage must compose: {error}")),
        )
        .unwrap_or_else(|error| panic!("gsine envelope stage must compose: {error}")),
    )
    .unwrap_or_else(|error| panic!("gsine input reorder must compose: {error}"));

    debug_assert_eq!(graph.inputs(), 4);
    debug_assert_eq!(graph.outputs(), 2);
    Processor::new(graph)
}

/// A compiled, per-note instance of a graph voice program.
///
/// The engine owns these in a fixed pool. All allocation happens in
/// [`GraphVoiceProgram::build_voice`] and the first block of
/// [`Self::prepare`]; after that, [`Self::process_frame`] and [`Self::reset`]
/// are allocation-free and lock-free.
#[derive(Debug)]
pub struct GraphVoice {
    processor: Processor,
}

impl GraphVoice {
    /// Warms the processor so lazy-but-once scratch buffers are grown off the
    /// audio thread, then resets all state.
    ///
    /// The engine renders one frame at a time, so a single one-frame block is
    /// sufficient to reach every scratch buffer in the graph.
    pub fn prepare(&mut self) {
        let _ = self.process_frame(0.0, 0.0, 0.0, 0.0);
        self.reset();
    }

    /// Renders one stereo frame with the given control values.
    ///
    /// Parameters flow as signals (ADR 0004): the gate, frequency, gain, and
    /// pan are one-frame input channels. This path must stay allocation-free.
    pub fn process_frame(&mut self, gate: f32, freq_hz: f32, gain: f32, pan: f32) -> (f32, f32) {
        let gate_buf = [gate];
        let freq_buf = [freq_hz];
        let gain_buf = [gain];
        let pan_buf = [pan];
        let inputs: [&[f32]; 4] = [&gate_buf, &freq_buf, &gain_buf, &pan_buf];
        let mut left = [0.0_f32];
        let mut right = [0.0_f32];
        {
            let mut outputs: [&mut [f32]; 2] = [&mut left, &mut right];
            self.processor.process(&inputs, &mut outputs, 1);
        }
        (left[0], right[0])
    }

    /// Resets all internal DSP state without releasing buffers.
    pub fn reset(&mut self) {
        self.processor.reset();
    }
}

/// One currently sounding note on a pooled graph voice.
#[derive(Clone, Copy, Debug)]
struct GraphVoiceNote {
    track_id: TrackId,
    gate_frames_remaining: u32,
    release_frames_remaining: u32,
    freq_hz: f32,
    gain: f32,
    pan: f32,
}

#[derive(Debug)]
struct GraphVoiceSlot {
    token: &'static str,
    release_frames: u32,
    voice: GraphVoice,
    note: Option<GraphVoiceNote>,
}

/// A fixed pool of prepared graph voices owned by the engine core.
///
/// Built once at engine construction (off the audio thread); triggering and
/// rendering never allocate.
#[derive(Debug)]
pub struct GraphVoiceBank {
    slots: Vec<GraphVoiceSlot>,
}

impl GraphVoiceBank {
    /// Builds and prepares the pool for every built-in program.
    pub fn with_builtin_programs(sample_rate_hz: f32) -> Self {
        let mut slots = Vec::new();
        for program in builtin_graph_voice_programs() {
            let release_frames = program.release_frames(sample_rate_hz);
            for _ in 0..GRAPH_VOICE_POLYPHONY {
                let mut voice = program.build_voice(sample_rate_hz);
                voice.prepare();
                slots.push(GraphVoiceSlot {
                    token: program.token(),
                    release_frames,
                    voice,
                    note: None,
                });
            }
        }
        Self { slots }
    }

    /// Whether `token` names a pooled graph voice program.
    pub fn has_program(&self, token: &str) -> bool {
        self.slots.iter().any(|slot| slot.token == token)
    }

    /// Starts a note on an idle pooled voice for `token`.
    ///
    /// Returns `false` (dropping the trigger) when the token names no program
    /// or every pooled voice for it is already sounding. Never allocates.
    pub fn trigger(
        &mut self,
        token: &str,
        track_id: TrackId,
        gate_frames: u32,
        freq_hz: f32,
        gain: f32,
        pan: f32,
    ) -> bool {
        let Some(slot) = self
            .slots
            .iter_mut()
            .find(|slot| slot.token == token && slot.note.is_none())
        else {
            return false;
        };

        slot.note = Some(GraphVoiceNote {
            track_id,
            gate_frames_remaining: gate_frames.max(1),
            release_frames_remaining: slot.release_frames,
            freq_hz,
            gain,
            pan,
        });
        true
    }

    /// Renders one frame of every sounding voice into the per-track mix.
    ///
    /// Finished voices are reset in place and returned to the pool; nothing
    /// is allocated or dropped on this path.
    pub fn render_frame(&mut self, track_mix: &mut [(f32, f32)]) {
        for slot in &mut self.slots {
            let Some(note) = slot.note.as_mut() else {
                continue;
            };

            let gate = if note.gate_frames_remaining > 0 {
                1.0
            } else {
                0.0
            };
            let (left, right) = slot
                .voice
                .process_frame(gate, note.freq_hz, note.gain, note.pan);

            if let Ok(track_index) = usize::try_from(note.track_id.get())
                && let Some((mix_left, mix_right)) = track_mix.get_mut(track_index)
            {
                *mix_left += left;
                *mix_right += right;
            }

            if note.gate_frames_remaining > 0 {
                note.gate_frames_remaining -= 1;
            } else if note.release_frames_remaining > 0 {
                note.release_frames_remaining -= 1;
            } else {
                slot.voice.reset();
                slot.note = None;
            }
        }
    }

    /// Silences every sounding voice and returns it to the pool.
    pub fn stop_all(&mut self) {
        for slot in &mut self.slots {
            if slot.note.take().is_some() {
                slot.voice.reset();
            }
        }
    }
}

/// Derives the graph voice control values from a scheduled trigger.
///
/// Frequency follows the analog-voice convention: the engine reference
/// frequency scaled by the trigger's playback rate. Gain applies the shared
/// output trim; pan is clamped to the stereo field.
#[allow(clippy::cast_possible_truncation)]
pub fn graph_note_params(trigger: &SampleTrigger, base_hz: f32) -> (f32, f32, f32) {
    let rate = if trigger.rate().is_finite() {
        trigger.rate().abs() as f32
    } else {
        1.0
    };
    let freq_hz = (base_hz * rate).max(0.0);

    let gain = if trigger.gain().is_finite() {
        (trigger.gain() as f32).max(0.0)
    } else {
        1.0
    } * GRAPH_OUTPUT_TRIM;

    let pan = if trigger.pan().is_finite() {
        (trigger.pan() as f32).clamp(-1.0, 1.0)
    } else {
        0.0
    };

    (freq_hz, gain, pan)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SR: f32 = 48_000.0;

    fn track(id: u32) -> TrackId {
        TrackId::new(id)
    }

    #[test]
    fn builtin_programs_expose_gsine() {
        let programs = builtin_graph_voice_programs();
        assert!(programs.iter().any(|program| program.token() == "gsine"));
    }

    #[test]
    fn gsine_voice_produces_audio_while_gated_and_decays_after_release() {
        let program = &builtin_graph_voice_programs()[0];
        let mut voice = program.build_voice(SR);
        voice.prepare();

        let mut gated_energy = 0.0_f32;
        for _ in 0..2_400 {
            let (left, right) = voice.process_frame(1.0, 220.0, 0.5, 0.0);
            gated_energy += left.abs() + right.abs();
        }
        assert!(gated_energy > 1.0, "gated voice should be audible");

        // Run the release out (0.02 s = 960 frames), then check silence.
        for _ in 0..2_400 {
            let _ = voice.process_frame(0.0, 220.0, 0.5, 0.0);
        }
        let (left, right) = voice.process_frame(0.0, 220.0, 0.5, 0.0);
        assert!(left.abs() < 1e-4 && right.abs() < 1e-4);
    }

    #[test]
    fn bank_trigger_respects_polyphony_and_unknown_tokens() {
        let mut bank = GraphVoiceBank::with_builtin_programs(SR);
        assert!(bank.has_program("gsine"));
        assert!(!bank.has_program("bd"));
        assert!(!bank.trigger("bd", track(0), 10, 220.0, 0.5, 0.0));

        for _ in 0..GRAPH_VOICE_POLYPHONY {
            assert!(bank.trigger("gsine", track(0), 10, 220.0, 0.5, 0.0));
        }
        assert!(
            !bank.trigger("gsine", track(0), 10, 220.0, 0.5, 0.0),
            "the pool is exhausted, the trigger must be dropped"
        );
    }

    #[test]
    fn bank_returns_finished_voices_to_the_pool() {
        let mut bank = GraphVoiceBank::with_builtin_programs(SR);
        assert!(bank.trigger("gsine", track(0), 2, 220.0, 0.5, 0.0));

        let mut mix = vec![(0.0_f32, 0.0_f32); 1];
        // 2 gate frames + release frames + the final reclaim frame.
        let program = &builtin_graph_voice_programs()[0];
        let lifetime = 2 + program.release_frames(SR) + 1;
        for _ in 0..lifetime {
            bank.render_frame(&mut mix);
        }

        for _ in 0..GRAPH_VOICE_POLYPHONY {
            assert!(bank.trigger("gsine", track(0), 2, 220.0, 0.5, 0.0));
        }
    }

    #[test]
    fn render_frame_ignores_out_of_range_tracks() {
        let mut bank = GraphVoiceBank::with_builtin_programs(SR);
        assert!(bank.trigger("gsine", track(7), 4, 220.0, 0.5, 0.0));
        let mut mix = vec![(0.0_f32, 0.0_f32); 1];
        bank.render_frame(&mut mix);
        assert_eq!(mix[0], (0.0, 0.0));
    }

    #[test]
    fn graph_note_params_follow_trigger_fields() {
        let trigger = SampleTrigger::named("gsine")
            .with_rate(2.0)
            .with_gain(1.0)
            .with_pan(-0.5);
        let (freq_hz, gain, pan) = graph_note_params(&trigger, 220.0);
        assert!((freq_hz - 440.0).abs() < 1e-3);
        assert!((gain - GRAPH_OUTPUT_TRIM).abs() < 1e-6);
        assert!((pan - -0.5).abs() < 1e-6);
    }
}
