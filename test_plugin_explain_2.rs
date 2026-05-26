use orpheus_lang::{eval_module, ReplMode};
fn main() {
    let module = eval_module("p = vst(\"test\")", ReplMode::Loose).unwrap();
    // Use downcast here? Or maybe we can just query it through the session.
}
