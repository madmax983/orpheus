use orpheus_lang::{ReplMode, eval_module};

#[test]
fn test_havoc_pedal_mix_coverage() {
    let source = "my_mix = graph { a = input |> mix ; a |> output }";
    let module = eval_module(source, ReplMode::Loose);
    assert!(module.is_err());
    assert_eq!(
        module.unwrap_err().to_string(),
        "`mix` requires at least two audio inputs"
    );
}

#[test]
fn test_havoc_pedal_mix_named_args() {
    let source = "my_mix = graph { a = input ; b = input ; a |> mix(b, gain=1) |> output }";
    let module = eval_module(source, ReplMode::Loose);
    assert!(module.is_err());
    assert_eq!(
        module.unwrap_err().to_string(),
        "`mix` does not accept named parameters in Task 3"
    );
}

#[test]
fn test_havoc_pedal_mix_non_audio() {
    let source = "my_mix = graph { a = input ; b = 1 ; a |> mix(b) |> output }";
    let module = eval_module(source, ReplMode::Loose);
    assert!(module.is_err());
    assert_eq!(
        module.unwrap_err().to_string(),
        "`mix` requires every positional argument to resolve to an audio signal"
    );
}
