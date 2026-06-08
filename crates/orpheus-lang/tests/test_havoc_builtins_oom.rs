use orpheus_lang::{ReplMode, eval_module};

#[test]
fn test_havoc_wolfram_oom() {
    // 👺 Havoc: Trigger an OOM / capacity overflow panic by passing a massive step count to wolfram
    let res = eval_module("a = wolfram(30, 4294967295)", ReplMode::Loose);
    assert!(res.is_err());
}

#[test]
fn test_havoc_lsystem_oom() {
    let res = eval_module(
        "a = lsystem(\"A\", 1000000000, \"A:AB,B:A\")",
        ReplMode::Loose,
    );
    assert!(res.is_err());
}

#[test]
fn test_havoc_hex_oom() {
    let mut large_str = String::from("a = hex(\"");
    for _ in 0..100000 {
        large_str.push_str("FF");
    }
    large_str.push_str("\")");
    let res = eval_module(&large_str, ReplMode::Loose);
    assert!(res.is_err());
}

#[test]
fn test_havoc_bin_oom() {
    let mut large_str = String::from("a = bin(\"");
    for _ in 0..200000 {
        large_str.push_str("1");
    }
    large_str.push_str("\")");
    let res = eval_module(&large_str, ReplMode::Loose);
    assert!(res.is_err());
}

#[test]
fn test_havoc_lsystem_exponential_oom() {
    let res = eval_module("a = lsystem(\"A\", 100, \"A:AB,B:A\")", ReplMode::Loose);
    assert!(res.is_err());
}

#[test]
fn test_havoc_wolfram_squared_oom() {
    let res = eval_module("a = wolfram(30, 10000)", ReplMode::Loose);
    assert!(res.is_err());
}
