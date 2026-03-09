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
