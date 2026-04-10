#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &str| {
    let mut session = orpheus_lang::ReplSession::with_engine(orpheus_dsp::EngineHandle::stub());
    let _ = session.eval_line(data);
});
