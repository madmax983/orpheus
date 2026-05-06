use orpheus_lang::parse_named_pitch_literal;
use proptest::prelude::*;

proptest! {
    #[test]
    fn havoc_parse_named_pitch_literal_fuzz(s in "\\PC*") {
        let _ = parse_named_pitch_literal(&s);
    }
}
