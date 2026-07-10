//! Smoke tests for the REPL binary.
use assert_cmd::cargo::cargo_bin_cmd;
use predicates::str::contains;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

static UNIQUE_TEMP_ID: AtomicU64 = AtomicU64::new(0);

fn temp_wav_path() -> std::path::PathBuf {
    std::env::temp_dir().join(format!("orpheus-smoke-render-{}.wav", unique_temp_suffix()))
}

fn temp_directory(name: &str) -> PathBuf {
    let directory = std::env::temp_dir().join(format!("orpheus-{name}-{}", unique_temp_suffix()));
    fs::create_dir_all(&directory).unwrap();
    directory
}

fn unique_temp_suffix() -> String {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let counter = UNIQUE_TEMP_ID.fetch_add(1, Ordering::Relaxed);
    format!("{timestamp}-{counter}")
}

fn write_wav(path: PathBuf, frames: &[f32]) {
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

#[test]
fn repl_accepts_pattern_and_reports_success() {
    let mut cmd = cargo_bin_cmd!("orpheus");

    cmd.write_stdin("drums = bd sn cp sn\n:quit\n")
        .assert()
        .success()
        .stdout(contains(
            "\u{2713} bound drums = Pattern<Sample>: Pattern<Sample>",
        ));
}

#[test]
fn repl_ignores_blank_lines_before_quit() {
    let mut cmd = cargo_bin_cmd!("orpheus");

    // the prompt is colorized, so we just check for multiple ">" appearances
    cmd.write_stdin("\n\n:quit\n")
        .assert()
        .success()
        .stdout(contains(">").count(3));
}

#[test]
fn repl_reports_eval_errors_to_stderr() {
    let mut cmd = cargo_bin_cmd!("orpheus");

    cmd.write_stdin("drums = nope\n:quit\n")
        .assert()
        .success()
        .stderr(contains("unresolved identifier"));
}

#[test]
fn repl_reuses_prior_bindings_across_lines() {
    let mut cmd = cargo_bin_cmd!("orpheus");

    cmd.write_stdin("drums = bd sn cp sn\ncopy = drums\n:quit\n")
        .assert()
        .success()
        .stdout(contains(
            "\u{2713} bound drums = Pattern<Sample>: Pattern<Sample>",
        ))
        .stdout(contains(
            "\u{2713} bound copy = Pattern<Sample>: Pattern<Sample>",
        ));
}

#[test]
fn repl_prints_inferred_function_types() {
    let mut cmd = cargo_bin_cmd!("orpheus");

    cmd.write_stdin("warp = fast(2)\n:quit\n")
        .assert()
        .success()
        .stdout(contains(
            "\u{2713} bound warp = Function(Builtin): Function(",
        ));
}

#[test]
fn repl_render_command_exports_wav() {
    let mut cmd = cargo_bin_cmd!("orpheus");
    let path = temp_wav_path();

    cmd.write_stdin(format!(
        "song = bd sn cp sn\n:render song {} 2\n:quit\n",
        path.display()
    ))
    .assert()
    .success()
    .stdout(contains("rendered `song`"));

    assert!(path.exists());
    assert!(fs::metadata(&path).unwrap().len() > 44);
    let _ = fs::remove_file(path);
}

#[test]
#[allow(clippy::cast_precision_loss)]
fn repl_export_master_command_exports_non_silent_wav() {
    let mut cmd = cargo_bin_cmd!("orpheus");
    let path = temp_wav_path();

    // A user `voice { }` instrument plus a sample drum pattern, both routed to
    // the master, prove the single-file master render includes graph voices and
    // sample voices summed together.
    cmd.write_stdin(format!(
        "pluck = voice {{ sine(freq) * ar(gate, 0.001, 0.2) }}\n\
         lead = pluck ~ pluck ~\n\
         drums = bd sn cp sn\n\
         :track new leadtrk\n\
         :track bind leadtrk lead\n\
         :track new drumtrk\n\
         :track bind drumtrk drums\n\
         :export master {} 2\n\
         :quit\n",
        path.display()
    ))
    .assert()
    .success()
    .stdout(contains("exported master"));

    assert!(path.exists());
    assert!(fs::metadata(&path).unwrap().len() > 44);

    // Read the master back and prove it is a valid, non-silent stereo WAV.
    let mut reader = hound::WavReader::open(&path).unwrap();
    let spec = reader.spec();
    assert_eq!(spec.channels, 2, "master must be stereo");
    let samples = reader
        .samples::<i16>()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    assert!(!samples.is_empty(), "master must contain audio");
    let peak = samples.iter().map(|s| s.unsigned_abs()).max().unwrap();
    let rms = (samples
        .iter()
        .map(|s| f64::from(*s) * f64::from(*s))
        .sum::<f64>()
        / samples.len() as f64)
        .sqrt();
    assert!(peak > 200, "master should be non-silent (peak = {peak})");
    assert!(rms > 10.0, "master should carry real energy (rms = {rms})");

    let _ = fs::remove_file(path);
}

#[test]
fn repl_tempo_command_reports_success() {
    let mut cmd = cargo_bin_cmd!("orpheus");

    cmd.write_stdin(":tempo 90\n:quit\n")
        .assert()
        .success()
        .stdout(contains("tempo set to 90 BPM"));
}

#[test]
fn repl_stop_and_play_commands_report_success() {
    let mut cmd = cargo_bin_cmd!("orpheus");

    cmd.write_stdin(":stop\n:play\n:quit\n")
        .assert()
        .success()
        .stdout(contains("transport stopped"))
        .stdout(contains("transport playing"));
}

#[test]
fn repl_renders_manifest_backed_sample_tokens() {
    let mut cmd = cargo_bin_cmd!("orpheus");
    let directory = temp_directory("sample-pack");
    fs::write(
        directory.join("samples.ron"),
        "(\n  tokens: {\n    \"vox_ah\": \"vox.wav\",\n  },\n)\n",
    )
    .unwrap();
    write_wav(directory.join("vox.wav"), &[0.33, 0.0, 0.0, 0.0]);
    let path = temp_wav_path();

    cmd.write_stdin(format!(
        ":samples {}\nsong = sample(\"vox_ah\")\n:render song {}\n:quit\n",
        directory.display(),
        path.display()
    ))
    .assert()
    .success()
    .stdout(contains("loaded sample overrides"))
    .stdout(contains("rendered `song`"));

    assert!(path.exists());
    assert!(fs::metadata(&path).unwrap().len() > 44);
    let _ = fs::remove_file(path);
    let _ = fs::remove_dir_all(directory);
}
