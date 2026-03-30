use orpheus_pattern::Rational;
use proptest::prelude::*;

proptest! {
    #[test]
    fn havoc_proptest_rational_checked_add(num1 in any::<i128>(), den1 in any::<i128>(), num2 in any::<i128>(), den2 in any::<i128>()) {
        if den1 == 0 || den2 == 0 { return Ok(()); }
        if let Ok(r1) = Rational::checked_from_parts(num1, den1) {
            if let Ok(r2) = Rational::checked_from_parts(num2, den2) {
                let _ = r1.checked_add(&r2);
            }
        }
    }
}

proptest! {
    #[test]
    fn havoc_proptest_rational_checked_mul(num1 in any::<i128>(), den1 in any::<i128>(), num2 in any::<i128>(), den2 in any::<i128>()) {
        if den1 == 0 || den2 == 0 { return Ok(()); }
        if let Ok(r1) = Rational::checked_from_parts(num1, den1) {
            if let Ok(r2) = Rational::checked_from_parts(num2, den2) {
                let _ = r1.checked_mul(&r2);
            }
        }
    }
}
