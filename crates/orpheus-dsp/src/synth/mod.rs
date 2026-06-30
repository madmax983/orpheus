//! Reusable scalar DSP primitives for analog-style voice construction.
//!
//! This module is the small verified-ish kernel for later voice and graph work.
//! The first slice intentionally stays narrow: phase helpers, gain, mixing, and
//! bounded nonlinearity.

mod filter;
mod gain;
mod math;
mod mix;
mod nonlinear;
mod osc;
mod voice;

pub use filter::LadderFilter;
pub use gain::Gain;
pub use math::PhaseAccumulator;
pub use mix::Mix;
pub use nonlinear::SoftSat;
pub use osc::{Noise, PulseOsc, SawOsc, TriOsc};
pub use voice::{AnalogVoice, AnalogVoiceParams, OscShape};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VoiceKind {
    /// Synthesized bass drum placeholder for `bd`.
    KickLike,
    /// Synthesized snare placeholder for `sn`.
    SnareLike,
    /// Synthesized clap placeholder for `cp`.
    ClapLike,
    /// Synthesized hi-hat placeholder for `hh`.
    HiHatLike,
    /// Analog saw voice placeholder for `saw`.
    AnalogSaw,
    /// Analog pulse voice placeholder for `pulse`.
    AnalogPulse,
    /// Analog triangle voice placeholder for `tri`.
    AnalogTri,
    /// Analog noise voice placeholder for `noise`.
    AnalogNoise,
}

impl VoiceKind {
    /// The canonical string identifier bridging the language REPL to the DSP engine.
    ///
    /// When users type `"bd"` or `"sn"` in the Orpheus language environment, the
    /// evaluator embeds these strings into the resulting playback sequence. The `SampleBank`
    /// maps these specific string tokens to this enum, enabling the audio engine to dispatch
    /// rendering to the correct fast-path synthesizer voice without executing expensive string
    /// comparisons on the audio thread.
    ///
    /// ## Examples
    ///
    /// ```
    /// use orpheus_dsp::VoiceKind;
    ///
    /// assert_eq!(VoiceKind::KickLike.token(), "bd");
    /// assert_eq!(VoiceKind::SnareLike.token(), "sn");
    /// ```
    #[must_use]
    pub const fn token(self) -> &'static str {
        match self {
            Self::KickLike => "bd",
            Self::SnareLike => "sn",
            Self::ClapLike => "cp",
            Self::HiHatLike => "hh",
            Self::AnalogSaw => "saw",
            Self::AnalogPulse => "pulse",
            Self::AnalogTri => "tri",
            Self::AnalogNoise => "noise",
        }
    }

    /// Resolves a phase-one drum token to the current synthesized fallback.
    #[must_use]
    pub fn from_token(token: &str) -> Option<Self> {
        match token {
            "bd" => Some(Self::KickLike),
            "sn" => Some(Self::SnareLike),
            "cp" => Some(Self::ClapLike),
            "hh" => Some(Self::HiHatLike),
            "saw" => Some(Self::AnalogSaw),
            "pulse" => Some(Self::AnalogPulse),
            "tri" => Some(Self::AnalogTri),
            "noise" => Some(Self::AnalogNoise),
            _ => None,
        }
    }

    pub(crate) const fn is_drum_voice(self) -> bool {
        matches!(
            self,
            Self::KickLike | Self::SnareLike | Self::ClapLike | Self::HiHatLike
        )
    }
}
