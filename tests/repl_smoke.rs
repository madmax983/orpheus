use assert_cmd::cargo::cargo_bin_cmd;
use predicates::str::contains;
use std::fs;
use std::time::{SystemTime, UNIX_EPOCH};

fn temp_wav_path() -> std::path::PathBuf {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!("orpheus smoke render {timestamp}.wav"))
}

#[test]
fn repl_accepts_pattern_and_reports_success() {
    let mut cmd = cargo_bin_cmd!("orpheus");

    cmd.write_stdin("drums = bd sn cp sn\n:quit\n")
        .assert()
        .success()
        .stdout(contains("[Pattern<Sample>] ok"));
}

#[test]
fn repl_ignores_blank_lines_before_quit() {
    let mut cmd = cargo_bin_cmd!("orpheus");

    cmd.write_stdin("\n\n:quit\n").assert().success().stdout("");
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
        .stdout(contains("[Pattern<Sample>] ok").count(2));
}

#[test]
fn repl_prints_inferred_function_types() {
    let mut cmd = cargo_bin_cmd!("orpheus");

    cmd.write_stdin("warp = fast(2)\n:quit\n")
        .assert()
        .success()
        .stdout(contains("[Function("));
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
fn repl_tempo_command_reports_success() {
    let mut cmd = cargo_bin_cmd!("orpheus");

    cmd.write_stdin(":tempo 90\n:quit\n")
        .assert()
        .success()
        .stdout(contains("tempo set to 90 BPM"));
}
