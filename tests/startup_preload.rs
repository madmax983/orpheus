//! Tests for startup file preloading.
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use assert_cmd::Command;

static UNIQUE_TEMP_ID: AtomicU64 = AtomicU64::new(0);

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join(name)
}

fn docs_example(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("docs")
        .join("examples")
        .join(name)
}

#[test]
fn startup_ode_argument_preloads_bindings_for_repl_commands() {
    let song = fixture("song.ode");
    let output = std::env::temp_dir().join(format!(
        "orpheus-startup-preload-{}.wav",
        unique_temp_suffix()
    ));

    let assert = Command::new(env!("CARGO_BIN_EXE_orpheus"))
        .arg(&song)
        .write_stdin(format!(":render song {}\n:quit\n", output.display()))
        .assert()
        .success();

    let stdout = String::from_utf8(assert.get_output().stdout.clone()).unwrap();
    assert!(stdout.contains("rendered `song`"));
    assert!(output.exists());
    assert!(fs::metadata(&output).unwrap().len() > 44);

    let _ = fs::remove_file(output);
}

#[test]
fn startup_ode_argument_accepts_multi_binding_example() {
    let song = docs_example("phase5_escape_hatch.ode");
    let output = std::env::temp_dir().join(format!(
        "orpheus-phase5-startup-{}.wav",
        unique_temp_suffix()
    ));

    let assert = Command::new(env!("CARGO_BIN_EXE_orpheus"))
        .arg(&song)
        .write_stdin(format!(":render song {}\n:quit\n", output.display()))
        .assert()
        .success();

    let stdout = String::from_utf8(assert.get_output().stdout.clone()).unwrap();
    assert!(stdout.contains("rendered `song`"));
    assert!(output.exists());
    assert!(fs::metadata(&output).unwrap().len() > 44);

    let _ = fs::remove_file(output);
}

fn unique_temp_suffix() -> String {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let counter = UNIQUE_TEMP_ID.fetch_add(1, Ordering::Relaxed);
    format!("{timestamp}-{counter}")
}
