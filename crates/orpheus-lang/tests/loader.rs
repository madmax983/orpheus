use std::path::PathBuf;

use orpheus_lang::load_file_strict;

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("tests")
        .join("fixtures")
        .join(name)
}

fn docs_example(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("docs")
        .join("examples")
        .join(name)
}

#[test]
fn loader_resolves_use_imports() {
    let module = load_file_strict(fixture("song.ode")).unwrap();

    assert!(module.contains_key("song"));
}

#[test]
fn loader_reports_missing_names_as_errors() {
    let error = load_file_strict(fixture("missing_name.ode")).unwrap_err();

    assert!(error.to_string().contains("unresolved name"));
}

#[test]
fn loader_accepts_multi_binding_phase5_example() {
    let module = load_file_strict(docs_example("phase5_escape_hatch.ode")).unwrap();

    assert!(module.contains_key("verse"));
    assert!(module.contains_key("bridge"));
    assert!(module.contains_key("song"));
}

#[test]
fn loader_accepts_analog_showcase_example() {
    let module = load_file_strict(docs_example("analog_showcase.ode")).unwrap();

    assert!(module.contains_key("drums"));
    assert!(module.contains_key("bass"));
    assert!(module.contains_key("pad"));
    assert!(module.contains_key("lead"));
    assert!(module.contains_key("song"));
}

#[test]
fn loader_reports_malformed_import_lines_as_errors() {
    let cases = vec![
        (
            "use file.ode\" (names)",
            "import path must start with a quoted filename",
        ),
        (
            "use \"file.ode (names)",
            "import path is missing a closing quote",
        ),
        ("use \"file.ode\" names", "import list must use parentheses"),
        (
            "use \"file.ode\" ()",
            "import list must name at least one binding",
        ),
    ];

    let dir = tempfile::tempdir().unwrap();
    let file_path = dir.path().join("test.ode");

    for (input, expected_error) in cases {
        std::fs::write(&file_path, input).unwrap();
        let error = load_file_strict(&file_path).unwrap_err();
        assert!(
            error.to_string().contains(expected_error),
            "Expected error for input '{input}' to contain '{expected_error}', but got '{error}'"
        );
    }
}
