use orpheus_lang::parse_module;

#[test]
fn test_stack_overflow() {
    let mut source = "bd".to_string();
    for _ in 0..10000 {
        source = format!("fast(2, {source})");
    }
    source = format!("notes = {source}");
    let _ = parse_module(&source);
}
