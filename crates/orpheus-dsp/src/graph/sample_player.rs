//! One-shot sample playback as a composable graph node.
//!
//! Closes the sample-playback item on the Faust-parity roadmap: samples and
//! synthesis previously lived in separate worlds (sample voices in
//! `engine.rs`, graph voices in `graph_voice.rs`), so a preloaded buffer
//! could not pass through a graph's filter/envelope/feedback chain. The
//! [`SamplePlayerNode`] makes a sample-bank buffer an ordinary [`Node`].

use std::sync::Arc;

use super::node::Node;
use crate::sample_bank::PlaybackSample;

/// A one-shot player over a preloaded, shared sample buffer.
/// 2 inputs (gate, rate), 1 output (audio).
///
/// The buffer handle is resolved at construction — an `Arc` clone of the
/// sample bank's decoded mono frames (the bank downmixes stereo files at
/// load, so the node has exactly one output). `process()` only dereferences
/// that shared, immutable memory: no allocation, no locking, no I/O on the
/// audio thread.
///
/// **Trigger semantics are one-shot**, matching the engine's sample voices
/// and composing with ADR 0009's rectangular event-span gate: a RISING gate
/// edge restarts playback from the top, and the gate level is otherwise
/// ignored — a falling gate does not stop the sample, it plays to the buffer
/// end and goes silent. Playback ends at the buffer end (no looping).
///
/// **The rate is a signal input** (params-as-signals, ADR 0004), read every
/// frame: 1.0 plays at the sample's native pitch (the playhead advances by
/// `source_rate / output_rate` source samples per output frame), 2.0 an
/// octave up in half the duration. Fractional playhead positions read with
/// linear interpolation, interpolating toward silence past the final sample;
/// non-positive or non-finite rates hold the playhead in place.
#[derive(Debug, Clone)]
pub struct SamplePlayerNode {
    frames: Arc<[f32]>,
    /// Playhead advance per output frame at rate 1.0, in source samples
    /// (`source_rate / output_rate`).
    step_base: f64,
    /// Playhead position in source samples, fractional.
    position: f64,
    playing: bool,
    prev_gate: f32,
}

impl Node for SamplePlayerNode {
    fn inputs(&self) -> u32 {
        2
    }
    fn outputs(&self) -> u32 {
        1
    }
    fn process(&mut self, inputs: &[&[f32]], outputs: &mut [&mut [f32]], frames: usize) {
        let gate = inputs[0];
        let rate = inputs[1];
        let out = &mut outputs[0];
        let len = self.frames.len();
        for i in 0..frames {
            let gate_now = gate[i];
            // Rising edge (NaN gates compare false and stay inert).
            if gate_now > 0.0 && self.prev_gate <= 0.0 {
                self.position = 0.0;
                self.playing = len > 0;
            }
            self.prev_gate = gate_now;

            if !self.playing {
                out[i] = 0.0;
                continue;
            }

            // `playing` guarantees position < len, so the whole part is a
            // valid index.
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            let index = self.position as usize;
            #[allow(clippy::cast_possible_truncation, clippy::cast_precision_loss)]
            let frac = (self.position - index as f64) as f32;
            let current = self.frames[index];
            // Past the final sample, interpolate toward silence so the
            // output stays continuous for fractional rates.
            let next = if index + 1 < len {
                self.frames[index + 1]
            } else {
                0.0
            };
            out[i] = frac.mul_add(next - current, current);

            let requested = f64::from(rate[i]);
            let step = if requested.is_finite() && requested > 0.0 {
                requested * self.step_base
            } else {
                0.0
            };
            self.position += step;
            #[allow(clippy::cast_precision_loss)]
            if self.position >= len as f64 {
                self.playing = false;
            }
        }
    }
    fn reset(&mut self) {
        self.position = 0.0;
        self.playing = false;
        self.prev_gate = 0.0;
    }
}

/// Creates a one-shot sample player over a preloaded bank buffer.
/// 2 inputs (gate, rate), 1 output (audio).
///
/// Construction clones the buffer's `Arc` handle (resolve it off the audio
/// thread); rendering then reads the shared frames without allocating or
/// locking. `output_sample_rate_hz` is the rate the node renders at —
/// non-finite or non-positive values fall back to 48 kHz, matching the other
/// primitives.
#[must_use]
pub fn sample_player(sample: &PlaybackSample, output_sample_rate_hz: f32) -> SamplePlayerNode {
    let output_rate = if output_sample_rate_hz.is_finite() && output_sample_rate_hz > 0.0 {
        f64::from(output_sample_rate_hz)
    } else {
        48_000.0
    };
    SamplePlayerNode {
        frames: Arc::clone(sample.frames()),
        step_base: f64::from(sample.sample_rate_hz()) / output_rate,
        position: 0.0,
        playing: false,
        prev_gate: 0.0,
    }
}
