use orpheus_pattern::Rational;
use proptest::prelude::*;

proptest! {
    #[test]
    fn havoc_rational_add_panic(num1 in any::<i128>(), den1 in 1_i128..=i128::MAX, num2 in any::<i128>(), den2 in 1_i128..=i128::MAX) {
        #[allow(clippy::collapsible_if)]
        if let Ok(r1) = Rational::checked_from_parts(num1, den1) {
            if let Ok(r2) = Rational::checked_from_parts(num2, den2) {
                // This triggers the Add trait which panics on overflow
                let _ = r1 + r2;
            }
        }
    }
}
