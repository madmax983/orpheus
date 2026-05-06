use orpheus_lang::f64_to_rational;
use proptest::prelude::*;

proptest! {
    #[test]
    fn havoc_f64_to_rational_fuzz(f in proptest::num::f64::ANY, context in "\\PC*") {
        let _ = f64_to_rational(f, &context);
    }
}
