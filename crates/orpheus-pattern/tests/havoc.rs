use orpheus_pattern::Rational;
use proptest::prelude::*;

proptest! {
    #[test]
    fn havoc_test_rational_add_overflow(num1 in any::<i128>(), den1 in any::<i128>(), num2 in any::<i128>(), den2 in any::<i128>()) {
        #[allow(clippy::collapsible_if)]
        if let Ok(r1) = Rational::checked_from_parts(num1, den1) {
            if let Ok(r2) = Rational::checked_from_parts(num2, den2) {
                let _ = r1.checked_add(&r2);
                let _ = r1.checked_sub(&r2);
                let _ = r1.checked_mul(&r2);
                let _ = r1.checked_cmp(&r2);
            }
        }
    }
}
