//! NES-authentic chiptune tone-generator leaf nodes.
//!
//! Ported from `madmax983/nes` `crates/nes-core/src/apu.rs` (branch `trunk`):
//! the pulse duty sequencer, the triangle staircase, and the 15-bit noise LFSR.
//! Only the channel *cores* are ported — the `DUTY_TABLE`/`TRIANGLE_TABLE`
//! constant tables, the duty-step advance, the triangle sequence advance, and
//! the LFSR tap/feedback rule. All emulator plumbing (MMIO register decode,
//! frame sequencer/IRQ, CPU clocking/resampling, DMC/DMA, the nonlinear mixer,
//! and the output filter chain) is intentionally dropped. Parameters that were
//! register writes in the APU (duty, volume, mode) are exposed here as signal
//! inputs per the Faust params-as-signals model (ADR 0004), and the CPU-clocked
//! timers are replaced by the graph's own per-sample advance at `sample_rate_hz`.
//!
//! The lo-fi character is deliberate: these are hard, non-band-limited sources
//! (the aliasing *is* the timbre) that sit beside — not in place of — the clean,
//! polyBLEP oscillators in `synth/osc.rs`. Levels are the raw NES 0..15 values
//! mapped to `level / 15.0`, so silence is `0.0` and the duty / staircase edges
//! are preserved exactly.

use super::node::Node;
use crate::synth::PhaseAccumulator;

// ---------------------------------------------------------------------------
// NES constant tables (ported verbatim from apu.rs)
// ---------------------------------------------------------------------------

/// The four 8-step pulse duty patterns: 12.5% / 25% / 50% / 25%-negated.
///
/// Ported from `apu.rs` `DUTY_TABLE`. Each row is one full waveform period; a
/// `1` emits the channel volume and a `0` emits silence — a hard two-level
/// square with no band-limiting.
const DUTY_TABLE: [[u8; 8]; 4] = [
    [0, 1, 0, 0, 0, 0, 0, 0], // 12.5%
    [0, 1, 1, 0, 0, 0, 0, 0], // 25%
    [0, 1, 1, 1, 1, 0, 0, 0], // 50%
    [1, 0, 0, 1, 1, 1, 1, 1], // 25% negated
];

/// The 32-step triangle staircase (15 → 0 → 0 → 15).
///
/// Ported from `apu.rs` `TRIANGLE_TABLE`. Only 16 distinct 4-bit levels appear;
/// the visible quantization staircase is the signature NES triangle timbre.
const TRIANGLE_TABLE: [u8; 32] = [
    15, 14, 13, 12, 11, 10, 9, 8, 7, 6, 5, 4, 3, 2, 1, 0, 0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12,
    13, 14, 15,
];

/// The number of duty steps in one pulse waveform period.
const PULSE_STEPS: f32 = 8.0;

/// The number of sequence steps in one triangle waveform period.
const TRIANGLE_STEPS: f32 = 32.0;

/// The maximum 4-bit level; NES channel amplitudes are `0..=15`.
const MAX_LEVEL: f32 = 15.0;

/// The 2A03 CPU clock, in Hz. Used only to reproduce the triangle's
/// ultrasonic-mute guard (`timer_reload < 2`); pitch itself is driven directly
/// by the `freq_hz` input, not by this clock.
const NES_CPU_HZ: f32 = 1_789_773.0;

/// Falls back to 48 kHz for non-finite or non-positive sample rates, mirroring
/// the other leaf nodes.
fn sanitize_sample_rate(sample_rate_hz: f32) -> f32 {
    if sample_rate_hz.is_finite() && sample_rate_hz > 0.0 {
        sample_rate_hz
    } else {
        48_000.0
    }
}

/// Quantizes a `0.0..=1.0` volume request to the NES 4-bit grid, returning the
/// integer level `0..=15` as `f32`. Non-finite requests read as silence.
fn quantize_volume_level(volume: f32) -> f32 {
    if volume.is_finite() {
        (volume.clamp(0.0, 1.0) * MAX_LEVEL).round()
    } else {
        0.0
    }
}

/// Advances the 15-bit noise LFSR one step, ported bit-for-bit from `apu.rs`
/// `NoiseChannel::clock_timer`.
///
/// `short_mode` selects the feedback tap: bit 6 (short, period 93) versus bit 1
/// (long, period 32767). The feedback bit is `bit0 XOR bit_tap`, the register
/// shifts right one place, and the feedback is inserted at bit 14. Starting from
/// the hardware reset value `1`, this reproduces the exact 2A03 noise stream.
#[must_use]
pub const fn advance_lfsr(shift_register: u16, short_mode: bool) -> u16 {
    let tap = if short_mode { 6 } else { 1 };
    let feedback = (shift_register & 0x0001) ^ ((shift_register >> tap) & 0x0001);
    (shift_register >> 1) | (feedback << 14)
}

// ---------------------------------------------------------------------------
// PulseNesNode
// ---------------------------------------------------------------------------

/// NES pulse (square) channel core.
/// 3 inputs (freq\_hz, duty, volume), 1 output (audio).
///
/// `duty` is the pattern index `0..=3` (rounded and clamped); `volume` is a
/// `0.0..=1.0` request quantized to the NES 4-bit grid. The output is a hard
/// two-level square — `duty_bit ? volume : 0` — with **no band-limiting**, so
/// silence is `0.0` and the single non-zero level is the quantized volume.
#[derive(Debug, Clone)]
pub struct PulseNesNode {
    phase: PhaseAccumulator,
    sample_rate_hz: f32,
}

impl Node for PulseNesNode {
    fn inputs(&self) -> u32 {
        3
    }
    fn outputs(&self) -> u32 {
        1
    }
    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        clippy::cast_precision_loss
    )]
    fn process(&mut self, inputs: &[&[f32]], outputs: &mut [&mut [f32]], frames: usize) {
        let freq = inputs[0];
        let duty = inputs[1];
        let volume = inputs[2];
        let out = &mut outputs[0];
        for i in 0..frames {
            // `phase` in [0, 1) selects the current 0..7 duty step.
            let step = ((self.phase.phase() * PULSE_STEPS) as usize).min(7);
            let duty_idx = if duty[i].is_finite() {
                duty[i].round().clamp(0.0, 3.0) as usize
            } else {
                0
            };
            let level = if DUTY_TABLE[duty_idx][step] == 0 {
                0.0
            } else {
                quantize_volume_level(volume[i])
            };
            out[i] = level / MAX_LEVEL;

            let f = freq[i];
            let increment = if f.is_finite() && f > 0.0 {
                f / self.sample_rate_hz
            } else {
                0.0
            };
            self.phase.advance(increment);
        }
    }
    fn reset(&mut self) {
        self.phase.reset();
    }
}

/// Creates a NES pulse channel node.
/// 3 inputs (freq\_hz, duty, volume), 1 output.
///
/// Non-finite or non-positive sample rates fall back to 48 kHz.
#[must_use]
pub fn pulse_nes(sample_rate_hz: f32) -> PulseNesNode {
    PulseNesNode {
        phase: PhaseAccumulator::new(),
        sample_rate_hz: sanitize_sample_rate(sample_rate_hz),
    }
}

// ---------------------------------------------------------------------------
// TriNesNode
// ---------------------------------------------------------------------------

/// NES triangle channel core.
/// 1 input (freq\_hz), 1 output (audio).
///
/// Emits the 32-step, 16-level `TRIANGLE_TABLE` staircase at fixed amplitude —
/// the hardware has **no volume control**. Reproduces the ultrasonic-mute guard:
/// when the equivalent 11-bit timer would be `< 2` (roughly `freq > 18.6 kHz`)
/// the channel is silenced, matching the `timer_reload < 2` check in `apu.rs`.
#[derive(Debug, Clone)]
pub struct TriNesNode {
    phase: PhaseAccumulator,
    sample_rate_hz: f32,
}

impl Node for TriNesNode {
    fn inputs(&self) -> u32 {
        1
    }
    fn outputs(&self) -> u32 {
        1
    }
    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        clippy::cast_precision_loss
    )]
    fn process(&mut self, inputs: &[&[f32]], outputs: &mut [&mut [f32]], frames: usize) {
        let freq = inputs[0];
        let out = &mut outputs[0];
        for i in 0..frames {
            let f = freq[i];
            let active = f.is_finite() && f > 0.0;
            // Reproduce the hardware ultrasonic mute: timer = CPU/(32*f) - 1 < 2.
            let muted = if active {
                (NES_CPU_HZ / (TRIANGLE_STEPS * f) - 1.0) < 2.0
            } else {
                true
            };
            let step = ((self.phase.phase() * TRIANGLE_STEPS) as usize).min(31);
            out[i] = if muted {
                0.0
            } else {
                f32::from(TRIANGLE_TABLE[step]) / MAX_LEVEL
            };

            if active {
                self.phase.advance(f / self.sample_rate_hz);
            }
        }
    }
    fn reset(&mut self) {
        self.phase.reset();
    }
}

/// Creates a NES triangle channel node.
/// 1 input (freq\_hz), 1 output.
///
/// Non-finite or non-positive sample rates fall back to 48 kHz.
#[must_use]
pub fn tri_nes(sample_rate_hz: f32) -> TriNesNode {
    TriNesNode {
        phase: PhaseAccumulator::new(),
        sample_rate_hz: sanitize_sample_rate(sample_rate_hz),
    }
}

// ---------------------------------------------------------------------------
// NoiseNesNode
// ---------------------------------------------------------------------------

/// NES noise channel core.
/// 3 inputs (mode, freq\_hz, volume), 1 output (audio).
///
/// `mode` selects the LFSR tap — `0` is long/tonal (15-bit, period 32767) and
/// non-zero is short/metallic (period 93). `freq_hz` is the LFSR advance rate;
/// `volume` is quantized to the NES 4-bit grid. Output is `volume` when the
/// register's low bit is `0`, else silence, matching `apu.rs`. The register is
/// seeded to the hardware reset value `1`.
#[derive(Debug, Clone)]
pub struct NoiseNesNode {
    shift_register: u16,
    /// Fractional-advance accumulator: the LFSR steps once per whole unit.
    accumulator: f32,
    sample_rate_hz: f32,
}

impl Node for NoiseNesNode {
    fn inputs(&self) -> u32 {
        3
    }
    fn outputs(&self) -> u32 {
        1
    }
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    fn process(&mut self, inputs: &[&[f32]], outputs: &mut [&mut [f32]], frames: usize) {
        let mode = inputs[0];
        let freq = inputs[1];
        let volume = inputs[2];
        let out = &mut outputs[0];
        for i in 0..frames {
            let short_mode = mode[i].is_finite() && mode[i].abs() >= 0.5;
            let f = freq[i];
            if f.is_finite() && f > 0.0 {
                self.accumulator += f / self.sample_rate_hz;
                // How many whole LFSR steps to take this sample; the fraction
                // carries forward. Avoids a float-comparison loop.
                let whole = self.accumulator.floor();
                self.accumulator -= whole;
                for _ in 0..(whole as usize) {
                    self.shift_register = advance_lfsr(self.shift_register, short_mode);
                }
            }
            let level = if self.shift_register & 0x0001 == 0 {
                quantize_volume_level(volume[i])
            } else {
                0.0
            };
            out[i] = level / MAX_LEVEL;
        }
    }
    fn reset(&mut self) {
        self.shift_register = 1;
        self.accumulator = 0.0;
    }
}

/// Creates a NES noise channel node.
/// 3 inputs (mode, freq\_hz, volume), 1 output.
///
/// Non-finite or non-positive sample rates fall back to 48 kHz.
#[must_use]
pub fn noise_nes(sample_rate_hz: f32) -> NoiseNesNode {
    NoiseNesNode {
        shift_register: 1,
        accumulator: 0.0,
        sample_rate_hz: sanitize_sample_rate(sample_rate_hz),
    }
}
