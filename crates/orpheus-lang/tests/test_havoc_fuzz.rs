use orpheus_lang::ReplSession;
use orpheus_dsp::EngineHandle;

proptest::proptest! {
    /// 👺 Havoc: Tests that parsing garbage bytes via the REPL eval loop does not crash the system.
    #[test]
    fn havoc_fuzz_eval(s in "[a-zA-Z0-9_ \\+\\-\\*\\/\\(\\)=|>:]*") {
        let mut session = ReplSession::with_engine(EngineHandle::stub());
        let _ = session.eval_line(&s);
    }
}
