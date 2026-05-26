use orpheus_lang::{ReplMode, eval_module};

#[test]
fn havoc_proptest_crash() {
    let src = "notes = meter(i32::MAX, 4, at(beat(0), bd))";
    let _ = eval_module(src, ReplMode::Loose);
}
