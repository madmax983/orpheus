//! Behavior tests for the Sega Genesis SN76489 PSG nodes (`psg_tone`,
//! `psg_noise`).
//!
//! These pin the ported genesoxide `psg.rs` cores and Orpheus's single-voice
//! headroom normalization:
//!   * the **tone** channel is a hard bipolar 50% square that tracks `freq_hz`,
//!     gated by `gate`, at the 2 dB logarithmic `level` grid;
//!   * the **period-0 quirk** (constant-high output) surfaces at ultrasonic
//!     requests;
//!   * the **noise** channel's white vs periodic modes differ in character, and
//!     periodic mode is pitched;
//!   * the **2 dB/step logarithmic** attenuation grid;
//!   * per-node **headroom/peak** safety — a single voice never clips.

#![allow(
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss
)]

use orpheus_dsp::{Node, psg_noise, psg_tone};

const SR: f32 = 48_000.0;

fn render_tone(node: &mut dyn Node, gate: f32, freq: f32, level: f32, frames: usize) -> Vec<f32> {
    let g = vec![gate; frames];
    let f = vec![freq; frames];
    let l = vec![level; frames];
    let mut out = vec![0.0; frames];
    node.process(&[&g, &f, &l], &mut [&mut out], frames);
    out
}

fn render_noise(
    node: &mut dyn Node,
    gate: f32,
    mode: f32,
    freq: f32,
    level: f32,
    frames: usize,
) -> Vec<f32> {
    let g = vec![gate; frames];
    let m = vec![mode; frames];
    let f = vec![freq; frames];
    let l = vec![level; frames];
    let mut out = vec![0.0; frames];
    node.process(&[&g, &m, &f, &l], &mut [&mut out], frames);
    out
}

fn peak(s: &[f32]) -> f32 {
    s.iter().map(|x| x.abs()).fold(0.0, f32::max)
}
fn rms(s: &[f32]) -> f32 {
    (s.iter().map(|x| x * x).sum::<f32>() / s.len() as f32).sqrt()
}
fn positive_crossings(s: &[f32]) -> u32 {
    let mut c = 0;
    for w in s.windows(2) {
        if w[0] < 0.0 && w[1] >= 0.0 {
            c += 1;
        }
    }
    c
}

// ---------------------------------------------------------------------------
// Contract
// ---------------------------------------------------------------------------

#[test]
fn channel_counts() {
    let tone = psg_tone(SR);
    assert_eq!(tone.inputs(), 3);
    assert_eq!(tone.outputs(), 1);
    let noise = psg_noise(SR);
    assert_eq!(noise.inputs(), 4);
    assert_eq!(noise.outputs(), 1);
}

#[test]
fn tone_silent_until_gated() {
    let mut node = psg_tone(SR);
    let s = render_tone(&mut node, 0.0, 440.0, 1.0, 4000);
    assert!(
        peak(&s) < 1e-6,
        "ungated tone should be silent, peak={}",
        peak(&s)
    );
}

#[test]
fn noise_silent_until_gated() {
    let mut node = psg_noise(SR);
    let s = render_noise(&mut node, 0.0, 1.0, 8000.0, 1.0, 4000);
    assert!(
        peak(&s) < 1e-6,
        "ungated noise should be silent, peak={}",
        peak(&s)
    );
}

// ---------------------------------------------------------------------------
// Tone: pitch tracking + squareness
// ---------------------------------------------------------------------------

#[test]
fn tone_tracks_frequency() {
    // Roughly one positive crossing per waveform period; higher freq → more.
    let mut low = psg_tone(SR);
    let mut high = psg_tone(SR);
    let frames = 24_000; // 0.5 s
    let lo = render_tone(&mut low, 1.0, 220.0, 1.0, frames);
    let hi = render_tone(&mut high, 1.0, 880.0, 1.0, frames);

    let lo_c = positive_crossings(&lo) as f32;
    let hi_c = positive_crossings(&hi) as f32;
    // ~0.5 s at 220 Hz ≈ 110 crossings; at 880 Hz ≈ 440.
    assert!((lo_c - 110.0).abs() < 12.0, "220 Hz crossings {lo_c}");
    assert!((hi_c - 440.0).abs() < 40.0, "880 Hz crossings {hi_c}");
    // Two octaves up ≈ 4× the crossings.
    let ratio = hi_c / lo_c;
    assert!((ratio - 4.0).abs() < 0.5, "octave ratio {ratio}");
}

#[test]
fn tone_is_bipolar_square() {
    // A full-volume gated square swings between +0.9 and -0.9 only.
    let mut node = psg_tone(SR);
    let s = render_tone(&mut node, 1.0, 440.0, 1.0, 8000);
    let hi = s.iter().copied().fold(f32::MIN, f32::max);
    let lo = s.iter().copied().fold(f32::MAX, f32::min);
    assert!((hi - 0.9).abs() < 1e-3, "positive rail {hi}");
    assert!((lo + 0.9).abs() < 1e-3, "negative rail {lo}");
    // Every sample is one of the two rails (hard square, no ramp).
    assert!(
        s.iter().all(|x| (x.abs() - 0.9).abs() < 1e-3),
        "square must be strictly two-level"
    );
}

#[test]
fn tone_period_zero_quirk_is_constant_high() {
    // An ultrasonic request maps to period 0 → constant-high output (+0.9),
    // never toggling (genesoxide period-0 quirk).
    let mut node = psg_tone(SR);
    let s = render_tone(&mut node, 1.0, 200_000.0, 1.0, 4000);
    assert!(
        s.iter().all(|&x| (x - 0.9).abs() < 1e-3),
        "period-0 output should be a constant positive DC level"
    );
}

// ---------------------------------------------------------------------------
// Tone: logarithmic level grid
// ---------------------------------------------------------------------------

#[test]
fn level_is_logarithmic_2db_grid() {
    // level = 1.0 → index 0 (1.0); one step down → index 1 (~0.794 → -2 dB).
    let mut full = psg_tone(SR);
    let mut down = psg_tone(SR);
    let a = render_tone(&mut full, 1.0, 440.0, 1.0, 8000);
    let b = render_tone(&mut down, 1.0, 440.0, 1.0 - 1.0 / 15.0, 8000);
    let ratio = peak(&b) / peak(&a);
    assert!(
        (ratio - 0.794_328).abs() < 1e-3,
        "one attenuation step should be ~-2 dB, ratio={ratio}"
    );
    // level = 0 → silence.
    let mut off = psg_tone(SR);
    let z = render_tone(&mut off, 1.0, 440.0, 0.0, 4000);
    assert!(peak(&z) < 1e-6, "level 0 should be silent");
}

// ---------------------------------------------------------------------------
// Noise
// ---------------------------------------------------------------------------

#[test]
fn noise_is_audible_and_bounded() {
    let mut node = psg_noise(SR);
    let s = render_noise(&mut node, 1.0, 1.0, 12_000.0, 1.0, 16_000);
    assert!(
        rms(&s) > 0.05,
        "white noise should have energy, rms={}",
        rms(&s)
    );
    assert!(
        peak(&s) <= 0.9 + 1e-4,
        "noise must stay bounded, peak={}",
        peak(&s)
    );
}

#[test]
fn white_and_periodic_noise_differ() {
    // Periodic noise (mode < 0.5) is a period-16 pitched buzz; white (mode >=
    // 0.5) is aperiodic. Over a long buffer they must produce different signals.
    let mut white = psg_noise(SR);
    let mut periodic = psg_noise(SR);
    let frames = 16_000;
    let w = render_noise(&mut white, 1.0, 1.0, 8000.0, 1.0, frames);
    let p = render_noise(&mut periodic, 1.0, 0.0, 8000.0, 1.0, frames);

    let sumdiff: f32 = w.iter().zip(&p).map(|(x, y)| (x - y).abs()).sum();
    assert!(
        sumdiff > 1.0,
        "white and periodic streams must differ, sumdiff={sumdiff}"
    );

    // Periodic noise repeats (period-16 LFSR): its crossing count is far lower
    // and more regular than white noise's for the same shift rate.
    let w_c = positive_crossings(&w);
    let p_c = positive_crossings(&p);
    assert!(
        p_c < w_c,
        "periodic noise should cross zero less often than white: periodic={p_c} white={w_c}"
    );
}

// ---------------------------------------------------------------------------
// Headroom (single voice must never clip)
// ---------------------------------------------------------------------------

#[test]
fn single_voice_never_clips() {
    // Loudest tone and loudest white noise: both must stay clear of the rails.
    let mut tone = psg_tone(SR);
    let t = render_tone(&mut tone, 1.0, 110.0, 1.0, 20_000);
    assert!(peak(&t) <= 0.99, "tone peak {}", peak(&t));
    assert!(
        t.iter().all(|x| x.abs() < 1.0),
        "tone must have no rail samples"
    );

    let mut noise = psg_noise(SR);
    let n = render_noise(&mut noise, 1.0, 1.0, 16_000.0, 1.0, 20_000);
    assert!(peak(&n) <= 0.99, "noise peak {}", peak(&n));
    assert!(
        n.iter().all(|x| x.abs() < 1.0),
        "noise must have no rail samples"
    );
}

// ---------------------------------------------------------------------------
// Reset
// ---------------------------------------------------------------------------

#[test]
fn reset_restores_initial_state() {
    let mut tone = psg_tone(SR);
    let _ = render_tone(&mut tone, 1.0, 440.0, 1.0, 4000);
    tone.reset();
    let s = render_tone(&mut tone, 0.0, 440.0, 1.0, 2000);
    assert!(peak(&s) < 1e-6, "reset tone should be silent");

    let mut noise = psg_noise(SR);
    let _ = render_noise(&mut noise, 1.0, 1.0, 8000.0, 1.0, 4000);
    noise.reset();
    let n = render_noise(&mut noise, 0.0, 1.0, 8000.0, 1.0, 2000);
    assert!(peak(&n) < 1e-6, "reset noise should be silent");
}
