use orpheus_lang::ReplMode;
use orpheus_lang::eval_module;

/// 👺 Havoc: Tests that supplying a very small fractional value to `every`
/// does not cause a division by zero panic when it rounds to 0.
#[test]
fn test_havoc_every_div_by_zero() {
    let source = "notes = every(0.00000000000000000000001, rev, bd)";
    if let Ok(env) = eval_module(&source, ReplMode::Loose) {
        if let Some(pattern) = env.get("notes") {
            if let Some(sample_pat) = pattern.as_sample_pattern() {
                let _ = sample_pat.query_unit();
            }
        }
    }
}
