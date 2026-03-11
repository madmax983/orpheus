use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use orpheus_dsp::{
    SampleTrigger, load_sample_bank_from_directory, render_events_to_file_with_bank,
};
use orpheus_pattern::{Event, Rational, TimeSpan};

#[test]
fn offline_render_applies_sample_gain_rate_and_slice() {
    let directory = temp_directory("sample-params-offline");
    fs::write(
        directory.join("samples.ron"),
        "(\n  tokens: {\n    \"vox_ah\": \"vox.wav\",\n  },\n)\n",
    )
    .unwrap();
    write_wav(directory.join("vox.wav"), &[0.2, 0.4, 0.6, 0.8]);
    let bank = load_sample_bank_from_directory(&directory).unwrap();
    let path = temp_wav_path();

    let events = vec![Event {
        whole: None,
        part: TimeSpan::new(Rational::zero(), Rational::new(1, 4).unwrap()).unwrap(),
        value: SampleTrigger::named("vox_ah")
            .with_gain(0.5)
            .with_rate(2.0)
            .with_slice(0.25, 1.0),
    }];

    render_events_to_file_with_bank(&path, &events, 1, &bank).unwrap();

    let mut reader = hound::WavReader::open(&path).unwrap();
    let samples = reader
        .samples::<i16>()
        .take(4)
        .collect::<Result<Vec<_>, _>>()
        .unwrap();

    assert_eq!(samples.len(), 4);
    assert!((i32::from(samples[0]) - 6_554).abs() <= 1);
    assert!((i32::from(samples[1]) - 6_554).abs() <= 1);
    assert!((i32::from(samples[2]) - 13_107).abs() <= 1);
    assert!((i32::from(samples[3]) - 13_107).abs() <= 1);

    let _ = fs::remove_file(path);
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn offline_render_uses_manifest_region_defaults_and_composes_explicit_slice() {
    let directory = temp_directory("sample-region-offline");
    fs::write(
        directory.join("samples.ron"),
        concat!(
            "(\n",
            "  tokens: {\n",
            "    \"amen\": \"amen.wav\",\n",
            "  },\n",
            "  regions: {\n",
            "    \"amen_tail\": (\n",
            "      token: \"amen\",\n",
            "      start: 0.25,\n",
            "      end: 0.75,\n",
            "    ),\n",
            "  },\n",
            ")\n"
        ),
    )
    .unwrap();
    write_wav(
        directory.join("amen.wav"),
        &[0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8],
    );
    let bank = load_sample_bank_from_directory(&directory).unwrap();
    let path = temp_wav_path();

    let events = vec![Event {
        whole: None,
        part: TimeSpan::new(Rational::zero(), Rational::new(1, 4).unwrap()).unwrap(),
        value: SampleTrigger::named("amen_tail").with_slice(0.5, 1.0),
    }];

    render_events_to_file_with_bank(&path, &events, 1, &bank).unwrap();

    let mut reader = hound::WavReader::open(&path).unwrap();
    let samples = reader
        .samples::<i16>()
        .take(4)
        .collect::<Result<Vec<_>, _>>()
        .unwrap();

    assert_eq!(samples.len(), 4);
    assert!((i32::from(samples[0]) - 16_384).abs() <= 1);
    assert!((i32::from(samples[1]) - 16_384).abs() <= 1);
    assert!((i32::from(samples[2]) - 19_660).abs() <= 1);
    assert!((i32::from(samples[3]) - 19_660).abs() <= 1);

    let _ = fs::remove_file(path);
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn offline_render_applies_sample_pan_balance() {
    let directory = temp_directory("sample-pan-offline");
    fs::write(
        directory.join("samples.ron"),
        "(\n  tokens: {\n    \"vox_ah\": \"vox.wav\",\n  },\n)\n",
    )
    .unwrap();
    write_wav(directory.join("vox.wav"), &[0.5, 0.0, 0.0, 0.0]);
    let bank = load_sample_bank_from_directory(&directory).unwrap();
    let path = temp_wav_path();

    let events = vec![Event {
        whole: None,
        part: TimeSpan::new(Rational::zero(), Rational::new(1, 4).unwrap()).unwrap(),
        value: SampleTrigger::named("vox_ah").with_pan(-1.0),
    }];

    render_events_to_file_with_bank(&path, &events, 1, &bank).unwrap();

    let mut reader = hound::WavReader::open(&path).unwrap();
    let samples = reader
        .samples::<i16>()
        .take(4)
        .collect::<Result<Vec<_>, _>>()
        .unwrap();

    assert_eq!(samples, vec![16_384, 0, 0, 0]);

    let _ = fs::remove_file(path);
    fs::remove_dir_all(directory).unwrap();
}

fn temp_directory(name: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let directory = std::env::temp_dir().join(format!("orpheus-dsp-{unique}-{name}"));
    fs::create_dir_all(&directory).unwrap();
    directory
}

fn temp_wav_path() -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!("orpheus-dsp-render-{unique}.wav"))
}

fn write_wav(path: impl AsRef<Path>, frames: &[f32]) {
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: 48_000,
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Float,
    };
    let mut writer = hound::WavWriter::create(path, spec).unwrap();
    for sample in frames {
        writer.write_sample(*sample).unwrap();
    }
    writer.finalize().unwrap();
}
