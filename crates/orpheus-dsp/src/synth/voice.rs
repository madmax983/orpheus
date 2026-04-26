//! Thin monophonic voice composition over the analog DSP primitive kernel.

use super::{Gain, LadderFilter, Noise, PulseOsc, SawOsc, SoftSat, TriOsc};

const DEFAULT_NOISE_SEED: u32 = 0xA10A_600D;

/// Selects which source primitive drives [`AnalogVoice`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OscShape {
    /// A band-limited saw source.
    Saw,
    /// A band-limited pulse source.
    Pulse,
    /// A triangle source derived from a band-limited square/integrator path.
    Tri,
    /// A deterministic white-noise source.
    Noise,
}

/// Scalar control snapshot for one monophonic analog voice sample.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AnalogVoiceParams {
    /// Which source primitive to use for this sample.
    pub osc_shape: OscShape,
    /// Oscillator frequency in Hz for pitched sources.
    pub freq_hz: f32,
    /// Pulse width for the pulse oscillator.
    pub pulse_width: f32,
    /// Ladder low-pass cutoff in Hz.
    pub cutoff_hz: f32,
    /// Ladder resonance amount in `[0, 1]`.
    pub resonance: f32,
    /// Soft saturation drive amount.
    pub drive: f32,
    /// Final linear output gain.
    pub gain: f32,
}

/// Thin monophonic subtractive voice wrapper over the primitive DSP algebra.
#[derive(Debug, Clone, PartialEq)]
pub struct AnalogVoice {
    saw: SawOsc,
    pulse: PulseOsc,
    tri: TriOsc,
    noise: Noise,
    filter: LadderFilter,
    sat: SoftSat,
    gain: Gain,
}

impl AnalogVoice {
    /// Creates a new analog voice wrapper for `sample_rate_hz`.
    ///
    /// ## Examples
    ///
    /// ```
    /// use orpheus_dsp::{AnalogVoice, AnalogVoiceParams, OscShape};
    ///
    /// let mut voice = AnalogVoice::new(48000.0);
    /// let params = AnalogVoiceParams {
    ///     osc_shape: OscShape::Saw,
    ///     freq_hz: 440.0,
    ///     pulse_width: 0.5,
    ///     cutoff_hz: 2000.0,
    ///     resonance: 0.0,
    ///     drive: 0.0,
    ///     gain: 1.0,
    /// };
    /// let sample = voice.next_sample(&params);
    /// ```
    #[must_use]
    pub fn new(sample_rate_hz: f32) -> Self {
        Self {
            saw: SawOsc::new(sample_rate_hz),
            pulse: PulseOsc::new(sample_rate_hz),
            tri: TriOsc::new(sample_rate_hz),
            noise: Noise::new(DEFAULT_NOISE_SEED),
            filter: LadderFilter::new(sample_rate_hz),
            sat: SoftSat::new(),
            gain: Gain::new(),
        }
    }

    /// Resets all owned primitive state to a deterministic starting point.
    pub const fn reset(&mut self) {
        self.saw.reset();
        self.pulse.reset();
        self.tri.reset();
        self.noise.reset(DEFAULT_NOISE_SEED);
        self.filter.reset();
        self.sat.reset();
        self.gain.reset();
    }

    /// Renders one monophonic sample through source, filter, saturation, and gain.
    ///
    /// ## Examples
    ///
    /// ```
    /// use orpheus_dsp::{AnalogVoice, AnalogVoiceParams, OscShape};
    ///
    /// let mut voice = AnalogVoice::new(48000.0);
    /// let params = AnalogVoiceParams {
    ///     osc_shape: OscShape::Saw,
    ///     freq_hz: 440.0,
    ///     pulse_width: 0.5,
    ///     cutoff_hz: 2000.0,
    ///     resonance: 0.0,
    ///     drive: 0.0,
    ///     gain: 1.0,
    /// };
    /// let sample = voice.next_sample(&params);
    /// ```
    #[must_use]
    pub fn next_sample(&mut self, params: &AnalogVoiceParams) -> f32 {
        let source = match params.osc_shape {
            OscShape::Saw => self.saw.next_sample(params.freq_hz),
            OscShape::Pulse => self.pulse.next_sample(params.freq_hz, params.pulse_width),
            OscShape::Tri => self.tri.next_sample(params.freq_hz),
            OscShape::Noise => self.noise.next_sample(),
        };

        let filtered = self
            .filter
            .process(source, params.cutoff_hz, params.resonance);
        let saturated = self.sat.process(filtered, params.drive);
        self.gain.process(saturated, params.gain)
    }
}
