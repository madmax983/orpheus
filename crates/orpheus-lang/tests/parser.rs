use orpheus_lang::{Expr, Stmt, parse_module};

fn binding_expr(source: &str) -> Expr {
    let module = parse_module(source).unwrap();
    match &module.statements[0] {
        Stmt::Binding { expr, .. } => expr.clone(),
    }
}

fn assert_parse_error_contains(source: &str, expected_fragments: &[&str]) {
    let error = parse_module(source).unwrap_err();
    let message = error.to_string();

    assert!(
        !message.trim().is_empty(),
        "parse error should not be empty"
    );
    assert!(
        message.starts_with("parse error:"),
        "parse error `{message}` did not start with `parse error:`"
    );
    for fragment in expected_fragments {
        assert!(
            message.contains(fragment),
            "parse error `{message}` did not mention required fragment `{fragment}`"
        );
    }
}

#[test]
fn parses_juxtaposition_as_sequence() {
    let module = parse_module("drums = bd sn cp").unwrap();
    match &module.statements[0] {
        Stmt::Binding { expr, .. } => assert!(matches!(expr, Expr::Seq(_))),
    }
}

#[test]
fn parses_pipe_as_left_associative() {
    let expr = binding_expr("drums = bd sn |> fast(2) |> rev");
    assert_eq!(
        expr,
        Expr::Pipe {
            lhs: Box::new(Expr::Pipe {
                lhs: Box::new(Expr::Seq(vec![
                    Expr::Ident("bd".to_owned()),
                    Expr::Ident("sn".to_owned()),
                ])),
                rhs: Box::new(Expr::Call {
                    callee: Box::new(Expr::Ident("fast".to_owned())),
                    args: vec![Expr::Number(2.0)],
                }),
            }),
            rhs: Box::new(Expr::Ident("rev".to_owned())),
        }
    );
}

#[test]
fn parses_grouping_inside_sequences() {
    let expr = binding_expr("drums = bd (sn cp)");
    match &expr {
        Expr::Seq(items) => {
            assert!(matches!(items[0], Expr::Ident(_)));
            assert!(matches!(items[1], Expr::Group(_)));
        }
        other => panic!("unexpected AST: {other:#?}"),
    }
}

#[test]
fn parses_stack_layers_with_rests() {
    let expr = binding_expr("drums = stack(bd ~, ~ sn)");
    assert_eq!(
        expr,
        Expr::Stack(vec![
            Expr::Seq(vec![Expr::Ident("bd".to_owned()), Expr::Rest]),
            Expr::Seq(vec![Expr::Rest, Expr::Ident("sn".to_owned())]),
        ])
    );
}

#[test]
fn parses_function_calls_with_numeric_arguments() {
    let expr = binding_expr("swing = fast(2)");
    match &expr {
        Expr::Call { callee, args } => {
            assert!(matches!(callee.as_ref(), Expr::Ident(name) if name == "fast"));
            assert!(
                matches!(args.as_slice(), [Expr::Number(value)] if (*value - 2.0).abs() < f64::EPSILON)
            );
        }
        other => panic!("unexpected AST: {other:#?}"),
    }
}

#[test]
fn rejects_bindings_without_equals() {
    assert_parse_error_contains("drums bd sn", &["="]);
}

#[test]
fn rejects_unterminated_stack_groups() {
    assert_parse_error_contains("drums = stack(bd ~, ~ sn", &[")", "expected"]);
}

#[test]
fn rejects_stack_layers_with_double_commas() {
    assert_parse_error_contains("drums = stack(bd ~,, ~ sn)", &["expected"]);
}

#[test]
fn rejects_a_second_top_level_binding() {
    assert_parse_error_contains(
        "drums = bd sn\nbass = cp",
        &["single top-level binding", "bass = cp"],
    );
}
