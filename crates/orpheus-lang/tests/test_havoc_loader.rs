use orpheus_lang::load_file_strict;

#[test]
fn loader_reports_missing_file_as_error() {
    let result = load_file_strict("this_file_does_not_exist.ode");
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("file not found"));
}
