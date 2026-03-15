use orpheus_lang::{ReplMode, eval_module};

#[test]
#[should_panic(expected = "`fast` factor exceeded the supported evaluator range")]
fn havoc_crash_try_query_arithmetic_overflow() {
    let payload = "crash = fast(9223372036854775807, bd)";

    let module = eval_module(payload, ReplMode::Loose).unwrap();
    let pattern = module.get("crash").unwrap().as_sample_pattern().unwrap();
    let _ = pattern.query_unit().unwrap();
}
