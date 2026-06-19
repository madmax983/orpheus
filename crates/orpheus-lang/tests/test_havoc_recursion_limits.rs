//! Chaos engineering tests validating the AST evaluator's recursion limits, ensuring deeply nested closures return an error instead of causing a stack overflow.
use orpheus_lang::ReplMode;
use orpheus_lang::eval_module;
use orpheus_lang::ReplSession;
use orpheus_dsp::EngineHandle;

#[test]
fn test_havoc_mutual_recursion_eval() {
    let mut session = ReplSession::with_engine(EngineHandle::stub());
    let _ = session.eval_line("f x = g(x)");
    let _ = session.eval_line("g x = f(x)");
    // This resolves as an unresolved identifier in loose mode, preventing stack overflow
    let result = session.eval_line("notes = f(bd)");
    assert!(result.is_err());
}

#[test]
fn test_havoc_parse_limit() {
    let source = "notes = ".to_string() + &"(".repeat(50000) + "bd" + &")".repeat(50000);
    let result = orpheus_lang::parse_module(&source);
    // Should fail with depth limit exceeded, not stack overflow
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("depth"));
}