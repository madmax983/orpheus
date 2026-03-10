use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use orpheus_lang::{
    ReplMode, eval_module, render_sample_pattern_to_file, render_sample_pattern_to_wav,
};

fn temp_wav_path() -> PathBuf {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!("orpheus-render-smoke-{timestamp}.wav"))
}

#[test]
fn render_smoke_exports_song_wav() {
    let module = eval_module(
        "song = seq_sections(section(meter(4, 4, stream(at(beat(0), bd), at(beat(1), sn), at(beat(2), cp), at(beat(3), sn))), 1), section(meter(4, 4, stream(at(beat(0), hh), at(beat(2), hh))), 1))",
        ReplMode::Loose,
    )
    .unwrap();
    let song = module.get("song").unwrap().as_sample_pattern().unwrap();
    let path = temp_wav_path();

    render_sample_pattern_to_wav(song, &path, 2).unwrap();

    let bytes = fs::read(&path).unwrap();
    assert!(bytes.starts_with(b"RIFF"));
    assert!(bytes.windows(4).any(|window| window == b"WAVE"));
    assert!(bytes.len() > 44);
    assert!(bytes[44..].iter().any(|byte| *byte != 0));

    let _ = fs::remove_file(path);
}

#[test]
fn render_smoke_exports_song_flac() {
    let module = eval_module(
        "song = seq_sections(section(meter(4, 4) stream(at(beat(0), bd), at(beat(1), sn), at(beat(2), cp), at(beat(3), sn)), 1), section(meter(4, 4) stream(at(beat(0), hh), at(beat(2), hh)), 1))",
        ReplMode::Loose,
    )
    .unwrap();
    let song = module.get("song").unwrap().as_sample_pattern().unwrap();
    let path = temp_wav_path().with_extension("flac");

    render_sample_pattern_to_file(song, &path, 2).unwrap();

    let bytes = fs::read(&path).unwrap();
    assert!(bytes.starts_with(b"fLaC"));
    assert!(bytes.len() > 4);

    let _ = fs::remove_file(path);
}
