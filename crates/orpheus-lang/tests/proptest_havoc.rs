use orpheus_lang::f64_to_rational;
use proptest::prelude::*;

proptest! {
    #[test]
    fn havoc_f64_to_rational_proptest(val in any::<f64>()) {
        // We just want to ensure it doesn't panic. It's allowed to return an Error.
        let _ = f64_to_rational(val, "proptest");
    }
}
