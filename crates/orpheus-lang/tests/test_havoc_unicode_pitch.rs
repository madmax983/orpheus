use orpheus_lang::ReplMode;
use orpheus_lang::eval_module;

#[test]
fn test_havoc_pitch_literal_unicode_graceful_rejection() {
    let result = eval_module("note = c世4", ReplMode::Loose);
    // Evaluating should result in a parse error or eval error, but not panic
    assert!(result.is_err());
}
