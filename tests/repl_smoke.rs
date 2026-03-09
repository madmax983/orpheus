use assert_cmd::cargo::cargo_bin_cmd;
use predicates::str::contains;

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
