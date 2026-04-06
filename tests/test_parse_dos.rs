use orpheus_lang::ReplMode;
use orpheus_lang::eval_module;
#[test]
fn test_dos_nested_parens() {
    let mut payload = String::new();
    payload.push_str("foo = ");
    for _ in 0..150 {
        payload.push('(');
    }
    payload.push('1');
    for _ in 0..150 {
        payload.push(')');
    }
    let res = eval_module(&payload, ReplMode::Loose);
    match res {
        Ok(_) => panic!("Expected parse error due to max AST depth, but succeeded"),
        Err(e) => {
            let msg = format!("{e:?}");
            assert!(
                msg.contains("maximum AST depth exceeded"),
                "Expected max AST depth error, got: {msg}"
            );
        }
    }
}
#[test]
fn test_dos_chained_pipes() {
    let mut payload = String::new();
    payload.push_str("foo = 1 ");
    for _ in 0..150 {
        payload.push_str("|> 1 ");
    }
    let res = eval_module(&payload, ReplMode::Loose);
    match res {
        Ok(_) => panic!("Expected parse error due to max AST depth, but succeeded"),
        Err(e) => {
            let msg = format!("{e:?}");
            assert!(
                msg.contains("maximum AST depth exceeded"),
                "Expected max AST depth error, got: {msg}"
            );
        }
    }
}
#[test]
fn test_dos_chained_calls() {
    let mut payload = String::new();
    payload.push_str("foo = f");
    for _ in 0..150 {
        payload.push_str("()");
    }
    let res = eval_module(&payload, ReplMode::Loose);
    match res {
        Ok(_) => panic!("Expected parse error due to max AST depth, but succeeded"),
        Err(e) => {
            let msg = format!("{e:?}");
            assert!(
                msg.contains("maximum AST depth exceeded"),
                "Expected max AST depth error, got: {msg}"
            );
        }
    }
}
#[test]
fn test_dos_meter_annotations() {
    let mut payload = String::new();
    payload.push_str("foo = ");
    for _ in 0..150 {
        payload.push_str("meter(1, 4) ");
    }
    payload.push('1');
    let res = eval_module(&payload, ReplMode::Loose);
    match res {
        Ok(_) => panic!("Expected parse error due to max AST depth, but succeeded"),
        Err(e) => {
            let msg = format!("{e:?}");
            assert!(
                msg.contains("maximum AST depth exceeded"),
                "Expected max AST depth error, got: {msg}"
            );
        }
    }
}
