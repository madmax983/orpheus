use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use assert_cmd::Command;

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join(name)
}

#[test]
fn startup_ode_argument_preloads_bindings_for_repl_commands() {
    let song = fixture("song.ode");
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let output = std::env::temp_dir().join(format!("orpheus-startup-preload-{unique}.wav"));

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
