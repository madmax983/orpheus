//! Smoke test for the non-interactive `orpheus render <file> --master <out.wav>`
//! CLI subcommand.
//!
//! This exercises the exact headless code path the binary's `render_master`
//! helper drives — a stub-engine [`ReplSession`], `open_file`, then the
//! `:export master` offline render — without starting cpal/audio. It confirms a
//! tiny program renders to a non-empty, non-silent WAV.

use std::fs;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use orpheus_dsp::EngineHandle;
use orpheus_lang::ReplSession;

static UNIQUE_TEMP_ID: AtomicU64 = AtomicU64::new(0);

fn unique_suffix() -> String {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let counter = UNIQUE_TEMP_ID.fetch_add(1, Ordering::Relaxed);
    format!("{timestamp}-{counter}")
}

#[test]
fn cli_render_master_writes_non_silent_wav() {
    let suffix = unique_suffix();
    let ode_path = std::env::temp_dir().join(format!("orpheus-cli-render-{suffix}.ode"));
    let wav_path = std::env::temp_dir().join(format!("orpheus-cli-render-{suffix}.wav"));

    // A minimal drum bed that produces at least one active track on export.
    fs::write(
        &ode_path,
        "song = seq_sections(section(meter(4, 4, stream(at(beat(0), bd), at(beat(1), sn), at(beat(2), cp), at(beat(3), sn))), 1))\n",
    )
    .unwrap();

    // Same path the `render` subcommand takes: stub engine, open file, export.
    let mut session = ReplSession::with_engine(EngineHandle::stub());
    session.open_file(&ode_path).unwrap();
    let message = session
        .eval_line(&format!(":export master {} {}", wav_path.display(), 2))
        .unwrap();
    assert!(
        message.contains("exported master"),
        "unexpected export message: {message}"
    );

    let bytes = fs::read(&wav_path).unwrap();
    assert!(bytes.starts_with(b"RIFF"), "output is not a RIFF/WAV file");
    assert!(bytes.windows(4).any(|window| window == b"WAVE"));
    assert!(bytes.len() > 44, "WAV has no sample payload");
    assert!(
        bytes[44..].iter().any(|byte| *byte != 0),
        "rendered master is silent"
    );

    let _ = fs::remove_file(&ode_path);
    let _ = fs::remove_file(&wav_path);
}
