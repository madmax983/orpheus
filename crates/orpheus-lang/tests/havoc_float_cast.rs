use orpheus_dsp::EngineHandle;
use orpheus_lang::ReplSession;

#[test]
fn test_havoc_float_cast_truncation() {
    let mut session = ReplSession::with_engine(EngineHandle::stub());
    let source = "x = seq(1.7014118346046923e38, bd, sn)";
    let result = session.eval_line(source);
    // If it panics due to precision issues causing out-of-bounds, this will crash.
    println!("{:?}", result);
}
