use orpheus_lang::{ReplMode, eval_module};
use proptest::prelude::*;

proptest! {
    #[test]
    fn test_havoc_proptest_negative_meter(
        beats in any::<f64>(),
        unit in any::<f64>(),
        beat in any::<f64>(),
    ) {
        if beats.is_finite() && unit.is_finite() && beat.is_finite() {
            let source = format!("notes = meter({beats}, {unit}, at(beat({beat}), bd))");
            let _ = eval_module(&source, ReplMode::Loose);
        }
    }
}
