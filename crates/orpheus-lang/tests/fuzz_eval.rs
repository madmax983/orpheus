use orpheus_lang::{ReplMode, eval_module};
use proptest::prelude::*;

proptest! {
    #[test]
    fn does_not_panic(value in any::<f64>()) {
        if !value.is_finite() || value <= 0.0 || value > 1e6 { return Ok(()); }
        let code = format!("a = every({value}, rev, bd sn)");
        let result = eval_module(&code, ReplMode::Loose);
        if let Ok(module) = result {
            if let Some(pattern) = module.get("a").and_then(|v| v.as_sample_pattern()) {
                let _ = pattern.query_unit();
            }
        }
    }
}
