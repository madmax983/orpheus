use std::collections::BTreeMap;
use std::fs;
use std::path::Path;
use std::sync::Arc;

use thiserror::Error;

use crate::sample::{DecodedSample, SampleError, load_wav_bytes, load_wav_for_test};
use crate::voice::VoiceKind;

const KICK_WAV: &[u8] = include_bytes!("../assets/kick.wav");
const SNARE_WAV: &[u8] = include_bytes!("../assets/snare.wav");
const CLAP_WAV: &[u8] = include_bytes!("../assets/clap.wav");
const HIHAT_WAV: &[u8] = include_bytes!("../assets/hihat.wav");

#[derive(Clone, Debug, PartialEq)]
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

#[derive(Clone, Debug, Default, PartialEq)]
pub struct SampleBank {
    samples: BTreeMap<Box<str>, PlaybackSample>,
}

impl SampleBank {
    #[must_use]
    pub fn load_builtin() -> Self {
        let mut bank = Self::default();
        for token in ["bd", "sn", "cp", "hh"] {
            if let Ok(sample) = load_builtin_sample_for_test(token) {
                bank.samples.insert(token.into(), sample.into());
            }
        }
        bank
    }

    #[must_use]
    pub fn get(&self, voice: VoiceKind) -> Option<&PlaybackSample> {
        self.get_by_token(voice.token())
    }

    #[must_use]
    pub fn get_by_token(&self, token: &str) -> Option<&PlaybackSample> {
        self.samples.get(token)
    }

    #[must_use]
    pub fn available_tokens(&self) -> Vec<String> {
        self.samples.keys().map(ToString::to_string).collect()
    }

    fn insert_token(&mut self, token: &str, sample: PlaybackSample) {
        self.samples.insert(token.into(), sample);
    }
}

/// Errors raised while scanning a sample directory for override assets.
#[derive(Debug, Error)]
pub enum SampleBankError {
    #[error("failed to read sample directory `{path}`: {source}")]
    DirectoryIo {
        path: Box<str>,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to decode sample override `{path}`: {source}")]
    Decode {
        path: Box<str>,
        #[source]
        source: SampleError,
    },
}

/// Loads a sample bank from built-ins plus any supported WAV overrides found in
/// `directory`.
///
/// Supported filenames map onto the current built-in drum tokens:
/// `bd`/`kick`, `sn`/`snare`, `cp`/`clap`, and `hh`/`hat`/`hihat`.
///
/// # Errors
///
/// Returns [`SampleBankError`] if the directory cannot be read or if one of the
/// supported override files fails to decode.
pub fn load_sample_bank_from_directory(
    directory: impl AsRef<Path>,
) -> Result<SampleBank, SampleBankError> {
    let directory = directory.as_ref();
    let display_path = directory.display().to_string();
    let mut entries = fs::read_dir(directory)
        .map_err(|source| SampleBankError::DirectoryIo {
            path: display_path.clone().into_boxed_str(),
            source,
        })?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|source| SampleBankError::DirectoryIo {
            path: display_path.clone().into_boxed_str(),
            source,
        })?;
    entries.sort_by_key(std::fs::DirEntry::file_name);

    let mut bank = SampleBank::load_builtin();
    let mut candidates: BTreeMap<&'static str, (u8, PlaybackSample)> = BTreeMap::new();

    for entry in entries {
        let path = entry.path();
        if !path.is_file() || !is_supported_wav_path(&path) {
            continue;
        }
        let Some(stem) = path.file_stem().and_then(|stem| stem.to_str()) else {
            continue;
        };
        let Some((token, priority)) = token_from_stem(stem) else {
            continue;
        };

        let sample = load_wav_for_test(&path).map_err(|source| SampleBankError::Decode {
            path: path.display().to_string().into_boxed_str(),
            source,
        })?;
        match candidates.get(token) {
            Some((existing_priority, _)) if *existing_priority <= priority => {}
            _ => {
                candidates.insert(token, (priority, sample.into()));
            }
        }
    }

    for (token, (_priority, sample)) in candidates {
        bank.insert_token(token, sample);
    }

    Ok(bank)
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

fn is_supported_wav_path(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| matches!(extension.to_ascii_lowercase().as_str(), "wav" | "wave"))
}

fn token_from_stem(stem: &str) -> Option<(&'static str, u8)> {
    match stem.to_ascii_lowercase().as_str() {
        "bd" => Some(("bd", 0)),
        "kick" => Some(("bd", 1)),
        "sn" => Some(("sn", 0)),
        "snare" => Some(("sn", 1)),
        "cp" => Some(("cp", 0)),
        "clap" => Some(("cp", 1)),
        "hh" => Some(("hh", 0)),
        "hat" | "hihat" => Some(("hh", 1)),
        _ => None,
    }
}
