//! Chaos engineering tests verifying that `rem_euclid` operations
//! are properly bounded by non-zero guards to prevent Thread Panics.
use orpheus_lang::{ReplMode, eval_module};

#[test]
fn test_havoc_every_zero() {
    let source = "notes = every(0, rev, \"c\")";
    let module = eval_module(source, ReplMode::Loose);
    assert!(module.is_err());
}

#[test]
fn test_havoc_when_zero() {
    let source = "notes = when(0, 1, rev, \"c\")";
    let module = eval_module(source, ReplMode::Loose);
    assert!(module.is_err());
}

#[test]
fn test_havoc_euclid_zero() {
    let source = "notes = euclid(8, 0)(\"c\")";
    let module = eval_module(source, ReplMode::Loose);
    assert!(module.is_err());
}

#[test]
fn test_havoc_degrees_empty() {
    let source = "notes = degrees(\"[1]\", pcs(\"\"))";
    let module = eval_module(source, ReplMode::Loose);
    assert!(module.is_err());
}
