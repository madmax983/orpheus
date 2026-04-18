use orpheus_lang::ReplMode;
use orpheus_lang::eval_module;

#[test]
fn test_havoc_stack_overflow_meter_eval() {
    let mut source = "bd".to_string();
    for _ in 0..300 {
        source = format!("({source})");
    }
    source = format!("notes = {source}");

    let result = eval_module(&source, ReplMode::Loose);
    // Either parse error or eval error, but not an abort
    if let Err(e) = result {
        assert!(
            e.to_string().contains("exceeded") || e.to_string().contains("limit"),
            "Unexpected error: {}",
            e
        );
    }
}
