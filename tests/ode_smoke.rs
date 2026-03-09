use std::path::PathBuf;

use orpheus_lang::load_file_strict;

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join(name)
}

#[test]
fn ode_smoke_compiles_song_file() {
    let module = load_file_strict(fixture("song.ode")).unwrap();

    assert!(module.contains_key("song"));
}
