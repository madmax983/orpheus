use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use orpheus_dsp::{
    SampleTrigger, load_sample_bank_from_directory, render_events_to_file_with_bank,
};
use orpheus_pattern::{Event, Rational, TimeSpan};

static UNIQUE_TEMP_ID: AtomicU64 = AtomicU64::new(0);

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
    let expected = vec![
        pcm16(0.2 * edge_envelope(0, 2)),
        pcm16(0.2 * edge_envelope(0, 2)),
        pcm16(0.4 * edge_envelope(1, 2)),
        pcm16(0.4 * edge_envelope(1, 2)),
    ];

    assert_eq!(samples.len(), 4);
    assert_eq!(samples, expected);

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
    let expected = vec![
        pcm16(0.5 * edge_envelope(0, 2)),
        pcm16(0.5 * edge_envelope(0, 2)),
        pcm16(0.6 * edge_envelope(1, 2)),
        pcm16(0.6 * edge_envelope(1, 2)),
    ];

    assert_eq!(samples.len(), 4);
    assert_eq!(samples, expected);

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

    assert_eq!(samples, vec![pcm16(0.5 * edge_envelope(0, 4)), 0, 0, 0]);

    let _ = fs::remove_file(path);
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn offline_render_applies_sample_low_pass_filter() {
    let directory = temp_directory("sample-lpf-offline");
    fs::write(
        directory.join("samples.ron"),
        "(\n  tokens: {\n    \"vox_ah\": \"vox.wav\",\n  },\n)\n",
    )
    .unwrap();
    write_wav(directory.join("vox.wav"), &[1.0, 0.0, 0.0, 0.0]);
    let bank = load_sample_bank_from_directory(&directory).unwrap();
    let path = temp_wav_path();
    let cutoff_hz = 1_200.0;

    let events = vec![Event {
        whole: None,
        part: TimeSpan::new(Rational::zero(), Rational::new(1, 4).unwrap()).unwrap(),
        value: SampleTrigger::named("vox_ah").with_lpf_cutoff_hz(cutoff_hz),
    }];

    render_events_to_file_with_bank(&path, &events, 1, &bank).unwrap();

    let mut reader = hound::WavReader::open(&path).unwrap();
    let samples = reader
        .samples::<i16>()
        .take(8)
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    let filtered = apply_one_pole_low_pass(&[1.0, 0.0, 0.0, 0.0], cutoff_hz, 48_000);
    let expected = vec![
        pcm16(filtered[0] * edge_envelope(0, 4)),
        pcm16(filtered[0] * edge_envelope(0, 4)),
        pcm16(filtered[1] * edge_envelope(1, 4)),
        pcm16(filtered[1] * edge_envelope(1, 4)),
        pcm16(filtered[2] * edge_envelope(2, 4)),
        pcm16(filtered[2] * edge_envelope(2, 4)),
        pcm16(filtered[3] * edge_envelope(3, 4)),
        pcm16(filtered[3] * edge_envelope(3, 4)),
    ];

    assert_eq!(samples, expected);

    let _ = fs::remove_file(path);
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn offline_render_applies_edge_ramps_to_sample_playback() {
    let directory = temp_directory("sample-ramp-offline");
    fs::write(
        directory.join("samples.ron"),
        "(\n  tokens: {\n    \"vox_ah\": \"vox.wav\",\n  },\n)\n",
    )
    .unwrap();
    write_wav(directory.join("vox.wav"), &[1.0, 1.0, 1.0, 1.0]);
    let bank = load_sample_bank_from_directory(&directory).unwrap();
    let path = temp_wav_path();

    let events = vec![Event {
        whole: None,
        part: TimeSpan::new(Rational::zero(), Rational::new(1, 4).unwrap()).unwrap(),
        value: SampleTrigger::named("vox_ah"),
    }];

    render_events_to_file_with_bank(&path, &events, 1, &bank).unwrap();

    let mut reader = hound::WavReader::open(&path).unwrap();
    let samples = reader
        .samples::<i16>()
        .take(8)
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    let left = [samples[0], samples[2], samples[4], samples[6]];

    assert_eq!(samples.len(), 8);
    assert!(left[0] > 0);
    assert!(left[0] < left[1]);
    assert_eq!(left[1], left[2]);
    assert!(left[3] > 0);
    assert!(left[3] < left[2]);
    assert_eq!(samples[0], samples[1]);
    assert_eq!(samples[6], samples[7]);

    let _ = fs::remove_file(path);
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn offline_render_supports_negative_rate_reverse_playback() {
    let directory = temp_directory("sample-reverse-offline");
    fs::write(
        directory.join("samples.ron"),
        "(\n  tokens: {\n    \"vox_ah\": \"vox.wav\",\n  },\n)\n",
    )
    .unwrap();
    write_wav(directory.join("vox.wav"), &[0.1, 0.2, 0.3, 0.4]);
    let bank = load_sample_bank_from_directory(&directory).unwrap();
    let path = temp_wav_path();

    let events = vec![Event {
        whole: None,
        part: TimeSpan::new(Rational::zero(), Rational::new(1, 4).unwrap()).unwrap(),
        value: SampleTrigger::named("vox_ah").with_rate(-1.0),
    }];

    render_events_to_file_with_bank(&path, &events, 1, &bank).unwrap();

    let mut reader = hound::WavReader::open(&path).unwrap();
    let samples = reader
        .samples::<i16>()
        .take(8)
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    let expected = vec![
        pcm16(0.4 * edge_envelope(0, 4)),
        pcm16(0.4 * edge_envelope(0, 4)),
        pcm16(0.3 * edge_envelope(1, 4)),
        pcm16(0.3 * edge_envelope(1, 4)),
        pcm16(0.2 * edge_envelope(2, 4)),
        pcm16(0.2 * edge_envelope(2, 4)),
        pcm16(0.1 * edge_envelope(3, 4)),
        pcm16(0.1 * edge_envelope(3, 4)),
    ];

    assert_eq!(samples, expected);

    let _ = fs::remove_file(path);
    fs::remove_dir_all(directory).unwrap();
}

fn temp_directory(name: &str) -> PathBuf {
    let directory =
        std::env::temp_dir().join(format!("orpheus-dsp-{}-{name}", unique_temp_suffix()));
    fs::create_dir_all(&directory).unwrap();
    directory
}

fn temp_wav_path() -> PathBuf {
    std::env::temp_dir().join(format!("orpheus-dsp-render-{}.wav", unique_temp_suffix()))
}

fn unique_temp_suffix() -> String {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let counter = UNIQUE_TEMP_ID.fetch_add(1, Ordering::Relaxed);
    format!("{timestamp}-{counter}")
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

#[allow(clippy::cast_possible_truncation)]
fn apply_one_pole_low_pass(input: &[f32], cutoff_hz: f64, sample_rate_hz: u32) -> Vec<f32> {
    let alpha = low_pass_alpha(cutoff_hz, sample_rate_hz);
    let mut state = 0.0_f64;
    input
        .iter()
        .map(|sample| {
            state += alpha * (f64::from(*sample) - state);
            state as f32
        })
        .collect()
}

fn low_pass_alpha(cutoff_hz: f64, sample_rate_hz: u32) -> f64 {
    let cutoff_hz = normalized_cutoff_hz(cutoff_hz, sample_rate_hz);
    let omega = (std::f64::consts::TAU * cutoff_hz) / f64::from(sample_rate_hz);
    omega / (1.0 + omega)
}

fn normalized_cutoff_hz(cutoff_hz: f64, sample_rate_hz: u32) -> f64 {
    let nyquist = (f64::from(sample_rate_hz) / 2.0) - 1.0;
    cutoff_hz.clamp(1.0, nyquist.max(1.0))
}

fn edge_envelope(frame_index: u32, total_frames: u32) -> f32 {
    let ramp_frames = total_frames.div_ceil(2).clamp(1, 32);
    let attack = normalized_edge_gain(frame_index, ramp_frames);
    let release = normalized_edge_gain(
        total_frames.saturating_sub(frame_index.saturating_add(1)),
        ramp_frames,
    );
    attack.min(release)
}

#[allow(clippy::cast_precision_loss)]
fn normalized_edge_gain(distance_from_edge: u32, ramp_frames: u32) -> f32 {
    (((distance_from_edge as f32) + 0.5) / (ramp_frames as f32)).min(1.0)
}

#[allow(clippy::cast_possible_truncation)]
fn pcm16(sample: f32) -> i16 {
    (sample.clamp(-1.0, 1.0) * f32::from(i16::MAX)).round() as i16
}
