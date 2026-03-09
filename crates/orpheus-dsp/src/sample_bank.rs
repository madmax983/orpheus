use std::sync::Arc;

use crate::sample::{DecodedSample, SampleError, load_wav_bytes};
use crate::voice::VoiceKind;

const KICK_WAV: &[u8] = include_bytes!("../assets/kick.wav");
const SNARE_WAV: &[u8] = include_bytes!("../assets/snare.wav");
const CLAP_WAV: &[u8] = include_bytes!("../assets/clap.wav");
const HIHAT_WAV: &[u8] = include_bytes!("../assets/hihat.wav");

#[derive(Clone, Debug)]
pub struct PlaybackSample {
    frames: Arc<[f32]>,
    sample_rate_hz: u32,
}

impl PlaybackSample {
    #[must_use]
    pub const fn frames(&self) -> &Arc<[f32]> {
        &self.frames
    }

    #[must_use]
    pub const fn sample_rate_hz(&self) -> u32 {
        self.sample_rate_hz
    }
}

impl From<DecodedSample> for PlaybackSample {
    fn from(sample: DecodedSample) -> Self {
        let frames = match sample.channels {
            1 => sample.frames,
            2 => sample
                .frames
                .chunks_exact(2)
                .map(|channel_pair| (channel_pair[0] + channel_pair[1]) * 0.5)
                .collect(),
            _ => unreachable!("decoded samples are constrained to mono or stereo"),
        };

        Self {
            frames: Arc::<[f32]>::from(frames),
            sample_rate_hz: sample.sample_rate_hz,
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct SampleBank {
    kick: Option<PlaybackSample>,
    snare: Option<PlaybackSample>,
    clap: Option<PlaybackSample>,
    hihat: Option<PlaybackSample>,
}

impl SampleBank {
    #[must_use]
    pub fn load_builtin() -> Self {
        Self {
            kick: load_builtin_sample_for_test("bd").ok().map(Into::into),
            snare: load_builtin_sample_for_test("sn").ok().map(Into::into),
            clap: load_builtin_sample_for_test("cp").ok().map(Into::into),
            hihat: load_builtin_sample_for_test("hh").ok().map(Into::into),
        }
    }

    #[must_use]
    pub const fn get(&self, voice: VoiceKind) -> Option<&PlaybackSample> {
        match voice {
            VoiceKind::KickLike => self.kick.as_ref(),
            VoiceKind::SnareLike => self.snare.as_ref(),
            VoiceKind::ClapLike => self.clap.as_ref(),
            VoiceKind::HiHatLike => self.hihat.as_ref(),
        }
    }
}

/// Loads one embedded built-in sample for deterministic tests.
///
/// # Errors
///
/// Returns [`SampleError`] when the built-in name is unknown or the embedded WAV
/// bytes fail to decode.
pub fn load_builtin_sample_for_test(name: &str) -> Result<DecodedSample, SampleError> {
    let (bytes, display_path) = builtin_sample_bytes(name)?;
    load_wav_bytes(bytes, display_path)
}

fn builtin_sample_bytes(name: &str) -> Result<(&'static [u8], &'static str), SampleError> {
    match name {
        "bd" => Ok((KICK_WAV, "builtin://kick.wav")),
        "sn" => Ok((SNARE_WAV, "builtin://snare.wav")),
        "cp" => Ok((CLAP_WAV, "builtin://clap.wav")),
        "hh" => Ok((HIHAT_WAV, "builtin://hihat.wav")),
        _ => Err(SampleError::UnknownBuiltinSample(name.to_owned())),
    }
}
