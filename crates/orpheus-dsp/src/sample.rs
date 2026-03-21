//! The `sample` module provides audio sample decoding and buffering.
//!
//! This module is responsible for loading audio files from disk (e.g., `.wav`), decoding
//! them into raw floating-point channels, and storing them in memory as `DecodedSample`s
//! so they can be instantly read by the real-time audio thread without allocation.

use std::fs::File;
use std::io::{Cursor, Read, Seek};
use std::path::Path;

use thiserror::Error;

/// Decoded PCM sample data normalized to interleaved `f32` frames.
#[derive(Clone, Debug, PartialEq)]
pub struct DecodedSample {
    pub channels: u16,
    pub sample_rate_hz: u32,
    pub frames: Vec<f32>,
}

/// Errors raised while decoding WAV sample assets.
#[derive(Debug, Error)]
pub enum SampleError {
    #[error("failed to read wav file `{path}`: {source}")]
    Io {
        path: Box<str>,
        #[source]
        source: hound::Error,
    },
    #[error("wav file `{0}` must contain either mono or stereo audio")]
    UnsupportedChannelCount(String),
    #[error("wav file `{0}` uses an unsupported float encoding")]
    UnsupportedFloatEncoding(String),
    #[error("wav file `{0}` uses an unsupported integer bit depth")]
    UnsupportedIntEncoding(String),
    #[error("unknown built-in sample `{0}`")]
    UnknownBuiltinSample(String),
}

/// Loads a WAV fixture for deterministic tests.
///
/// # Errors
///
/// Returns [`SampleError`] when the file cannot be read or uses an unsupported
/// encoding.
pub fn load_wav_for_test(path: impl AsRef<Path>) -> Result<DecodedSample, SampleError> {
    let path = path.as_ref();
    let display_path = path.display().to_string();
    let file = File::open(path).map_err(|source| SampleError::Io {
        path: display_path.clone().into_boxed_str(),
        source: source.into(),
    })?;
    decode_wav_reader(file, &display_path)
}

pub fn load_wav_bytes(
    bytes: &'static [u8],
    display_path: &'static str,
) -> Result<DecodedSample, SampleError> {
    decode_wav_reader(Cursor::new(bytes), display_path)
}

fn decode_wav_reader<R>(reader: R, display_path: &str) -> Result<DecodedSample, SampleError>
where
    R: Read + Seek,
{
    let mut reader = hound::WavReader::new(reader).map_err(|source| SampleError::Io {
        path: display_path.to_owned().into_boxed_str(),
        source,
    })?;
    let spec = reader.spec();
    if !matches!(spec.channels, 1 | 2) {
        return Err(SampleError::UnsupportedChannelCount(
            display_path.to_owned(),
        ));
    }

    let frames = match spec.sample_format {
        hound::SampleFormat::Float if spec.bits_per_sample == 32 => reader
            .samples::<f32>()
            .collect::<Result<Vec<_>, _>>()
            .map_err(|source| SampleError::Io {
                path: display_path.to_owned().into_boxed_str(),
                source,
            })?,
        hound::SampleFormat::Float => {
            return Err(SampleError::UnsupportedFloatEncoding(
                display_path.to_owned(),
            ));
        }
        hound::SampleFormat::Int if (1..=32).contains(&spec.bits_per_sample) => {
            let scale = integer_scale(spec.bits_per_sample)
                .ok_or_else(|| SampleError::UnsupportedIntEncoding(display_path.to_owned()))?;
            reader
                .samples::<i32>()
                .map(|sample| {
                    sample
                        .map(|sample| normalize_int_sample(sample, scale))
                        .map_err(|source| SampleError::Io {
                            path: display_path.to_owned().into_boxed_str(),
                            source,
                        })
                })
                .collect::<Result<Vec<_>, _>>()?
        }
        hound::SampleFormat::Int => {
            return Err(SampleError::UnsupportedIntEncoding(display_path.to_owned()));
        }
    };

    Ok(DecodedSample {
        channels: spec.channels,
        sample_rate_hz: spec.sample_rate,
        frames,
    })
}

fn integer_scale(bits_per_sample: u16) -> Option<f64> {
    let exponent = i32::from(bits_per_sample.checked_sub(1)?);
    Some(2_f64.powi(exponent))
}

#[allow(clippy::cast_possible_truncation, clippy::cast_precision_loss)]
fn normalize_int_sample(sample: i32, scale: f64) -> f32 {
    (f64::from(sample) / scale).clamp(-1.0, 1.0) as f32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sample_error_formats_correctly() {
        assert_eq!(
            SampleError::UnsupportedChannelCount("test.wav".into()).to_string(),
            "wav file `test.wav` must contain either mono or stereo audio"
        );
        assert_eq!(
            SampleError::UnsupportedFloatEncoding("test.wav".into()).to_string(),
            "wav file `test.wav` uses an unsupported float encoding"
        );
        assert_eq!(
            SampleError::UnsupportedIntEncoding("test.wav".into()).to_string(),
            "wav file `test.wav` uses an unsupported integer bit depth"
        );
        assert_eq!(
            SampleError::UnknownBuiltinSample("test.wav".into()).to_string(),
            "unknown built-in sample `test.wav`"
        );
    }

    #[test]
    fn integer_scale_returns_none_for_zero_bits() {
        assert_eq!(integer_scale(0), None);
    }

    #[test]
    fn integer_scale_returns_power_of_two_scale() {
        assert_eq!(integer_scale(8), Some(128.0));
        assert_eq!(integer_scale(16), Some(32768.0));
        assert_eq!(integer_scale(24), Some(8_388_608.0));
    }

    #[test]
    fn normalize_int_sample_clamps_to_unit_range() {
        // Normal ranges
        assert!((normalize_int_sample(16384, 32768.0) - 0.5).abs() < f32::EPSILON);
        assert!((normalize_int_sample(-16384, 32768.0) - (-0.5)).abs() < f32::EPSILON);

        // Out of bounds, should be clamped
        assert!((normalize_int_sample(40000, 32768.0) - 1.0).abs() < f32::EPSILON);
        assert!((normalize_int_sample(-40000, 32768.0) - (-1.0)).abs() < f32::EPSILON);
    }
}
