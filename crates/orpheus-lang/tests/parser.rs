use orpheus_lang::{Expr, Stmt, parse_module};

fn binding_expr(source: &str) -> Expr {
    let module = parse_module(source).unwrap();
    match &module.statements[0] {
        Stmt::Binding { expr, .. } => expr.clone(),
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
    let module = parse_module("drums = bd sn |> fast(2) |> rev").unwrap();
    let rendered = format!("{:#?}", module.statements[0]);
    assert!(rendered.contains("Pipe"));
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
    match &expr {
        Expr::Stack(layers) => {
            assert_eq!(layers.len(), 2);
            assert!(format!("{:#?}", layers[0]).contains("Rest"));
            assert!(format!("{:#?}", layers[1]).contains("Rest"));
        }
        other => panic!("unexpected AST: {other:#?}"),
    }
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
