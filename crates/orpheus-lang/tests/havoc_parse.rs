use orpheus_lang::parse_module;
use proptest::prelude::*;

proptest! {
    #![proptest_config(ProptestConfig::with_cases(10))]
    #[test]
    fn test_havoc_capacity_overflow_parse(depth in 1..200_usize) {
        let mut source = "bd".to_string();
        for _ in 0..depth {
            source = format!("fast(2, {source})");
        }
        source = format!("notes = {source}");

        let res = parse_module(&source);
        if depth > 64 {
            assert!(res.is_err());
        }
    }

    #[test]
    fn test_havoc_capacity_overflow_parse_implicit(depth in 1..200_usize) {
        let mut source = "bd".to_string();
        for _ in 0..depth {
            source = format!("fast 2 {source}");
        }
        source = format!("notes = {source}");

        let res = parse_module(&source);
        if depth > 64 {
            assert!(res.is_err());
        }
    }
}
