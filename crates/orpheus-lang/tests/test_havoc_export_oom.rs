use orpheus_lang::ReplMode;
use orpheus_lang::eval_module;
use orpheus_lang::render_sample_pattern_to_file;

#[test]
fn test_havoc_render_sample_pattern_oom() {
    let source = "pattern = fast(1000, bd sn cp)";
    let module = eval_module(source, ReplMode::Loose).unwrap();
    let pattern = module.get("pattern").unwrap().as_sample_pattern().unwrap();

    let path = std::env::temp_dir().join("test_havoc_oom.wav");
    // Extremely large cycle limit (if it doesn't fail fast, might OOM).
    let result = render_sample_pattern_to_file(pattern, &path, 100_000);
    assert!(
        result.is_err(),
        "Should have failed due to maximum step/cycle limit"
    );
    let _ = std::fs::remove_file(path);
}
