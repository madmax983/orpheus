use std::process::Command;

#[test]
fn test_print_help_output() {
    let output = Command::new(env!("CARGO_BIN_EXE_orpheus"))
        .arg("--help")
        .output()
        .expect("Failed to execute process");

    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("Orpheus"));
    assert!(stdout.contains("Usage:"));
    assert!(stdout.contains("Arguments:"));
    assert!(stdout.contains("Options:"));
}

#[test]
fn test_print_version_output() {
    let output = Command::new(env!("CARGO_BIN_EXE_orpheus"))
        .arg("--version")
        .output()
        .expect("Failed to execute process");

    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("orpheus"));
    assert!(stdout.contains(env!("CARGO_PKG_VERSION")));
}

#[test]
fn test_invalid_argument_output() {
    let output = Command::new(env!("CARGO_BIN_EXE_orpheus"))
        .arg("--invalid")
        .output()
        .expect("Failed to execute process");

    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("unexpected argument"));
    assert!(stderr.contains("--invalid"));
}
