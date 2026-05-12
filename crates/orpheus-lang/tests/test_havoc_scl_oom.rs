//! Chaos engineering tests for the Scala scale loader to ensure massive or maliciously crafted files do not cause Out-Of-Memory (OOM) panics.
use orpheus_lang::parse_scala_source;

/// 👺 Havoc: Tests that an extremely large note count declared in a Scala
/// file's header does not cause a Denial of Service (OOM/Panic) during vector allocation.
#[test]
fn test_havoc_scl_oom() {
    let mut source = String::from("! OOM test\nOOM scale\n");
    // Declare an unbacked expected size that is astronomically large
    source.push_str("18446744073709551615\n"); // usize::MAX
    source.push_str("2/1\n");

    // We expect this to either Err quickly or panic, we are checking for panic here.
    // The test framework catches the panic and marks it as a failure, so if it finishes without crashing,
    // we handled it successfully.
    let result = parse_scala_source(&source, "oom_test");
    assert!(
        result.is_err(),
        "Expected an error due to note count mismatch"
    );
}
