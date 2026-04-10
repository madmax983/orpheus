#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &str| {
    let _ = orpheus_lang::infer_module(data, orpheus_lang::ReplMode::Loose);
});
