use orpheus_lang::{BinaryOp, Expr, Stmt, parse_module};

fn binding_expr(source: &str) -> Expr {
    let module = parse_module(source).unwrap();
    match &module.statements[0] {
        Stmt::Binding { expr, .. } => expr.clone(),
    }
}

fn binding_names(source: &str) -> Vec<String> {
    let module = parse_module(source).unwrap();
    module
        .statements
        .into_iter()
        .map(|statement| match statement {
            Stmt::Binding { name, .. } => name,
        })
        .collect()
}

fn assert_parse_error_contains(source: &str, expected_fragments: &[&str]) {
    let error = parse_module(source).unwrap_err();
    let message = error.to_string();

    assert!(
        !message.trim().is_empty(),
        "parse error should not be empty"
    );
    assert!(
        message.starts_with("parse error at line"),
        "parse error `{message}` did not start with `parse error at line`"
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
fn parses_parameterized_binding_headers() {
    let module = parse_module("swing amt pat = pat |> shift(amt)").unwrap();
    match &module.statements[0] {
        Stmt::Binding { name, expr, .. } => {
            assert_eq!(name, "swing");
            assert!(matches!(expr, Expr::Pipe { .. }));
        }
    }
}

#[test]
fn parses_chained_curried_calls() {
    let expr = binding_expr("groove = swing(0.125)(bd sn)");
    assert_eq!(
        expr,
        Expr::Call {
            callee: Box::new(Expr::Call {
                callee: Box::new(Expr::Ident("swing".to_owned())),
                args: vec![Expr::Number(0.125)],
            }),
            args: vec![Expr::Seq(vec![
                Expr::Ident("bd".to_owned()),
                Expr::Ident("sn".to_owned()),
            ])],
        }
    );
}

#[test]
fn duplicate_parameter_names_are_rejected() {
    assert_parse_error_contains("swing amt amt = amt", &["duplicate", "amt"]);
}

#[test]
fn parses_negative_numeric_arguments() {
    let expr = binding_expr("lead = pan(-1)");
    match &expr {
        Expr::Call { callee, args } => {
            assert!(matches!(callee.as_ref(), Expr::Ident(name) if name == "pan"));
            assert!(
                matches!(args.as_slice(), [Expr::Number(value)] if (*value + 1.0).abs() < f64::EPSILON)
            );
        }
        other => panic!("unexpected AST: {other:#?}"),
    }
}

#[test]
fn parses_sample_call_with_string_literal() {
    let expr = binding_expr(r#"lead = sample("vox_ah")"#);

    assert_eq!(
        expr,
        Expr::Call {
            callee: Box::new(Expr::Ident("sample".to_owned())),
            args: vec![Expr::String("vox_ah".to_owned())],
        }
    );
}

#[test]
fn named_pitch_literals_parse_inside_sequences() {
    let expr = binding_expr("melody = c4 ef4 g4 bf4");

    assert_eq!(
        expr,
        Expr::Seq(vec![
            Expr::Ident("c4".to_owned()),
            Expr::Ident("ef4".to_owned()),
            Expr::Ident("g4".to_owned()),
            Expr::Ident("bf4".to_owned()),
        ])
    );
}

#[test]
fn parses_meter_annotation_prefix_form() {
    let expr = binding_expr("bridge = meter(4, 4) stream(at(beat(0), bd), at(beat(2), sn))");

    assert_eq!(
        expr,
        Expr::Meter {
            beats: Box::new(Expr::Number(4.0)),
            unit: Box::new(Expr::Number(4.0)),
            pattern: Box::new(Expr::Stream(vec![
                Expr::At {
                    start: Box::new(Expr::Beat(Box::new(Expr::Number(0.0)))),
                    pattern: Box::new(Expr::Ident("bd".to_owned())),
                },
                Expr::At {
                    start: Box::new(Expr::Beat(Box::new(Expr::Number(2.0)))),
                    pattern: Box::new(Expr::Ident("sn".to_owned())),
                },
            ])),
        }
    );
}

#[test]
fn rejects_bindings_without_equals() {
    assert_parse_error_contains("drums bd sn", &["expected binding"]);
}

#[test]
fn rejects_unterminated_stack_groups() {
    assert_parse_error_contains("drums = stack(bd ~, ~ sn", &[")"]);
}

#[test]
fn rejects_stack_layers_with_double_commas() {
    assert_parse_error_contains("drums = stack(bd ~,, ~ sn)", &["expected"]);
}

#[test]
fn parses_multiple_top_level_bindings() {
    assert_eq!(
        binding_names("drums = bd sn\nbass = cp"),
        vec!["drums".to_owned(), "bass".to_owned()]
    );
}

#[test]
fn pedal_graph_parses_let_bound_block() {
    let source = "drivebox = graph { wet = input |> clip(model=silicon_hard) ; wet |> output }";
    let module = parse_module(source).unwrap();
    match &module.statements[0] {
        Stmt::Binding { name, expr, .. } => {
            assert_eq!(name, "drivebox");
            match expr {
                Expr::Graph { bindings, result } => {
                    assert_eq!(bindings.len(), 1);
                    assert_eq!(bindings[0].name, "wet");
                    assert!(matches!(
                        &bindings[0].expr,
                        Expr::Pipe { lhs, rhs }
                            if matches!(lhs.as_ref(), Expr::Ident(name) if name == "input")
                                && matches!(
                                    rhs.as_ref(),
                                    Expr::Call { callee, args }
                                        if matches!(callee.as_ref(), Expr::Ident(name) if name == "clip")
                                            && matches!(
                                                args.as_slice(),
                                                [Expr::Binary { lhs, op: BinaryOp::Assign, rhs }]
                                                    if matches!(lhs.as_ref(), Expr::Ident(name) if name == "model")
                                                        && matches!(rhs.as_ref(), Expr::Ident(name) if name == "silicon_hard")
                                            )
                                )
                    ));
                    assert!(matches!(
                        result.as_ref(),
                        Expr::Pipe { lhs, rhs }
                            if matches!(lhs.as_ref(), Expr::Ident(name) if name == "wet")
                                && matches!(rhs.as_ref(), Expr::Ident(name) if name == "output")
                    ));
                }
                other => panic!("unexpected AST: {other:#?}"),
            }
        }
    }
}

#[test]
fn pedal_graph_parses_binary_control_expressions() {
    let source = "drivebox = graph { dry = input ; wet = input |> clip(model=silicon_hard) ; mix(dry * 0.2 + wet * 0.8, dry) |> output }";
    let module = parse_module(source).unwrap();
    match &module.statements[0] {
        Stmt::Binding { expr, .. } => match expr {
            Expr::Graph { bindings, result } => {
                assert_eq!(bindings.len(), 2);
                assert_eq!(bindings[0].name, "dry");
                assert_eq!(bindings[1].name, "wet");
                assert!(matches!(
                    result.as_ref(),
                    Expr::Pipe { lhs, rhs }
                        if matches!(
                            lhs.as_ref(),
                            Expr::Call { callee, args }
                                if matches!(callee.as_ref(), Expr::Ident(name) if name == "mix")
                                    && matches!(
                                        args.as_slice(),
                                        [
                                            Expr::Binary {
                                                lhs,
                                                op: BinaryOp::Add,
                                                rhs
                                            },
                                            Expr::Ident(name)
                                        ]
                                            if name == "dry"
                                                && matches!(
                                                    lhs.as_ref(),
                                                    Expr::Binary {
                                                        lhs,
                                                        op: BinaryOp::Mul,
                                                        rhs
                                                    }
                                                        if matches!(lhs.as_ref(), Expr::Ident(name) if name == "dry")
                                                            && matches!(rhs.as_ref(), Expr::Number(value) if (*value - 0.2).abs() < f64::EPSILON)
                                                )
                                                && matches!(
                                                    rhs.as_ref(),
                                                    Expr::Binary {
                                                        lhs,
                                                        op: BinaryOp::Mul,
                                                        rhs
                                                    }
                                                        if matches!(lhs.as_ref(), Expr::Ident(name) if name == "wet")
                                                            && matches!(rhs.as_ref(), Expr::Number(value) if (*value - 0.8).abs() < f64::EPSILON)
                                                )
                                    )
                        )
                            && matches!(rhs.as_ref(), Expr::Ident(name) if name == "output")
                ));
            }
            other => panic!("unexpected AST: {other:#?}"),
        },
    }
}

#[test]
fn pedal_graph_rejects_binding_after_result_expression() {
    let source = "drivebox = graph { wet = input ; wet |> output ; dry = input }";
    assert_parse_error_contains(
        source,
        &["bindings must appear before the final result expression"],
    );
}

#[test]
fn pedal_graph_requires_result_expression() {
    let source = "drivebox = graph { wet = input |> clip(model=silicon_hard) }";
    assert_parse_error_contains(source, &["graph", "result expression"]);
}

#[test]
fn pedal_graph_rejects_general_assignment_expressions() {
    let source = "drivebox = input = output";
    assert!(parse_module(source).is_err());
}

#[test]
fn pipe_target_does_not_accept_a_sequence() {
    let source = "drivebox = bd |> sn cp";
    assert_parse_error_contains(source, &["expected", "EOI"]);
}
