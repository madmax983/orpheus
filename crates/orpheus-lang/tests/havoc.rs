use orpheus_lang::ReplSession;
use orpheus_dsp::EngineHandle;
use proptest::prelude::*;

proptest! {
    #![proptest_config(ProptestConfig::with_cases(5000))]
    #[test]
    fn havoc_eval_random_bytes(data in proptest::collection::vec(any::<u8>(), 0..1024)) {
        let mut session = ReplSession::with_engine(EngineHandle::stub());
        let s = String::from_utf8_lossy(&data);
        let _ = session.eval_line(&s);
    }
}
