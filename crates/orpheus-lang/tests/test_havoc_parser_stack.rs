use orpheus_lang::parse_module;

#[test]
fn test_havoc_parser_stack_overflow() {
    let mut source = String::new();
    source.push_str("a = ");
    for _ in 0..10000 {
        source.push_str("(");
    }
    source.push_str("bd");
    for _ in 0..10000 {
        source.push_str(")");
    }
    let _ = parse_module(&source);
}
