use orpheus_dsp::EngineHandle;
use orpheus_lang::ReplSession;

/// 👺 Havoc: Tests that evaluating cyclic variables with unbounded iterators does not stack overflow or OOM
#[test]
fn havoc_cyclic_resilience() {
    let mut session = ReplSession::with_engine(EngineHandle::stub());
    let _ = session.eval_line("a = b\nb = a");
    let _ = session.eval_line("res = a");
}

/// 👺 Havoc: Tests that structurally invalid forms (like scaling a boolean) do not crash
#[test]
fn havoc_type_resilience() {
    let mut session = ReplSession::with_engine(EngineHandle::stub());
    let _ = session.eval_line("res = struct(\"t f\", time_shift(NaN, bd))");
    let _ = session.eval_line(":export res stems 10");
}
