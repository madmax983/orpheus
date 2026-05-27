use orpheus_lang::{ReplMode, eval_module};
#[test]
fn test_havoc_evaluation_limits() {
    let source_fast = "notes = fast(20000000000000, bd)";
    let res_fast = eval_module(source_fast, ReplMode::Loose);
    assert!(res_fast.is_err());
    assert!(
        res_fast
            .unwrap_err()
            .to_string()
            .contains("exceeded the maximum allowed bound")
    );

    let source_every = "notes = every(0, rev, bd)";
    let res_every = eval_module(source_every, ReplMode::Loose);
    assert!(res_every.is_err());

    let source_sections = "notes = seq_sections(section(bd, 200000000000))";
    let res_sections = eval_module(source_sections, ReplMode::Loose);
    assert!(res_sections.is_err());
}
