use orpheus_lang::{eval_module, ReplMode, export_number_pattern_to_midi};
use std::fs;

fn main() {
    let module = eval_module("pat = 60 62 64 65", ReplMode::Loose).unwrap();
    let pat = module.get("pat").unwrap().as_number_pattern().unwrap();
    let path = std::env::temp_dir().join("havoc_huge_export.mid");

    // We expect this to panic or OOM if not bounded.
    let _ = export_number_pattern_to_midi(pat, &path, u64::MAX);
}
