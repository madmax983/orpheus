//! Integration tests for parsing and evaluating Scala (.scl) microtonal tuning files to ensure accurate pitch translations.
use orpheus_lang::{parse_scala_file, parse_scala_source};

const EPS: f64 = 1e-9;

fn assert_close(actual: f64, expected: f64) {
    assert!(
        (actual - expected).abs() < EPS,
        "expected {expected}, got {actual} (delta {})",
        (actual - expected).abs()
    );
}

#[test]
fn parses_just_intonation_ratios_from_fixture() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/tuning/just_intonation.scl");
    let tuning = parse_scala_file(&path).unwrap();

    assert_eq!(tuning.name(), "just_intonation");
    assert_eq!(tuning.ratios().len(), 12);
    assert_close(tuning.ratios()[0], 1.0);
    assert_close(tuning.ratios()[7], 1.5);
    assert_close(tuning.period(), 2.0);
}

#[test]
fn parses_cents_lines_into_ratios() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/tuning/cents_example.scl");
    let tuning = parse_scala_file(&path).unwrap();

    assert_eq!(tuning.ratios().len(), 3);
    assert_close(tuning.ratios()[0], 1.0);
    assert_close(tuning.ratios()[1], (100.0_f64 / 1200.0).exp2());
    assert_close(tuning.ratios()[2], (701.9550_f64 / 1200.0).exp2());
    assert_close(tuning.period(), 2.0);
}

#[test]
fn accepts_integer_entries_as_ratios() {
    // Integer `2` in the period slot means 2/1 (octave); `5/4` is a plain ratio.
    let src = "simple\n\
 2\n\
!\n\
 5/4\n\
 2\n\
";
    let tuning = parse_scala_source(src, "simple").unwrap();
    assert_close(tuning.ratios()[1], 1.25);
    assert_close(tuning.period(), 2.0);
}

#[test]
fn rejects_count_mismatch() {
    let src = "bad\n\
 3\n\
!\n\
 9/8\n\
 2/1\n\
";
    let err = parse_scala_source(src, "bad").unwrap_err();
    assert!(
        err.to_string().to_lowercase().contains("count")
            || err.to_string().to_lowercase().contains("expected 3"),
        "error should mention count mismatch, got: {err}"
    );
}

#[test]
fn ignores_bang_comments_and_blank_lines() {
    let src = "\
! leading comment\n\
\n\
! another\n\
mixed\n\
! note-count comes next\n\
 2\n\
!\n\
 3/2\n\
 2/1\n\
";
    let tuning = parse_scala_source(src, "mixed").unwrap();
    assert_eq!(tuning.ratios().len(), 2);
    assert_close(tuning.ratios()[1], 1.5);
    assert_close(tuning.period(), 2.0);
}

#[test]
fn rejects_non_octave_period_in_phase_one() {
    let src = "tritave\n\
 2\n\
!\n\
 5/3\n\
 3/1\n\
";
    let err = parse_scala_source(src, "tritave").unwrap_err();
    let msg = err.to_string().to_lowercase();
    assert!(
        msg.contains("period") || msg.contains("octave") || msg.contains("2/1"),
        "error should mention period/octave constraint, got: {err}"
    );
}
