use orpheus_lang::{ReplMode, eval_module};

#[test]
fn test_divide_by_zero_every() {
    let source = "pat = every(0, rev, \"bd\")";
    let module = eval_module(source, ReplMode::Loose);
    assert!(module.is_err());
}

#[test]
fn test_divide_by_zero_when() {
    let source = "pat = when(0, 0, rev, \"bd\")";
    let module = eval_module(source, ReplMode::Loose);
    assert!(module.is_err());
}
