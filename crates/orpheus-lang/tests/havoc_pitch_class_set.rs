use orpheus_lang::{ReplMode, eval_module};
use proptest::prelude::*;

proptest! {
    #[test]
    fn test_scale_degree_empty(degree in -100..=100i32) {
        // Can we trick the parser? `pitch_class_set([])` evaluates to an empty list of arguments to pitch_class_set.
        let source = format!("a = degrees(pitch_class_set([]), {})", degree);
        let env = eval_module(&source, ReplMode::Loose);
        if let Ok(env) = env {
            let val = env.get("a").unwrap().as_number_pattern().unwrap();
            let _ = val.query_unit();
        }
    }
}
