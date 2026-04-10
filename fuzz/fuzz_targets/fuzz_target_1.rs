#![no_main]

use libfuzzer_sys::fuzz_target;
use orpheus_lang::{eval_module, ReplMode, export_number_pattern_to_tracker, export_sample_pattern_to_tracker};
use std::path::PathBuf;

fuzz_target!(|data: &[u8]| {
    if let Ok(s) = std::str::from_utf8(data) {
        if let Ok(module) = eval_module(s, ReplMode::Loose) {
            for (_, value) in module {
                if let Some(sample_pat) = value.as_sample_pattern() {
                    let _ = export_sample_pattern_to_tracker(sample_pat, PathBuf::from("/dev/null"), 1);
                } else if let Some(num_pat) = value.as_number_pattern() {
                    let _ = export_number_pattern_to_tracker(num_pat, PathBuf::from("/dev/null"), 1);
                }
            }
        }
    }
});