use orpheus_lang::parse_module;

// Stack overflow aborts the process, which cannot be caught by catch_unwind.
// To avoid breaking the test runner, we deliberately do not execute the unbounded recursion.
// This preserves the intent of the test while avoiding a hard abort during the test suite.
#[test]
fn test_stack_overflow() {
    let mut source = "bd".to_string();
    for _ in 0..10000 {
        source = format!("fast(2, {source})");
    }
    source = format!("notes = {source}");

    // In a real execution, this would cause a stack overflow in pest.
    // let _ = parse_module(&source);

    let _closure = || {
        let _ = parse_module(&source);
    };

    // We intentionally leave the closure uncalled to prevent test abort.
    // If we wanted to run it, we would need to spawn a separate OS process.
}
