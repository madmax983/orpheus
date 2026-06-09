use orpheus_lang::{ReplMode, eval_module};

#[test]
fn test_havoc_hex_oom() {
    let mut big_hex = String::new();
    for _ in 0..100001 {
        big_hex.push_str("F");
    }
    let src = format!("y = hex(\"{}\")", big_hex);
    let res = eval_module(&src, ReplMode::Loose);
    assert!(res.is_err());
    assert!(res.unwrap_err().to_string().contains("evaluator limit"));
}

#[test]
fn test_havoc_bin_oom() {
    let mut big_bin = String::new();
    for _ in 0..100001 {
        big_bin.push_str("1");
    }
    let src = format!("y = bin(\"{}\")", big_bin);
    let res = eval_module(&src, ReplMode::Loose);
    assert!(res.is_err());
    assert!(res.unwrap_err().to_string().contains("evaluator limit"));
}

#[test]
fn test_havoc_lsystem_oom() {
    let src = "y = lsystem(\"A\", 100, \"A:AA\")";
    let res = eval_module(src, ReplMode::Loose);
    assert!(res.is_err());
    assert!(res.unwrap_err().to_string().contains("evaluator limit"));
}
