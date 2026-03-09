use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use orpheus_dsp::{SampleError, load_builtin_sample_for_test, load_wav_for_test};

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("tests")
        .join("fixtures")
        .join(name)
}

#[test]
fn wav_loader_decodes_mono_f32_samples() {
    let sample = load_wav_for_test(fixture("kick.wav")).unwrap();

    assert!(!sample.frames.is_empty());
}

#[test]
fn wav_loader_rejects_non_mono_or_stereo_sources() {
    let path = temp_fixture("tri_channel.wav");
    let spec = hound::WavSpec {
        channels: 3,
        sample_rate: 48_000,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut writer = hound::WavWriter::create(&path, spec).unwrap();
    for sample in [0_i16, 0, 0] {
        writer.write_sample(sample).unwrap();
    }
    writer.finalize().unwrap();

    let error = load_wav_for_test(&path).unwrap_err();
    assert!(matches!(error, SampleError::UnsupportedChannelCount(_)));

    fs::remove_file(path).unwrap();
}

#[test]
fn built_in_drum_assets_decode_for_test_use() {
    for token in ["bd", "sn", "cp", "hh"] {
        let sample = load_builtin_sample_for_test(token).unwrap();
        assert!(
            !sample.frames.is_empty(),
            "builtin sample {token} was empty"
        );
    }
}

fn temp_fixture(name: &str) -> PathBuf {
    let directory = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("target")
        .join("test-artifacts");
    fs::create_dir_all(&directory).unwrap();

    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    directory.join(format!("{unique}-{name}"))
}
