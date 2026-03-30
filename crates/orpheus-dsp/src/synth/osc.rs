//! Source primitives for the analog DSP kernel.

use super::PhaseAccumulator;

const MAX_NORMALIZED_STEP: f32 = 0.49;
const MIN_PULSE_WIDTH: f32 = 0.01;
const MAX_PULSE_WIDTH: f32 = 0.99;

fn sanitize_sample_rate(sample_rate_hz: f32) -> f32 {
    if sample_rate_hz.is_finite() && sample_rate_hz > 0.0 {
        sample_rate_hz
    } else {
        48_000.0
    }
}

fn normalized_step(freq_hz: f32, sample_rate_hz: f32) -> f32 {
    if !freq_hz.is_finite() || freq_hz <= 0.0 {
        0.0
    } else {
        (freq_hz / sample_rate_hz).clamp(0.0, MAX_NORMALIZED_STEP)
    }
}

fn poly_blep(t: f32, dt: f32) -> f32 {
    if dt <= 0.0 {
        0.0
    } else if t < dt {
        let x = t / dt;
        (-x).mul_add(x, (x + x) - 1.0)
    } else if t > 1.0 - dt {
        let x = (t - 1.0) / dt;
        x.mul_add(x + 2.0, 1.0)
    } else {
        0.0
    }
}

fn wrap_phase_offset(phase: f32, offset: f32) -> f32 {
    (phase + offset).rem_euclid(1.0)
}

/// A `PolyBLEP` band-limited saw oscillator.
#[derive(Debug, Clone, PartialEq)]
pub struct SawOsc {
    sample_rate_hz: f32,
    phase: PhaseAccumulator,
}

impl SawOsc {
    /// Creates a new saw oscillator.
    #[must_use]
    pub fn new(sample_rate_hz: f32) -> Self {
        Self {
            sample_rate_hz: sanitize_sample_rate(sample_rate_hz),
            phase: PhaseAccumulator::new(),
        }
    }

    /// Resets the oscillator phase to the start of the cycle.
    pub const fn reset(&mut self) {
        self.phase.reset();
    }

    /// Produces the next sample at `freq_hz`.
    #[must_use]
    pub fn next_sample(&mut self, freq_hz: f32) -> f32 {
        let step = normalized_step(freq_hz, self.sample_rate_hz);
        let phase = self.phase.phase();
        let sample = 2.0_f32.mul_add(phase, -1.0) - poly_blep(phase, step);
        self.phase.advance(step);
        sample
    }
}

/// A `PolyBLEP` band-limited pulse oscillator.
#[derive(Debug, Clone, PartialEq)]
pub struct PulseOsc {
    sample_rate_hz: f32,
    phase: PhaseAccumulator,
}

impl PulseOsc {
    /// Creates a new pulse oscillator.
    #[must_use]
    pub fn new(sample_rate_hz: f32) -> Self {
        Self {
            sample_rate_hz: sanitize_sample_rate(sample_rate_hz),
            phase: PhaseAccumulator::new(),
        }
    }

    /// Resets the oscillator phase to the start of the cycle.
    pub const fn reset(&mut self) {
        self.phase.reset();
    }

    /// Produces the next pulse sample at `freq_hz` and `pulse_width`.
    #[must_use]
    pub fn next_sample(&mut self, freq_hz: f32, pulse_width: f32) -> f32 {
        let step = normalized_step(freq_hz, self.sample_rate_hz);
        let phase = self.phase.phase();
        let width = pulse_width.clamp(MIN_PULSE_WIDTH, MAX_PULSE_WIDTH);
        let shifted_phase = wrap_phase_offset(phase, 1.0 - width);

        let mut sample = if phase < width { 1.0 } else { -1.0 };
        sample += poly_blep(phase, step);
        sample -= poly_blep(shifted_phase, step);

        self.phase.advance(step);
        sample
    }
}

/// A triangle oscillator derived from a `PolyBLEP` square/integrator path.
#[derive(Debug, Clone, PartialEq)]
pub struct TriOsc {
    sample_rate_hz: f32,
    phase: PhaseAccumulator,
    integrator: f32,
}

impl TriOsc {
    /// Creates a new triangle oscillator.
    #[must_use]
    pub fn new(sample_rate_hz: f32) -> Self {
        Self {
            sample_rate_hz: sanitize_sample_rate(sample_rate_hz),
            phase: PhaseAccumulator::new(),
            integrator: -1.0,
        }
    }

    /// Resets the oscillator phase and integrator state.
    pub const fn reset(&mut self) {
        self.phase.reset();
        self.integrator = -1.0;
    }

    /// Produces the next triangle sample at `freq_hz`.
    #[must_use]
    pub fn next_sample(&mut self, freq_hz: f32) -> f32 {
        let step = normalized_step(freq_hz, self.sample_rate_hz);
        let phase = self.phase.phase();
        let half_cycle = wrap_phase_offset(phase, 0.5);

        let mut square = if phase < 0.5 { 1.0 } else { -1.0 };
        square += poly_blep(phase, step);
        square -= poly_blep(half_cycle, step);

        let leak = (1.0 - step).clamp(0.0, 1.0);
        self.integrator = self.integrator.mul_add(leak, square * step * 4.0);
        self.integrator = self.integrator.clamp(-1.0, 1.0);

        self.phase.advance(step);
        self.integrator
    }
}

/// A deterministic white-noise source.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Noise {
    state: u32,
}

impl Noise {
    /// Creates a new deterministic noise generator from `seed`.
    #[must_use]
    pub const fn new(seed: u32) -> Self {
        Self {
            state: if seed == 0 { 0xA341_316C } else { seed },
        }
    }

    /// Resets the noise generator to `seed`.
    pub const fn reset(&mut self, seed: u32) {
        self.state = if seed == 0 { 0xA341_316C } else { seed };
    }

    /// Produces the next deterministic white-noise sample in `[-1.0, 1.0]`.
    #[must_use]
    pub fn next_sample(&mut self) -> f32 {
        self.state ^= self.state << 13;
        self.state ^= self.state >> 17;
        self.state ^= self.state << 5;

        let normalized = f32::from_bits(0x3F80_0000_u32 | (self.state >> 9)) - 1.0;
        normalized.mul_add(2.0, -1.0)
    }
}
