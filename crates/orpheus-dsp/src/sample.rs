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
    let mut reader = hound::WavReader::open(path).map_err(|source| SampleError::Io {
        path: display_path.clone().into_boxed_str(),
        source,
    })?;
    let spec = reader.spec();
    if !matches!(spec.channels, 1 | 2) {
        return Err(SampleError::UnsupportedChannelCount(display_path));
    }

    let frames = match spec.sample_format {
        hound::SampleFormat::Float if spec.bits_per_sample == 32 => reader
            .samples::<f32>()
            .collect::<Result<Vec<_>, _>>()
            .map_err(|source| SampleError::Io {
                path: display_path.clone().into_boxed_str(),
                source,
            })?,
        hound::SampleFormat::Float => {
            return Err(SampleError::UnsupportedFloatEncoding(display_path));
        }
        hound::SampleFormat::Int if (1..=32).contains(&spec.bits_per_sample) => {
            let scale = integer_scale(spec.bits_per_sample)
                .ok_or_else(|| SampleError::UnsupportedIntEncoding(display_path.clone()))?;
            reader
                .samples::<i32>()
                .map(|sample| {
                    sample
                        .map(|sample| normalize_int_sample(sample, scale))
                        .map_err(|source| SampleError::Io {
                            path: display_path.clone().into_boxed_str(),
                            source,
                        })
                })
                .collect::<Result<Vec<_>, _>>()?
        }
        hound::SampleFormat::Int => {
            return Err(SampleError::UnsupportedIntEncoding(display_path));
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
