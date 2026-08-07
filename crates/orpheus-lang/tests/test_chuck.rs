use orpheus_lang::{ReplMode, eval_module, export_sample_pattern_to_chuck};

#[test]
fn test_chuck_integration() {
    let source = "pattern = bd sn";
    let module = eval_module(source, ReplMode::Loose).unwrap();
    let pattern = module.get("pattern").unwrap().as_sample_pattern().unwrap();

    export_sample_pattern_to_chuck(pattern, "test_sample.ck", 2).unwrap();

    let contents = std::fs::read_to_string("test_sample.ck").unwrap();
    assert!(contents.contains("spork ~ play_samples();"));
    std::fs::remove_file("test_sample.ck").unwrap();
}
