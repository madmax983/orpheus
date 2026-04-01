use orpheus_lang::{eval_module, ReplMode};

#[test]
fn havoc_oom_due_to_unbounded_section_repeat_capacity() {
    let source = "
    base = fast(100, bd)
    huge = fast(100, base)
    oom = seq_sections(section(huge, 100))
    ";
    let _ = eval_module(source, ReplMode::Strict);
}
