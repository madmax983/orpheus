//! Integration test guarding the official Orpheus reference song
//! (`docs/examples/reference_song.ode`).
//!
//! The song exercises a broad slice of the language surface (custom `voice`
//! instruments with pragmas and per-note parameter automation, mini-notation,
//! Euclidean masks, polymeter, the transform and randomness families,
//! `segment`/`range` control automation, and section sequencing). Loading it
//! through the same strict loader path as `loader.rs` proves every top-level
//! binding still parses and evaluates, so the reference song can never rot.

use std::path::PathBuf;

use orpheus_lang::load_file_strict;

fn reference_song() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("docs")
        .join("examples")
        .join("reference_song.ode")
}

#[test]
fn reference_song_loads_and_evaluates_every_binding() {
    let module = load_file_strict(reference_song())
        .expect("reference song should load and evaluate cleanly");

    // Every top-level binding declared in the file must be present.
    let expected = [
        // Custom instruments.
        "acid",
        "pad",
        "pluck",
        // Drum / percussion bed.
        "kick",
        "kick_fill",
        "hats",
        "hats_busy",
        "open_hat",
        "clap",
        "snare",
        "perc",
        // Bass material and filter automation.
        "bass_deg",
        "filter_env",
        "acidline",
        "bass_deep",
        "bass_open",
        "bass_evolve",
        "bass_drive",
        "bass_solo",
        // Pad harmony.
        "pad_chord",
        "pad_lift",
        // Lead / arp material.
        "arp_deg",
        "plucks",
        "lead",
        "lead_a",
        "lead_b",
        "lead_wander",
        "lead_choose",
        "lead_evolve",
        // Sections and the master arrangement.
        "intro",
        "build",
        "drop",
        "peak",
        "melodic",
        "breakdown",
        "outro",
        "song",
    ];

    for name in expected {
        assert!(
            module.contains_key(name),
            "reference song is missing expected binding `{name}`"
        );
    }

    // The conventional entry point must be the section-sequenced arrangement.
    assert!(module.contains_key("song"));
}
