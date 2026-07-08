//! Multi-mode filter nodes for the graph combinator system: a TPT
//! state-variable filter ([`SvfNode`]) and an RBJ-cookbook biquad
//! ([`BiquadNode`]).
//!
//! Both follow the params-as-signals convention (ADR 0004): cutoff/center
//! frequency, Q, and gain arrive as input channels, not configuration fields.
//! They differ in how often those signals are honored — the SVF recomputes
//! its coefficients every sample (audio-rate modulation safe), the biquad
//! once per block (cheap, block-rate stepping under fast modulation).

use std::f32::consts::{FRAC_1_SQRT_2, PI, TAU};

use super::node::Node;

/// Highest cutoff/center frequency as a fraction of the sample rate.
///
/// Kept strictly below Nyquist so the TPT prewarp `tan(pi * fc / sr)` and the
/// RBJ `sin`/`cos` terms stay well-conditioned.
const MAX_FREQUENCY_RATIO: f32 = 0.49;

/// Lowest accepted cutoff/center frequency in Hertz.
///
/// Strictly positive so the biquad coefficient formulas never degenerate
/// (`alpha = 0` would place the poles exactly on the unit circle).
const MIN_FREQUENCY_HZ: f32 = 1.0;

/// Q clamp bounds shared by both filters. Anywhere inside these bounds the
/// pole radius stays strictly inside the unit circle.
const MIN_Q: f32 = 0.05;
const MAX_Q: f32 = 100.0;

/// Peaking gain clamp in decibels (either direction).
const MAX_GAIN_DB: f32 = 40.0;

/// Clamps `value` to `[lo, hi]`, falling back to `fallback` for non-finite
/// input (`f32::clamp` propagates NaN).
const fn sanitize(value: f32, lo: f32, hi: f32, fallback: f32) -> f32 {
    if value.is_finite() {
        value.clamp(lo, hi)
    } else {
        fallback
    }
}

/// Falls back to 48 kHz for non-finite or non-positive sample rates.
fn sanitize_sample_rate(sample_rate_hz: f32) -> f32 {
    if sample_rate_hz.is_finite() && sample_rate_hz > 0.0 {
        sample_rate_hz
    } else {
        48_000.0
    }
}

// ---------------------------------------------------------------------------
// SvfNode
// ---------------------------------------------------------------------------

/// A state-variable filter with four simultaneous responses.
/// 3 inputs (audio, cutoff\_hz, q), 4 outputs (lowpass, highpass, bandpass,
/// notch) — a natural `split` source.
///
/// **Topology: Cytomic/Andrew Simper TPT** (topology-preserving transform)
/// SVF — the trapezoidal-integration, zero-delay-feedback form from Simper's
/// "Solving the continuous SVF equations using trapezoidal integration and
/// equivalent currents" (2013). Coefficients are recomputed *every sample*
/// from the input signals, and the two integrator states track the underlying
/// analog circuit, so the filter stays stable and click-free when cutoff and
/// Q sweep at audio rate. This is the modulation-friendly filter; prefer
/// [`BiquadNode`] when you want the cookbook EQ shapes and only block-rate
/// parameter changes.
///
/// The bandpass output is normalized to unity gain at the center frequency
/// (`k * v1` in Simper's notation), which makes the outputs exactly
/// complementary: `lowpass + bandpass + highpass` reconstructs the input
/// sample-for-sample, and `notch = lowpass + highpass = input - bandpass`.
///
/// Cutoff is clamped to \[1, 0.49 x sample rate\] Hz (non-finite values fall
/// back to the low bound), Q to \[0.05, 100\] (non-finite falls back to
/// 1/sqrt(2)).
#[derive(Debug, Clone)]
pub struct SvfNode {
    sample_rate_hz: f32,
    /// First integrator state (`ic1eq` in Simper's derivation).
    ic1: f32,
    /// Second integrator state (`ic2eq`).
    ic2: f32,
}

impl Node for SvfNode {
    fn inputs(&self) -> u32 {
        3
    }
    fn outputs(&self) -> u32 {
        4
    }
    fn process(&mut self, inputs: &[&[f32]], outputs: &mut [&mut [f32]], frames: usize) {
        let audio = inputs[0];
        let cutoff = inputs[1];
        let q = inputs[2];
        let [low_out, high_out, band_out, notch_out, ..] = outputs else {
            return;
        };
        let max_cutoff = self.sample_rate_hz * MAX_FREQUENCY_RATIO;
        for i in 0..frames {
            let fc = sanitize(cutoff[i], MIN_FREQUENCY_HZ, max_cutoff, MIN_FREQUENCY_HZ);
            let quality = sanitize(q[i], MIN_Q, MAX_Q, FRAC_1_SQRT_2);
            let g = (PI * fc / self.sample_rate_hz).tan();
            let k = 1.0 / quality;
            let a1 = 1.0 / g.mul_add(g + k, 1.0);
            let a2 = g * a1;
            let a3 = g * a2;

            let v0 = audio[i];
            let v3 = v0 - self.ic2;
            let v1 = a2.mul_add(v3, a1 * self.ic1);
            let v2 = a3.mul_add(v3, a2.mul_add(self.ic1, self.ic2));
            self.ic1 = v1.mul_add(2.0, -self.ic1);
            self.ic2 = v2.mul_add(2.0, -self.ic2);

            let low = v2;
            let band = k * v1;
            let high = k.mul_add(-v1, v0) - v2;
            low_out[i] = low;
            high_out[i] = high;
            band_out[i] = band;
            notch_out[i] = low + high;
        }
    }
    fn reset(&mut self) {
        self.ic1 = 0.0;
        self.ic2 = 0.0;
    }
}

/// Creates a TPT state-variable filter node.
/// 3 inputs (audio, cutoff\_hz, q), 4 outputs (lowpass, highpass, bandpass,
/// notch). Non-finite or non-positive sample rates fall back to 48 kHz.
#[must_use]
pub fn svf(sample_rate_hz: f32) -> SvfNode {
    SvfNode {
        sample_rate_hz: sanitize_sample_rate(sample_rate_hz),
        ic1: 0.0,
        ic2: 0.0,
    }
}

// ---------------------------------------------------------------------------
// BiquadNode
// ---------------------------------------------------------------------------

/// Response shapes for [`BiquadNode`], from the RBJ Audio EQ Cookbook.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BiquadMode {
    /// 12 dB/octave low-pass.
    Lowpass,
    /// 12 dB/octave high-pass.
    Highpass,
    /// Band-pass with constant 0 dB peak gain at the center frequency.
    Bandpass,
    /// Band-reject with a null exactly at the center frequency.
    Notch,
    /// Peaking (bell) EQ; takes a fourth input channel (gain\_db).
    Peaking,
}

/// An RBJ-cookbook second-order filter section.
/// Inputs (audio, freq\_hz, q) — plus gain\_db in [`BiquadMode::Peaking`] —
/// 1 output.
///
/// **Coefficients: RBJ Audio EQ Cookbook** (Robert Bristow-Johnson),
/// evaluated in transposed direct form II. Coefficients are recomputed *once
/// per block* from the block-start values of the parameter signals. That
/// makes updates cheap (a handful of transcendentals per block) at the cost
/// of parameter changes quantizing to block boundaries: sweeping the center
/// frequency at audio rate produces block-rate stepping ("zipper" artifacts)
/// instead of a smooth glide. For audio-rate filter modulation use
/// [`SvfNode`], whose TPT structure recomputes per sample.
///
/// Center frequency is clamped to \[1, 0.49 x sample rate\] Hz (non-finite
/// values fall back to the low bound), Q to \[0.05, 100\] (non-finite falls
/// back to 1/sqrt(2)), gain to \[-40, 40\] dB (non-finite falls back to
/// 0 dB). Every parameter combination inside those bounds keeps both poles
/// strictly inside the unit circle.
#[derive(Debug, Clone)]
pub struct BiquadNode {
    sample_rate_hz: f32,
    mode: BiquadMode,
    // Normalized coefficients (a0 divided out).
    b0: f32,
    b1: f32,
    b2: f32,
    a1: f32,
    a2: f32,
    // Transposed direct form II state.
    s1: f32,
    s2: f32,
}

impl BiquadNode {
    /// Recomputes the normalized cookbook coefficients for the block.
    fn update_coefficients(&mut self, freq_hz: f32, q: f32, gain_db: f32) {
        let max_freq = self.sample_rate_hz * MAX_FREQUENCY_RATIO;
        let freq = sanitize(freq_hz, MIN_FREQUENCY_HZ, max_freq, MIN_FREQUENCY_HZ);
        let quality = sanitize(q, MIN_Q, MAX_Q, FRAC_1_SQRT_2);
        let gain = sanitize(gain_db, -MAX_GAIN_DB, MAX_GAIN_DB, 0.0);

        let w0 = TAU * freq / self.sample_rate_hz;
        let (sin_w0, cos_w0) = w0.sin_cos();
        let alpha = sin_w0 / (2.0 * quality);

        let (b0, b1, b2, a0, a1, a2) = match self.mode {
            BiquadMode::Lowpass => {
                let b1 = 1.0 - cos_w0;
                (
                    b1 * 0.5,
                    b1,
                    b1 * 0.5,
                    1.0 + alpha,
                    -2.0 * cos_w0,
                    1.0 - alpha,
                )
            }
            BiquadMode::Highpass => {
                let peak = 1.0 + cos_w0;
                (
                    peak * 0.5,
                    -peak,
                    peak * 0.5,
                    1.0 + alpha,
                    -2.0 * cos_w0,
                    1.0 - alpha,
                )
            }
            BiquadMode::Bandpass => (alpha, 0.0, -alpha, 1.0 + alpha, -2.0 * cos_w0, 1.0 - alpha),
            BiquadMode::Notch => (
                1.0,
                -2.0 * cos_w0,
                1.0,
                1.0 + alpha,
                -2.0 * cos_w0,
                1.0 - alpha,
            ),
            BiquadMode::Peaking => {
                let amp = 10.0_f32.powf(gain / 40.0);
                (
                    alpha.mul_add(amp, 1.0),
                    -2.0 * cos_w0,
                    alpha.mul_add(-amp, 1.0),
                    (alpha / amp) + 1.0,
                    -2.0 * cos_w0,
                    1.0 - (alpha / amp),
                )
            }
        };

        let inv_a0 = 1.0 / a0;
        self.b0 = b0 * inv_a0;
        self.b1 = b1 * inv_a0;
        self.b2 = b2 * inv_a0;
        self.a1 = a1 * inv_a0;
        self.a2 = a2 * inv_a0;
    }
}

impl Node for BiquadNode {
    fn inputs(&self) -> u32 {
        match self.mode {
            BiquadMode::Peaking => 4,
            _ => 3,
        }
    }
    fn outputs(&self) -> u32 {
        1
    }
    fn process(&mut self, inputs: &[&[f32]], outputs: &mut [&mut [f32]], frames: usize) {
        if frames == 0 {
            return;
        }
        let audio = inputs[0];
        let gain_db = if self.mode == BiquadMode::Peaking {
            inputs[3][0]
        } else {
            0.0
        };
        self.update_coefficients(inputs[1][0], inputs[2][0], gain_db);

        let out = &mut outputs[0];
        for i in 0..frames {
            let x = audio[i];
            let y = self.b0.mul_add(x, self.s1);
            self.s1 = self.b1.mul_add(x, self.a1.mul_add(-y, self.s2));
            self.s2 = self.b2.mul_add(x, self.a2 * -y);
            out[i] = y;
        }
    }
    fn reset(&mut self) {
        self.s1 = 0.0;
        self.s2 = 0.0;
    }
}

/// Creates an RBJ-cookbook biquad filter node.
/// 3 inputs (audio, freq\_hz, q) — 4 with [`BiquadMode::Peaking`], which adds
/// gain\_db — 1 output. Non-finite or non-positive sample rates fall back to
/// 48 kHz.
#[must_use]
pub fn biquad(sample_rate_hz: f32, mode: BiquadMode) -> BiquadNode {
    BiquadNode {
        sample_rate_hz: sanitize_sample_rate(sample_rate_hz),
        mode,
        b0: 0.0,
        b1: 0.0,
        b2: 0.0,
        a1: 0.0,
        a2: 0.0,
        s1: 0.0,
        s2: 0.0,
    }
}
