use orpheus_dsp::EngineHandle;
use orpheus_lang::ReplSession;
use orpheus_lang::format_cycle_position;

#[test]
fn test_format_cycle_position_zero() {
    let session = ReplSession::with_engine(EngineHandle::stub());
    let snapshot = session.transport_snapshot();
    assert_eq!(format_cycle_position(&snapshot), "0.000");
}
