use orpheus_lang::ReplMode;
use orpheus_lang::eval_module;

#[test]
fn test_havoc_stack_overflow() {
    let source = "f x = x(x)\nomega = f(f)";
    let result = eval_module(source, ReplMode::Loose);
    assert!(result.is_err(), "Expected an error but got: {result:?}");
}
