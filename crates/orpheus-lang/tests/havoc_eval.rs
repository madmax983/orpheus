use orpheus_lang::ReplMode;
use orpheus_lang::eval_module;
use proptest::prelude::*;
use std::process::Command;

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]
    #[test]
    fn test_havoc_capacity_overflow_eval(depth in 1..100_usize, factor in 2..10_i64) {
        let mut source = "bd".to_string();
        for _ in 0..depth {
            source = format!("fast({factor}, {source})");
        }
        source = format!("notes = {source}");

        let module = eval_module(&source, ReplMode::Loose);
        #[allow(clippy::collapsible_if)]
        if let Ok(module) = module {
            if let Some(val) = module.get("notes") {
                // It should return an EvalError instead of triggering an OOM abort
                let _ = val.as_sample_pattern().unwrap().query_unit();
            }
        }
    }
}

use std::os::unix::process::ExitStatusExt;

#[test]
fn test_havoc_stack_overflow() {
    // The Orpheus CLI currently expects `orpheus [path]`.
    // Let's just create a temporary file with the script, and pass its path to `cargo run`
    let script = "f x = x(x)\nerr = f(f)";
    let temp_dir = std::env::temp_dir();
    let temp_file = temp_dir.join("havoc_stack_overflow.ode");
    std::fs::write(&temp_file, script).unwrap();

    let output = Command::new("cargo")
        .arg("run")
        .arg("--manifest-path")
        .arg("../../Cargo.toml")
        .arg("--")
        .arg(temp_file.to_str().unwrap())
        .output()
        .expect("Failed to execute cargo run");

    // Check if the process aborted due to stack overflow (signal 6 SIGABRT or signal 11 SIGSEGV).
    let sig = output.status.signal();
    assert!(
        sig.is_none(),
        "Process was killed by signal (likely stack overflow): {sig:?}"
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(
        stderr.contains("depth exceeded")
            || stdout.contains("depth exceeded")
            || output.status.success()
            || stderr.contains("maximum evaluation depth exceeded"),
        "Should return an eval error about evaluation depth. \nStdout: {stdout}\nStderr: {stderr}",
    );

    std::fs::remove_file(temp_file).unwrap();
}
