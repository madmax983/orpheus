#[test]
fn havoc_test_eval_str() {
    let _ = orpheus_lang::eval_module(&"a".repeat(100_000), orpheus_lang::ReplMode::Loose);
}
