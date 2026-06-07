use orpheus_pattern::Rational;
use proptest::prelude::*;

proptest! {
    #[test]
    fn test_havoc_rational_addition_no_panic(
        a_num in any::<i128>(),
        a_den in 1..=i128::MAX,
        b_num in any::<i128>(),
        b_den in 1..=i128::MAX
    ) {
        if let (Ok(a), Ok(b)) = (
            Rational::checked_from_parts(a_num, a_den),
            Rational::checked_from_parts(b_num, b_den)
        ) {
            let result = std::panic::catch_unwind(|| {
                let _ = a.checked_add(&b);
            });
            assert!(result.is_ok(), "Rational::checked_add panicked for a={a:?} and b={b:?}");
        }
    }
}
