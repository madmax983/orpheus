//! Sample playback as a composable graph node.
//!
//! Closes the sample-playback item on the Faust-parity roadmap: samples and
//! synthesis previously lived in separate worlds (sample voices in
//! `engine.rs`, graph voices in `graph_voice.rs`), so a preloaded buffer
//! could not pass through a graph's filter/envelope/feedback chain. The
//! [`SamplePlayerNode`] makes a sample-bank buffer an ordinary [`Node`],
//! in four flavors: the one-shot [`sample_player`], the hard-wrapping
//! [`sample_player_looped`], the click-free
//! [`sample_player_looped_crossfaded`], and the note-tracking
//! [`sample_player_pitched`].

use std::sync::Arc;

use super::node::Node;
use crate::sample_bank::PlaybackSample;
use crate::voice::DEFAULT_ANALOG_BASE_FREQUENCY_HZ;

/// A player over a preloaded, shared sample buffer.
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
/// ignored — a falling gate does not stop the sample. In the default
/// one-shot mode playback ends at the buffer end; in loop mode
/// ([`sample_player_looped`]) the playhead hard-wraps to the buffer head
/// instead (no crossfade) and playback continues until the node is reset.
/// The crossfaded loop mode ([`sample_player_looped_crossfaded`]) blends the
/// loop tail into the head over a short window instead of hard-wrapping —
/// see that constructor for the fade semantics.
///
/// **The rate is a signal input** (params-as-signals, ADR 0004), read every
/// frame: 1.0 plays at the sample's native pitch (the playhead advances by
/// `source_rate / output_rate` source samples per output frame), 2.0 an
/// octave up in half the duration. In pitched mode
/// ([`sample_player_pitched`]) the rate input carries a frequency in Hertz
/// instead, divided by the construction-time reference frequency — the
/// division happens per frame, so a rate input exactly equal to the
/// reference plays at exactly 1.0. Fractional playhead positions read with
/// linear interpolation, interpolating toward silence past the final sample
/// (one-shot) or toward the buffer head (looped); non-positive or non-finite
/// rates hold the playhead in place.
#[derive(Debug, Clone)]
pub struct SamplePlayerNode {
    frames: Arc<[f32]>,
    /// Playhead advance per output frame at rate 1.0, in source samples
    /// (`source_rate / output_rate`).
    step_base: f64,
    /// The denominator applied to the rate input before stepping: 1.0 for
    /// plain rate semantics, the pitch reference in Hertz when the rate
    /// input carries the note frequency ([`sample_player_pitched`]).
    rate_reference: f64,
    /// Hard-wrap at the buffer end instead of stopping.
    looped: bool,
    /// Loop-wrap crossfade length in source samples; 0.0 keeps the hard
    /// wrap. Fixed at construction ([`LOOP_CROSSFADE_SECONDS`] of source
    /// material, capped at [`LOOP_CROSSFADE_MAX_BUFFER_FRACTION`] of the
    /// buffer), so the steady-state loop period is `len - fade`.
    loop_fade: f64,
    /// Playhead position in source samples, fractional.
    position: f64,
    playing: bool,
    prev_gate: f32,
}

/// Loop-crossfade length in seconds of source material (5 ms).
pub const LOOP_CROSSFADE_SECONDS: f64 = 0.005;

/// The loop crossfade never exceeds this fraction of the buffer (10%), so
/// short loops keep most of their material outside the fade.
pub const LOOP_CROSSFADE_MAX_BUFFER_FRACTION: f64 = 0.1;

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

            // `playing` guarantees position < len, so reads stay in range.
            let tail = self.read_at(self.position);
            #[allow(clippy::cast_precision_loss)]
            let period = len as f64 - self.loop_fade;
            out[i] = if self.loop_fade > 0.0 && self.position >= period {
                // Loop crossfade: over the last `loop_fade` source samples
                // the tail read blends LINEARLY (constant-gain) into the
                // matching read at the buffer head. Both reads come from the
                // same, correlated material, so an equal-gain fade preserves
                // amplitude (an equal-power fade would bump the correlated
                // sum by up to +3 dB mid-fade). Computed from the two read
                // positions on the fly — no fade buffer, no allocation.
                #[allow(clippy::cast_possible_truncation)]
                let t = ((self.position - period) / self.loop_fade) as f32;
                let head = self.read_at(self.position - period);
                tail.mul_add(1.0 - t, head * t)
            } else {
                tail
            };

            // In pitched mode the per-frame division maps the reference
            // frequency to exactly rate 1.0; plain mode divides by 1.0,
            // which leaves the requested rate bit-identical.
            let requested = f64::from(rate[i]) / self.rate_reference;
            let step = if requested.is_finite() && requested > 0.0 {
                requested * self.step_base
            } else {
                0.0
            };
            self.position += step;
            #[allow(clippy::cast_precision_loss)]
            if self.position >= len as f64 {
                if !self.looped {
                    self.playing = false;
                } else if self.loop_fade > 0.0 {
                    // Crossfaded wrap: during the fade the output blended
                    // toward the head read at `position - period`, so
                    // landing at `loop_fade + overshoot` keeps the output
                    // continuous; the modulo keeps over-length steps in
                    // range.
                    self.position = self.loop_fade + (self.position - len as f64) % period;
                } else {
                    // Hard-wrap, preserving the fractional phase (a step
                    // larger than the buffer wraps as many times as needed).
                    self.position %= len as f64;
                }
            }
        }
    }
    fn reset(&mut self) {
        self.position = 0.0;
        self.playing = false;
        self.prev_gate = 0.0;
    }
}

impl SamplePlayerNode {
    /// Linear-interpolated read at `pos` (in \[0, len)). Past the final
    /// sample the read interpolates toward the buffer head when looped,
    /// toward silence otherwise.
    fn read_at(&self, pos: f64) -> f32 {
        let len = self.frames.len();
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let index = pos as usize;
        #[allow(clippy::cast_possible_truncation, clippy::cast_precision_loss)]
        let frac = (pos - index as f64) as f32;
        let current = self.frames[index];
        let next = if index + 1 < len {
            self.frames[index + 1]
        } else if self.looped {
            self.frames[0]
        } else {
            0.0
        };
        frac.mul_add(next - current, current)
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
    sample_player_with_options(sample, output_sample_rate_hz, false, false, None)
}

/// Creates a looping sample player over a preloaded bank buffer.
/// 2 inputs (gate, rate), 1 output (audio).
///
/// Identical to [`sample_player`] except that the playhead hard-wraps to the
/// buffer head when it reaches the buffer end (no crossfade; at rate 1.0 the
/// output is the buffer tiled end to end, bit for bit). The gate keeps its
/// one-shot trigger semantics: a rising edge restarts from the top and a
/// falling gate does not stop playback — the loop sounds until the node is
/// reset (in a graph voice, until the note's release tail ends).
#[must_use]
pub fn sample_player_looped(
    sample: &PlaybackSample,
    output_sample_rate_hz: f32,
) -> SamplePlayerNode {
    sample_player_with_options(sample, output_sample_rate_hz, true, false, None)
}

/// Creates a looping sample player that crossfades the loop wrap.
/// 2 inputs (gate, rate), 1 output (audio).
///
/// Identical to [`sample_player_looped`] except that the loop tail blends
/// into the buffer head over a short LINEAR (constant-gain) crossfade —
/// [`LOOP_CROSSFADE_SECONDS`] (5 ms) of source material, capped at
/// [`LOOP_CROSSFADE_MAX_BUFFER_FRACTION`] (10%) of the buffer — so loops
/// that do not end on a zero crossing stop clicking at the wrap. Linear
/// (not equal-power) because the two reads come from the same correlated
/// material, where constant-gain blending preserves amplitude.
///
/// The head's first `fade` samples double as crossfade material, so the
/// steady-state loop period is `len - fade` source samples (the loop
/// re-enters at the fade's end, exactly where the blend landed — the output
/// stays continuous). Buffers too short for a whole fade sample (under 10
/// frames) fall back to the hard wrap. The blend is computed from the two
/// read positions on the fly: no fade buffer, no allocation, audio-thread
/// safe.
#[must_use]
pub fn sample_player_looped_crossfaded(
    sample: &PlaybackSample,
    output_sample_rate_hz: f32,
) -> SamplePlayerNode {
    sample_player_with_options(sample, output_sample_rate_hz, true, true, None)
}

/// Creates a one-shot sample player whose rate input is a frequency in
/// Hertz. 2 inputs (gate, freq\_hz), 1 output (audio).
///
/// The playback rate is `freq_hz / reference_hz`, computed per frame: a note
/// at the reference frequency plays at exactly native rate, an octave above
/// it at exactly 2.0. Non-finite or non-positive `reference_hz` values fall
/// back to [`DEFAULT_ANALOG_BASE_FREQUENCY_HZ`] (220 Hz), the engine's
/// rate-1.0 reference for note frequencies.
#[must_use]
pub fn sample_player_pitched(
    sample: &PlaybackSample,
    output_sample_rate_hz: f32,
    reference_hz: f32,
) -> SamplePlayerNode {
    sample_player_with_options(
        sample,
        output_sample_rate_hz,
        false,
        false,
        Some(reference_hz),
    )
}

/// The general sample-player constructor. 2 inputs (gate, rate), 1 output.
///
/// Backs [`sample_player`], [`sample_player_looped`],
/// [`sample_player_looped_crossfaded`], and [`sample_player_pitched`];
/// voice-spec lowering uses it directly so loop, crossfade, and pitch modes
/// compose in a spec. When `pitch_reference_hz` is set the rate input
/// carries a frequency in Hertz. `loop_crossfade` only applies when `looped`
/// is set; see [`sample_player_looped_crossfaded`] for the fade semantics.
#[must_use]
pub fn sample_player_with_options(
    sample: &PlaybackSample,
    output_sample_rate_hz: f32,
    looped: bool,
    loop_crossfade: bool,
    pitch_reference_hz: Option<f32>,
) -> SamplePlayerNode {
    let output_rate = if output_sample_rate_hz.is_finite() && output_sample_rate_hz > 0.0 {
        f64::from(output_sample_rate_hz)
    } else {
        48_000.0
    };
    let rate_reference = match pitch_reference_hz {
        Some(reference) if reference.is_finite() && reference > 0.0 => f64::from(reference),
        Some(_) => f64::from(DEFAULT_ANALOG_BASE_FREQUENCY_HZ),
        None => 1.0,
    };
    let loop_fade = if looped && loop_crossfade {
        // 5 ms of SOURCE material (the fade region lives in source-sample
        // coordinates), capped at 10% of the buffer; buffers too short for
        // one whole fade sample keep the hard wrap.
        let fade = (LOOP_CROSSFADE_SECONDS * f64::from(sample.sample_rate_hz())).round();
        #[allow(clippy::cast_precision_loss)]
        let cap = (sample.frames().len() as f64 * LOOP_CROSSFADE_MAX_BUFFER_FRACTION).floor();
        fade.min(cap).max(0.0)
    } else {
        0.0
    };
    SamplePlayerNode {
        frames: Arc::clone(sample.frames()),
        step_base: f64::from(sample.sample_rate_hz()) / output_rate,
        rate_reference,
        looped,
        loop_fade,
        position: 0.0,
        playing: false,
        prev_gate: 0.0,
    }
}
