use orpheus_lang::ReplMode;
use orpheus_lang::eval_module;
use orpheus_lang::f64_to_rational;
use proptest::prelude::*;

proptest! {
    #[test]
    fn doesn_crash_f64_to_rational_large(f in -1e200..1e200) {
        let _ = f64_to_rational(f, "fuzzing");
    }

    #[test]
    fn doesn_crash_eval_module_large_float(f in -1e200..1e200) {
        let code = format!("x = meter({}, 4)", f);
        let _ = eval_module(&code, ReplMode::Strict);
    }
}
