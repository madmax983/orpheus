#![no_main]

use libfuzzer_sys::fuzz_target;
use orpheus_lang::{eval_module, ReplMode};

fuzz_target!(|data: &[u8]| {
    if let Ok(s) = std::str::from_utf8(data) {
        let _ = eval_module(s, ReplMode::Loose);
    }
});
