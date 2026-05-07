use proptest::prelude::*;
use orpheus_lang::{
    parse_module, eval_module, eval_into_bindings, f64_to_rational,
    parse_named_pitch_literal, escape_json_string, parse_scala_source,
    ReplMode,
};
use std::collections::BTreeMap;

proptest! {
    #[test]
    fn test_parse_module_does_not_panic(s in "\\PC*") {
        let _ = parse_module(&s);
    }

    #[test]
    fn test_eval_module_does_not_panic(s in "\\PC*") {
        let _ = eval_module(&s, ReplMode::Loose);
    }

    #[test]
    fn test_eval_into_bindings_does_not_panic(s in "\\PC*") {
        let mut bindings = BTreeMap::new();
        let _ = eval_into_bindings(&s, ReplMode::Loose, &mut bindings);
    }

    #[test]
    fn test_f64_to_rational_does_not_panic(value in any::<f64>()) {
        let _ = f64_to_rational(value, "context");
    }

    #[test]
    fn test_parse_named_pitch_literal_does_not_panic(s in "\\PC*") {
        let _ = parse_named_pitch_literal(&s);
    }

    #[test]
    fn test_escape_json_string_does_not_panic(s in "\\PC*") {
        let _ = escape_json_string(&s);
    }

    #[test]
    fn test_parse_scala_source_does_not_panic(s in "\\PC*") {
        let _ = parse_scala_source(&s, "test");
    }
}
