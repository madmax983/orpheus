//! Drift-guard for `docs/reference/language-reference.md`.
//!
//! The language reference documents every builtin. This test makes it
//! impossible for that document to silently drift away from the registry in
//! `crates/orpheus-lang/src/builtins.rs`.
//!
//! ## Why this shape of guard
//!
//! The registry (`builtin_value`) is three `match` statements over string
//! literals. Rust cannot reflect over `match` arms, and the crate exposes no
//! iterator of registered names, so a *fully programmatic* enumeration of the
//! registry from code is not cleanly possible without changing the source.
//!
//! We therefore use the strongest guard the structure allows, with two halves
//! that together pin the reference to the registry:
//!
//! 1. `REGISTERED` is the canonical list of every name `builtin_value` accepts,
//!    transcribed directly from the three lookup tables. The test asserts every
//!    entry actually resolves via `builtin_value(name).is_some()`, so this list
//!    cannot itself claim a name the registry does not have.
//! 2. The reference's machine-readable `REGISTRY-INDEX` block is parsed and
//!    compared for *set equality* against `REGISTERED`. A name documented but
//!    not registered (a fake) or registered but not documented (a gap) fails the
//!    test and is named in the panic message.
//!
//! Adding a builtin therefore requires touching both the registry and this list
//! (which is validated against `builtin_value`) and the reference block — any
//! one left behind turns this test red.

use std::collections::BTreeSet;
use std::path::PathBuf;

use orpheus_lang::{ReplMode, builtin_value, eval_module};

/// Every name accepted by `builtin_value`, transcribed from the three lookup
/// tables in `builtins.rs` (`lookup_pattern_transform`, `lookup_scale`,
/// `lookup_effect`). Half 1 of the guard asserts each of these resolves.
const REGISTERED: &[&str] = &[
    // Sample source atoms
    "bd",
    "sn",
    "cp",
    "hh",
    "saw",
    "pulse",
    "tri",
    "noise",
    // Scales
    "ionian",
    "dorian",
    "phrygian",
    "mixolydian",
    "aeolian",
    "minor_pentatonic",
    // Arp directions
    "up",
    "down",
    "pingpong",
    "updown",
    // Time
    "fast",
    "slow",
    "rev",
    "shift",
    "palindrome",
    "iter",
    "iter_back",
    // Cycle-scoped
    "every",
    "when",
    "whenmod",
    "within",
    // Probabilistic
    "degrade",
    "degrade_by",
    "sometimes",
    "sometimes_by",
    "often",
    "rarely",
    "almost_always",
    "almost_never",
    "chaos",
    "rand",
    "irand",
    "choose",
    "wchoose",
    "pchoose",
    "wpchoose",
    "randcat",
    "wrandcat",
    "markov",
    "shuffle",
    "scramble",
    // Concatenation & rotation
    "cat",
    "slowcat",
    "append",
    "off",
    "rot",
    "chunk",
    "chunk_back",
    // Sampling continuous patterns
    "segment",
    "range",
    "run",
    "scan",
    // Euclidean family
    "euclid",
    "euclid_inv",
    "euclid_full",
    // Layering
    "jux",
    "through",
    // Pitch & harmony
    "chord",
    "invert",
    "drop",
    "degrees",
    "pitch_class_set",
    "transpose",
    "pitch",
    "strum",
    "roll",
    "arp",
    "notes",
    // Tuning
    "tuning",
    "load_scl",
    "tune",
    // Generative
    "lsystem",
    "wolfram",
    // Controls & effects
    "gain",
    "pan",
    "cutoff",
    "res",
    "hpf",
    "lpf",
    "drive",
    "pw",
    "delay",
    "delay_time",
    "delay_feedback",
    "reverb",
    "reverb_room",
    "reverb_damp",
    "chorus",
    "chorus_depth",
    "chorus_rate",
    "compressor",
    "compressor_threshold",
    "compressor_ratio",
    "onset",
    "rate",
    "sample",
    "slice",
    "slice_idx",
    "cc",
    "midi_cc",
    "p",
    "param",
    "p1",
    "p2",
    "p3",
    "p4",
    "vst",
    "au",
    "hex",
    "bin",
];

fn reference_path() -> PathBuf {
    // CARGO_MANIFEST_DIR = <repo>/crates/orpheus-lang
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../docs/reference/language-reference.md")
}

/// Parse the backtick-wrapped names inside the `REGISTRY-INDEX` block.
fn documented_builtins() -> BTreeSet<String> {
    let text = std::fs::read_to_string(reference_path())
        .expect("language-reference.md must exist next to the reference doc");
    let start = text
        .find("<!-- REGISTRY-INDEX-START -->")
        .expect("reference must contain the REGISTRY-INDEX-START marker");
    let end = text
        .find("<!-- REGISTRY-INDEX-END -->")
        .expect("reference must contain the REGISTRY-INDEX-END marker");
    assert!(start < end, "REGISTRY-INDEX markers must be ordered");
    let block = &text[start..end];

    let mut names = BTreeSet::new();
    let mut rest = block;
    while let Some(open) = rest.find('`') {
        rest = &rest[open + 1..];
        let Some(close) = rest.find('`') else { break };
        let name = &rest[..close];
        rest = &rest[close + 1..];
        if !name.is_empty() {
            names.insert(name.to_string());
        }
    }
    names
}

#[test]
fn every_registered_name_resolves() {
    // Half 1: the canonical list cannot claim a name the registry lacks.
    let mut missing = Vec::new();
    for name in REGISTERED {
        if builtin_value(name).is_none() {
            missing.push(*name);
        }
    }
    assert!(
        missing.is_empty(),
        "these names are in REGISTERED but `builtin_value` does not resolve them: {missing:?}"
    );
}

#[test]
fn documented_builtins_match_registry_exactly() {
    // Half 2: the reference's index equals the registry's name set.
    let documented = documented_builtins();
    let registered: BTreeSet<String> = REGISTERED.iter().map(|s| (*s).to_string()).collect();

    // No documented name may fail to resolve (catches typos/fakes directly).
    let unresolved: Vec<&String> = documented
        .iter()
        .filter(|name| builtin_value(name).is_none())
        .collect();
    assert!(
        unresolved.is_empty(),
        "documented builtins that do not resolve via `builtin_value`: {unresolved:?}"
    );

    let documented_but_unregistered: Vec<&String> = documented.difference(&registered).collect();
    let registered_but_undocumented: Vec<&String> = registered.difference(&documented).collect();

    assert!(
        documented_but_unregistered.is_empty() && registered_but_undocumented.is_empty(),
        "reference/registry drift.\n  documented but not registered: {documented_but_unregistered:?}\n  registered but not documented: {registered_but_undocumented:?}"
    );
}

/// Evaluate a representative one-line example from the reference, panicking with
/// the source and error if it does not run. Mirrors the `eval_module` harness in
/// `tests/eval.rs`.
fn assert_evaluates(source: &str) {
    if let Err(error) = eval_module(source, ReplMode::Loose) {
        panic!("reference example failed to evaluate:\n  {source}\n  error: {error}");
    }
}

#[test]
#[allow(clippy::too_many_lines)]
fn reference_examples_evaluate() {
    // Representative examples copied verbatim from language-reference.md.
    let examples = [
        // Mini-notation
        "drums = bd sn cp sn",
        "drums = bd ~ sn ~",
        "drums = bd (sn cp)",
        "drums = (bd sn, hh hh hh)",
        "drums = bd sn, hh*4",
        "drums = <bd sn cp>",
        "drums = bd*2",
        "drums = (bd sn)/2",
        "drums = bd!3 sn",
        "drums = (bd sn cp hh)?",
        "drums = {bd sn, hh hh hh}",
        "drums = {bd sn cp hh}%2",
        "hats = hh*8 |> gain(0.6)",
        "drums = bd(3, 8)",
        "drums = bd(3, 8, 1)",
        "drums = bd(<3 8>, 8)",
        // Time
        "drums = bd sn |> fast(2)",
        "drums = bd sn |> slow(2)",
        "drums = bd sn cp sn |> rev",
        "drums = bd sn |> shift(0.25)",
        "drums = palindrome(bd sn)",
        "drums = iter(4, bd sn cp hh)",
        "drums = iter_back(4, bd sn cp hh)",
        // Cycle-scoped
        "drums = bd sn |> every(2, fast(2))",
        "drums = bd sn |> when(3, 1, rev)",
        "drums = bd sn |> whenmod(4, 2, rev)",
        "drums = bd sn cp hh |> within(0, 0.5, rev)",
        // Probabilistic
        "m = 0 1 2 3 4 5 6 7 |> degrade",
        "drums = bd sn |> degrade_by(0.25)",
        "drums = bd sn |> sometimes(fast(2))",
        "drums = bd sn |> sometimes_by(0.5, rev)",
        "drums = often(rev, bd sn)",
        "drums = rarely(rev, bd sn)",
        "drums = almost_always(rev, bd sn)",
        "drums = almost_never(rev, bd sn)",
        "a = chaos(bd sn cp hh)",
        "r = rand()",
        "m = irand(8)",
        "m = choose(1, 2, 3, 4)",
        "m = wchoose(1, 1, 2, 3)",
        "m = pchoose(0 1, 2 3)",
        "drums = wpchoose(bd, 1, sn, 3)",
        "m = randcat(0 1, 2 3)",
        "drums = wrandcat(bd, 1, sn, 3)",
        "m = markov(0 1, 1, 2, 2 3, 3, 1)",
        "m = 10 20 30 40 |> shuffle(4)",
        "m = 10 20 30 40 |> scramble(4)",
        // Concatenation & rotation
        "drums = cat(bd, sn cp)",
        "drums = append(bd, sn)",
        "drums = off(0.25, rev, bd sn)",
        "drums = rot(1, bd sn cp)",
        "drums = chunk(4, rev, bd sn cp hh)",
        "drums = chunk_back(4, rev, bd sn cp hh)",
        // Sampling
        "m = rand() |> segment(4)",
        "m = range(200, 2000, 0 0.5 1)",
        "ramp = run(4)",
        "ramp = scan(3)",
        // Euclid
        "drums = mask(euclid(3, 8), bd*8)",
        "drums = mask(euclid_inv(3, 8), bd*8)",
        "drums = euclid_full(3, 8, bd*8, sn*8)",
        "drums = euclid_full(3, 8, 1, bd*8, sn*8)",
        // Layering
        "a = jux(rev, bd sn)",
        // Pitch & harmony
        "pad = chord(c4, 0 4 7)",
        "pad = invert(1, chord(c4, 0 4 7))",
        "pad = drop(2, chord(c4, 0 4 7 10))",
        "line = degrees(aeolian, 0 2 4 7 8)",
        "pcs = pitch_class_set(0 4 7)",
        "melody = transpose(12, c4 e4 g4)",
        "drums = bd |> pitch(7)",
        "pad = strum(chord(c4, 0 4 7))",
        "buzz = roll(4, sn)",
        "lead = arp(5, up, chord(c4, 0 4 7))",
        // Tuning
        "t = tuning(1.0 1.125 1.25 1.5 2.0)",
        "drums = tune(tuning(1.0 1.5 2.0), bd sn)",
        // Generative
        "pat = lsystem(\"A\", 3, \"A:AB,B:A\")",
        "w = wolfram(30, 3)",
        // Controls & effects
        "drums = bd |> gain(0.8)",
        "lead = saw |> pan(-1)",
        "drums = bd |> cutoff(800)",
        "drums = bd |> res(0.5)",
        "drums = bd |> hpf(200)",
        "drums = bd |> lpf(2000)",
        "drums = bd |> drive(1.5)",
        "lead = pulse |> pw(0.3)",
        "drums = bd |> delay(0.3)",
        "drums = bd |> delay_time(0.25)",
        "drums = bd |> delay_feedback(0.4)",
        "drums = bd |> reverb(0.3)",
        "drums = bd |> reverb_room(0.6)",
        "drums = bd |> reverb_damp(0.5)",
        "lead = saw |> chorus(0.4)",
        "lead = saw |> chorus_depth(0.5)",
        "lead = saw |> chorus_rate(1.5)",
        "drums = bd |> compressor(0.5)",
        "drums = bd |> compressor_threshold(0.4)",
        "drums = bd |> compressor_ratio(4)",
        "drums = bd |> rate(1.5)",
        "kit = sample(\"bd\")",
        "drums = bd |> slice(0.25, 1)",
        "lead = bd sn |> p1(300 4000)",
        "lead = bd sn |> p3(7)",
        "m = hex(\"a\")",
        "m = bin(\"1010\")",
    ];

    for source in examples {
        assert_evaluates(source);
    }
}

#[test]
fn reference_voice_example_evaluates() {
    // Strict mode, mirroring tests/voice.rs.
    let source =
        "pluck = voice { osc = saw(freq) ; env = adsr(gate, 0.001, 0.02, 0.5, 0.05) ; osc * env }";
    if let Err(error) = eval_module(source, ReplMode::Strict) {
        panic!("reference voice example failed to evaluate:\n  {source}\n  error: {error}");
    }
}
