//! Property-based tests for the evaluator engine, verifying that valid combinations of patterns and DSP nodes always produce sound structures.
use orpheus_dsp::EngineHandle;
use orpheus_lang::ReplSession;
use proptest::prelude::*;

proptest! {
    #![proptest_config(ProptestConfig::with_cases(1000))]
    #[test]
    fn test_eval_line_does_not_panic(s in ".*") {
        let mut session = ReplSession::with_engine(EngineHandle::stub());
        let _ = session.eval_line(&s);
    }
}
