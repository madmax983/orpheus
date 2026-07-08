//! Integration tests for the syntax parser, ensuring all language constructs translate correctly into the internal Abstract Syntax Tree.
use orpheus_lang::{BinaryOp, Expr, StepOp, Stmt, parse_module};

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

fn assert_is_ident(expr: &Expr, expected: &str) {
    match expr {
        Expr::Ident(name) => assert_eq!(name, expected),
        _ => panic!("Expected identifier {expected}, got {expr:#?}"),
    }
}

fn assert_is_number(expr: &Expr, expected: f64) {
    match expr {
        Expr::Number(value) => assert!(
            (value - expected).abs() < f64::EPSILON,
            "Expected {expected}, got {value}"
        ),
        _ => panic!("Expected number {expected}, got {expr:#?}"),
    }
}

fn assert_is_binary(expr: &Expr, expected_op: BinaryOp) -> (&Expr, &Expr) {
    match expr {
        Expr::Binary { lhs, op, rhs } => {
            assert_eq!(op, &expected_op);
            (lhs, rhs)
        }
        _ => panic!("Expected binary expression {expected_op:?}, got {expr:#?}"),
    }
}

fn assert_is_pipe(expr: &Expr) -> (&Expr, &Expr) {
    match expr {
        Expr::Pipe { lhs, rhs } => (lhs, rhs),
        _ => panic!("Expected pipe expression, got {expr:#?}"),
    }
}

fn assert_is_call<'a>(expr: &'a Expr, expected_callee: &str) -> &'a [Expr] {
    match expr {
        Expr::Call { callee, args } => {
            assert_is_ident(callee, expected_callee);
            args
        }
        _ => panic!("Expected call to {expected_callee}, got {expr:#?}"),
    }
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

                    let (lhs, rhs) = assert_is_pipe(&bindings[0].expr);
                    assert_is_ident(lhs, "input");

                    let args = assert_is_call(rhs, "clip");
                    assert_eq!(args.len(), 1);

                    let (assign_lhs, assign_rhs) = assert_is_binary(&args[0], BinaryOp::Assign);
                    assert_is_ident(assign_lhs, "model");
                    assert_is_ident(assign_rhs, "silicon_hard");

                    let (res_lhs, res_rhs) = assert_is_pipe(result);
                    assert_is_ident(res_lhs, "wet");
                    assert_is_ident(res_rhs, "output");
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

                let (lhs, rhs) = assert_is_pipe(result);
                assert_is_ident(rhs, "output");

                let args = assert_is_call(lhs, "mix");
                assert_eq!(args.len(), 2);

                let (add_lhs, add_rhs) = assert_is_binary(&args[0], BinaryOp::Add);
                assert_is_ident(&args[1], "dry");

                let (mul1_lhs, mul1_rhs) = assert_is_binary(add_lhs, BinaryOp::Mul);
                assert_is_ident(mul1_lhs, "dry");
                assert_is_number(mul1_rhs, 0.2);

                let (mul2_lhs, mul2_rhs) = assert_is_binary(add_rhs, BinaryOp::Mul);
                assert_is_ident(mul2_lhs, "wet");
                assert_is_number(mul2_rhs, 0.8);
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

#[test]
fn parses_angle_brackets_as_alternation() {
    let expr = binding_expr("drums = bd <sn cp>");
    let Expr::Seq(items) = expr else {
        panic!("expected sequence expression");
    };
    assert_eq!(items[0], Expr::Ident("bd".to_owned()));
    assert_eq!(
        items[1],
        Expr::Alternation(vec![
            Expr::Ident("sn".to_owned()),
            Expr::Ident("cp".to_owned()),
        ])
    );
}

#[test]
fn parses_standalone_alternation() {
    let expr = binding_expr("drums = <bd sn>");
    assert_eq!(
        expr,
        Expr::Alternation(vec![
            Expr::Ident("bd".to_owned()),
            Expr::Ident("sn".to_owned()),
        ])
    );
}

#[test]
fn parses_groups_inside_alternations() {
    let expr = binding_expr("drums = <(bd sn) cp>");
    assert_eq!(
        expr,
        Expr::Alternation(vec![
            Expr::Group(vec![
                Expr::Ident("bd".to_owned()),
                Expr::Ident("sn".to_owned()),
            ]),
            Expr::Ident("cp".to_owned()),
        ])
    );
}

#[test]
fn rejects_empty_alternations() {
    assert_parse_error_contains("drums = <>", &[]);
}

// --- mini-notation step operators: `*`, `/`, `!`, `?`, `{...}` polymeter ---

/// Like [`assert_parse_error_contains`] but without requiring positional
/// information; step-operator validation happens after AST construction where
/// spans are no longer available.
fn assert_parse_fails_mentioning(source: &str, expected_fragments: &[&str]) {
    let error = parse_module(source).unwrap_err();
    let message = error.to_string();
    for fragment in expected_fragments {
        assert!(
            message.contains(fragment),
            "parse error `{message}` did not mention required fragment `{fragment}`"
        );
    }
}

fn modified(inner: Expr, op: StepOp) -> Expr {
    Expr::Modified {
        inner: Box::new(inner),
        op,
    }
}

fn ident(name: &str) -> Expr {
    Expr::Ident(name.to_owned())
}

#[test]
fn parses_tight_star_as_repetition_inside_sequences() {
    let expr = binding_expr("drums = bd*2 sn");
    assert_eq!(
        expr,
        Expr::Seq(vec![modified(ident("bd"), StepOp::Fast(2.0)), ident("sn"),])
    );
}

#[test]
fn parses_tight_slash_as_slow() {
    let expr = binding_expr("drums = bd/2 sn");
    assert_eq!(
        expr,
        Expr::Seq(vec![modified(ident("bd"), StepOp::Slow(2)), ident("sn")])
    );
}

#[test]
fn parses_step_operators_on_groups_and_alternations() {
    assert_eq!(
        binding_expr("drums = (bd sn)*2"),
        modified(
            Expr::Group(vec![ident("bd"), ident("sn")]),
            StepOp::Fast(2.0),
        )
    );
    assert_eq!(
        binding_expr("drums = <bd sn>*2"),
        modified(
            Expr::Alternation(vec![ident("bd"), ident("sn")]),
            StepOp::Fast(2.0),
        )
    );
}

#[test]
fn parses_replication_as_separate_steps() {
    assert_eq!(
        binding_expr("drums = bd!3 sn"),
        Expr::Seq(vec![ident("bd"), ident("bd"), ident("bd"), ident("sn")])
    );
}

#[test]
fn parses_replication_inside_alternations() {
    assert_eq!(
        binding_expr("drums = <bd!2 sn>"),
        Expr::Alternation(vec![ident("bd"), ident("bd"), ident("sn")])
    );
}

#[test]
fn replication_distributes_later_modifiers_over_each_copy() {
    assert_eq!(
        binding_expr("drums = bd!2?"),
        Expr::Seq(vec![
            modified(ident("bd"), StepOp::Degrade(0.5)),
            modified(ident("bd"), StepOp::Degrade(0.5)),
        ])
    );
}

#[test]
fn parses_degrade_with_and_without_probability_suffix() {
    assert_eq!(
        binding_expr("drums = bd? sn"),
        Expr::Seq(vec![
            modified(ident("bd"), StepOp::Degrade(0.5)),
            ident("sn"),
        ])
    );
    assert_eq!(
        binding_expr("drums = bd?0.3 sn"),
        Expr::Seq(vec![
            modified(ident("bd"), StepOp::Degrade(0.3)),
            ident("sn"),
        ])
    );
}

#[test]
fn parses_polymeter_with_explicit_step_count() {
    assert_eq!(
        binding_expr("drums = {bd sn, hh hh hh}%4"),
        Expr::Polymeter {
            groups: vec![
                vec![ident("bd"), ident("sn")],
                vec![ident("hh"), ident("hh"), ident("hh")],
            ],
            steps: Some(4),
        }
    );
}

#[test]
fn parses_polymeter_without_step_suffix() {
    assert_eq!(
        binding_expr("drums = {bd sn, hh cp sn}"),
        Expr::Polymeter {
            groups: vec![
                vec![ident("bd"), ident("sn")],
                vec![ident("hh"), ident("cp"), ident("sn")],
            ],
            steps: None,
        }
    );
}

#[test]
fn spaced_star_still_parses_as_binary_multiplication() {
    let expr = binding_expr("gainy = bd * 2");
    let (lhs, rhs) = assert_is_binary(&expr, BinaryOp::Mul);
    assert_is_ident(lhs, "bd");
    assert_is_number(rhs, 2.0);
}

#[test]
fn tight_star_inside_pedal_graphs_stays_multiplication() {
    let source = "drivebox = graph { dry = input ; mix(dry*0.2, dry) |> output }";
    let module = parse_module(source).unwrap();
    match &module.statements[0] {
        Stmt::Binding { expr, .. } => match expr {
            Expr::Graph { result, .. } => {
                let (lhs, _) = assert_is_pipe(result);
                let args = assert_is_call(lhs, "mix");
                let (mul_lhs, mul_rhs) = assert_is_binary(&args[0], BinaryOp::Mul);
                assert_is_ident(mul_lhs, "dry");
                assert_is_number(mul_rhs, 0.2);
            }
            other => panic!("unexpected AST: {other:#?}"),
        },
    }
}

#[test]
fn rejects_out_of_bounds_step_operator_factors() {
    assert_parse_fails_mentioning("drums = bd*0 sn", &["`*`", "1", "1024"]);
    assert_parse_fails_mentioning("drums = bd*1025 sn", &["`*`", "1", "1024"]);
    assert_parse_fails_mentioning("drums = bd*1.5 sn", &["`*`", "integer"]);
    assert_parse_fails_mentioning("drums = bd/0 sn", &["`/`", "1", "1024"]);
    assert_parse_fails_mentioning("drums = bd!0 sn", &["`!`", "1", "1024"]);
    assert_parse_fails_mentioning("drums = bd?1.5 sn", &["`?`", "probability"]);
    assert_parse_fails_mentioning("drums = {bd sn}%0", &["polymeter", "1", "1024"]);
}

#[test]
fn rejects_malformed_polymeters() {
    assert!(parse_module("drums = {bd sn").is_err());
    assert!(parse_module("drums = {}").is_err());
    assert!(parse_module("drums = {bd sn,}").is_err());
}

#[test]
fn rejects_oversized_replication() {
    // Replication expands into real sequence steps, so it shares the flat
    // sequence length budget.
    assert_parse_fails_mentioning("drums = bd!1024", &["maximum AST depth exceeded"]);
}
