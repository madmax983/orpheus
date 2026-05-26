use orpheus_dsp::EngineHandle;
use orpheus_lang::ReplSession;
use proptest::prelude::*;

proptest! {
    #[test]
    fn eval_line_resilience_to_garbage_strings(s in any::<String>()) {
        let mut session = ReplSession::with_engine(EngineHandle::stub());
        let _ = session.eval_line(&s);
    }
}
