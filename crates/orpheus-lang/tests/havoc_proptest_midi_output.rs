use orpheus_lang::ReplSession;
use orpheus_dsp::EngineHandle;
use proptest::prelude::*;

proptest! {
    #[test]
    fn havoc_midi_send_binding_garbage(binding_name in "\\PC*", channel in any::<u8>()) {
        let (engine, _) = EngineHandle::split_for_test();
        let mut session = ReplSession::with_engine(engine);
        // It will fail validation, but we check for panics
        let _ = session.eval_line(&format!(":midi send {} {}", binding_name, channel));
    }
}
