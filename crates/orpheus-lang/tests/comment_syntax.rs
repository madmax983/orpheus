//! Integration tests for `//` line-comment syntax in `.ode` source, covering
//! the parser (comment stripping and the string-literal guard), the evaluator
//! (comments never change results), and the file loader.
use std::path::PathBuf;

use orpheus_lang::{
    Expr, ReplMode, StepOp, Stmt, Value, eval_module, load_file_strict, parse_module,
};

fn binding_expr(source: &str) -> Expr {
    let module = parse_module(source).unwrap();
    match &module.statements[0] {
        Stmt::Binding { expr, .. } => expr.clone(),
    }
}

fn binding_names(source: &str) -> Vec<String> {
    parse_module(source)
        .unwrap()
        .statements
        .into_iter()
        .map(|statement| match statement {
            Stmt::Binding { name, .. } => name,
        })
        .collect()
}

fn sample_names(value: &Value) -> Vec<String> {
    value
        .as_sample_pattern()
        .unwrap()
        .query_unit()
        .unwrap()
        .into_iter()
        .map(|event| event.value.sample().to_owned())
        .collect()
}

// --- Parser-level behavior ---------------------------------------------------

#[test]
fn full_line_comment_is_ignored() {
    let names = binding_names("// a leading comment line\ndrums = bd sn");
    assert_eq!(names, vec!["drums".to_owned()]);
}

#[test]
fn trailing_comment_after_binding_is_ignored() {
    let expr = binding_expr("drums = bd sn // the backbeat");
    assert_eq!(
        expr,
        Expr::Seq(vec![
            Expr::Ident("bd".to_owned()),
            Expr::Ident("sn".to_owned()),
        ])
    );
}

#[test]
fn comment_line_does_not_break_the_following_binding() {
    let names = binding_names("kick = bd\n// a divider comment\nsnare = sn");
    assert_eq!(names, vec!["kick".to_owned(), "snare".to_owned()]);
}

#[test]
fn empty_comment_line_is_allowed() {
    let names = binding_names("//\ndrums = bd sn\n//");
    assert_eq!(names, vec!["drums".to_owned()]);
}

#[test]
fn comment_marker_inside_string_literal_is_not_stripped() {
    let expr = binding_expr(r#"lead = sample("a//b")"#);
    assert_eq!(
        expr,
        Expr::Call {
            callee: Box::new(Expr::Ident("sample".to_owned())),
            args: vec![Expr::String("a//b".to_owned())],
        }
    );
}

#[test]
fn slow_step_operator_still_parses_with_comment_syntax() {
    let expr = binding_expr("drums = bd/2 sn");
    match &expr {
        Expr::Seq(items) => {
            assert!(matches!(
                &items[0],
                Expr::Modified { op: StepOp::Slow(factor), .. } if (*factor - 2.0).abs() < f64::EPSILON
            ));
            assert!(matches!(&items[1], Expr::Ident(name) if name == "sn"));
        }
        other => panic!("unexpected AST: {other:#?}"),
    }
}

// --- Evaluator-level behavior ------------------------------------------------

#[test]
fn comments_do_not_change_evaluation() {
    let plain = eval_module("drums = bd sn cp sn", ReplMode::Loose).unwrap();
    let commented = eval_module(
        "// a drum pattern\ndrums = bd sn cp sn // four on the floor\n// trailing note",
        ReplMode::Loose,
    )
    .unwrap();

    let plain_names = sample_names(plain.get("drums").unwrap());
    let commented_names = sample_names(commented.get("drums").unwrap());
    assert_eq!(plain_names, commented_names);
    assert_eq!(plain_names, vec!["bd", "sn", "cp", "sn"]);
}

#[test]
fn comments_between_bindings_evaluate_all_bindings() {
    let module = eval_module(
        "// intro\nkick = bd ~ bd ~\n// the snare\nsnare = ~ sn ~ sn\nsong = stack(kick, snare)",
        ReplMode::Loose,
    )
    .unwrap();

    assert!(module.contains_key("kick"));
    assert!(module.contains_key("snare"));
    assert!(module.contains_key("song"));
}

// --- Loader-level behavior ---------------------------------------------------

#[test]
fn loader_accepts_ode_files_with_comments() {
    let source = "\
// a self-documenting drum sketch
kick = bd ~ bd ~ // four on the floor
// the backbeat snare
snare = ~ sn ~ sn
song = stack(kick, snare) // the master mix
";

    let dir = tempfile::tempdir().unwrap();
    let file_path = PathBuf::from(dir.path()).join("commented.ode");
    std::fs::write(&file_path, source).unwrap();

    let module = load_file_strict(&file_path).unwrap();

    assert!(module.contains_key("kick"));
    assert!(module.contains_key("snare"));
    assert!(module.contains_key("song"));
}
