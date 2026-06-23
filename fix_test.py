import sys

def replace_in_file(filepath, old_str, new_str):
    with open(filepath, 'r') as f:
        content = f.read()
    if old_str not in content:
        print(f"Warning: could not find old_str in {filepath}")
    content = content.replace(old_str, new_str)
    with open(filepath, 'w') as f:
        f.write(content)

replace_in_file('crates/orpheus-lang/tests/havoc_rem_euclid.rs',
'''#[test]
fn test_havoc_degrees_empty() {
    let source = "notes = degrees(\\"[1]\\", \\"\\")";
    let module = eval_module(source, ReplMode::Loose);
    assert!(module.is_ok());
    let bindings = module.unwrap();
    let pat = bindings.get("notes").unwrap().as_number_pattern().unwrap();
    let res = pat.try_query_unit();
    assert!(res.is_err());
}''',
'''#[test]
fn test_havoc_degrees_empty() {
    let source = "notes = degrees(\\"[1]\\", pcs(\\"\\"))";
    let module = eval_module(source, ReplMode::Loose);
    assert!(module.is_err());
}''')
