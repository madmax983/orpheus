use orpheus_lang::{ReplMode, eval_module};

#[test]
fn test_havoc_division_by_zero_recovery() {
    // 👺 Havoc: Using a tiny float bypassing fractional checks rounds down to 0 during formatting,
    // which then triggered a division-by-zero panic in rem_euclid for pattern operators.
    let source = "notes = every(0.0000000000000000001, rev, bd)";
    let v = eval_module(source, ReplMode::Loose);
    assert!(v.is_err());
    assert!(
        v.unwrap_err()
            .to_string()
            .contains("requires a positive integer factor")
    );
}
