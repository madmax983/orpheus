use orpheus_lang::{eval_module, ReplMode, export_sample_pattern_to_json};
use std::path::Path;

#[test]
fn test_havoc_export_oom() {
    let source = "pattern = bd sn cp";
    let module = eval_module(source, ReplMode::Loose).unwrap();
    let pattern = module.get("pattern").unwrap().as_sample_pattern().unwrap();

    let result = export_sample_pattern_to_json(pattern, Path::new("test.json"), 1_000_000);
    assert!(result.is_err());
    assert_eq!(result.unwrap_err().to_string(), "evaluation exceeded the maximum allowed event limit");
}
