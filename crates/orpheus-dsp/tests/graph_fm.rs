//! Behavior tests for the Sega Genesis YM2612 FM node (`fm_genesis`).
//!
//! These pin the ported genesoxide `ym2612.rs` core and Orpheus's two additions
//! (the ladder crossover and the single-voice headroom normalization):
//!   * the load-bearing **algorithm routing table** — which operators are
//!     carriers vs modulators for each of the 8 topologies;
//!   * **envelope** rise/decay/release and key-scaling;
//!   * **op-1 feedback** raising harmonic content while staying bounded;
//!   * the **ladder** effect being an audible, asymmetric-but-bounded change;
//!   * **MUL** frequency ratios;
//!   * per-preset **headroom/peak** safety (this doubles as the genesoxide
//!     clipping diagnostic — a single voice's ported math must never clip).

#![allow(
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss
)]

use orpheus_dsp::{FmOp, FmPatch, Node, bell, brass, drum, ebass, epiano, fm_genesis, lead};

const SR: f32 = 48_000.0;

/// Render `frames` samples with constant control inputs.
fn render(
    node: &mut dyn Node,
    gate: f32,
    freq: f32,
    bright: f32,
    fb: f32,
    frames: usize,
) -> Vec<f32> {
    let g = vec![gate; frames];
    let f = vec![freq; frames];
    let b = vec![bright; frames];
    let fbv = vec![fb; frames];
    let mut out = vec![0.0; frames];
    node.process(&[&g, &f, &b, &fbv], &mut [&mut out], frames);
    out
}

fn peak(s: &[f32]) -> f32 {
    s.iter().map(|x| x.abs()).fold(0.0, f32::max)
}
fn mean(s: &[f32]) -> f32 {
    s.iter().sum::<f32>() / s.len() as f32
}
fn rms(s: &[f32]) -> f32 {
    (s.iter().map(|x| x * x).sum::<f32>() / s.len() as f32).sqrt()
}
/// Mean absolute first difference, normalized by peak — a cheap harmonic-content
/// proxy (a pure sine has low total variation; adding partials raises it).
fn total_variation_ratio(s: &[f32]) -> f32 {
    let tv: f32 = s.windows(2).map(|w| (w[1] - w[0]).abs()).sum::<f32>() / s.len() as f32;
    tv / peak(s).max(1e-6)
}
/// Count positive-going zero crossings (a rough fundamental-frequency measure).
fn positive_crossings(s: &[f32]) -> u32 {
    let mut c = 0;
    for w in s.windows(2) {
        if w[0] < 0.0 && w[1] >= 0.0 {
            c += 1;
        }
    }
    c
}

/// A patch where exactly one operator is loud (`TL = 0`) and the rest are
/// silent (`TL = 127`); channel output is audible iff the loud operator is a
/// carrier under `algorithm`. Ladder off so the measurement is clean.
fn single_op_loud(algorithm: u8, loud: usize) -> FmPatch {
    let ops = std::array::from_fn(|i| FmOp {
        mul: 1,
        detune: 0,
        total_level: if i == loud { 0 } else { 127 },
        ar: 31,
        d1r: 0,
        sl: 0,
        d2r: 0,
        rr: 15,
        rate_scale: 0,
        ssg_eg: 0,
        am_on: false,
    });
    FmPatch {
        algorithm,
        feedback: 0,
        ops,
        lfo_rate: 0,
        pms: 0,
        ams: 0,
        ladder: false,
    }
}

// ---------------------------------------------------------------------------
// Basic contract
// ---------------------------------------------------------------------------

#[test]
fn fm_genesis_channel_counts() {
    let node = fm_genesis(SR, lead());
    assert_eq!(node.inputs(), 4);
    assert_eq!(node.outputs(), 1);
}

#[test]
fn silent_until_gated() {
    let mut node = fm_genesis(SR, lead());
    // Gate held low: envelopes stay in Release at max attenuation → silence.
    let s = render(&mut node, 0.0, 220.0, 1.0, 0.5, 4000);
    assert!(
        peak(&s) < 1e-4,
        "ungated voice should be silent, peak={}",
        peak(&s)
    );
}

// ---------------------------------------------------------------------------
// Algorithm routing — the load-bearing topology pin
// ---------------------------------------------------------------------------

/// Expected carrier operator indices per algorithm (design doc §2.3).
const fn expected_carriers(algorithm: u8) -> &'static [usize] {
    match algorithm {
        0..=3 => &[3],
        4 => &[1, 3],
        5 | 6 => &[1, 2, 3],
        _ => &[0, 1, 2, 3],
    }
}

#[test]
fn algorithm_routing_topology_is_pinned() {
    for algo in 0..8u8 {
        let carriers = expected_carriers(algo);
        for op in 0..4usize {
            let mut node = fm_genesis(SR, single_op_loud(algo, op));
            let _ = render(&mut node, 1.0, 440.0, 1.0, 0.0, 2000); // settle attack
            let s = render(&mut node, 1.0, 440.0, 1.0, 0.0, 2000);
            let p = peak(&s);
            if carriers.contains(&op) {
                assert!(
                    p > 0.1,
                    "algo {algo}: op {op} is a carrier and should be audible, peak={p}"
                );
            } else {
                assert!(
                    p < 1e-3,
                    "algo {algo}: op {op} is a modulator and should not reach output, peak={p}"
                );
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Envelope
// ---------------------------------------------------------------------------

/// algo 7, only op0 as a carrier, with a moderate attack and a decay toward a
/// sustain knee — used to observe the envelope shape.
const fn env_patch(ar: u8, d1r: u8, sl: u8, rate_scale: u8) -> FmPatch {
    let mut ops = [FmOp::silent(); 4];
    ops[0] = FmOp {
        mul: 1,
        detune: 0,
        total_level: 0,
        ar,
        d1r,
        sl,
        d2r: 0,
        rr: 10,
        rate_scale,
        ssg_eg: 0,
        am_on: false,
    };
    FmPatch {
        algorithm: 7,
        feedback: 0,
        ops,
        lfo_rate: 0,
        pms: 0,
        ams: 0,
        ladder: false,
    }
}

#[test]
fn envelope_attacks_up_then_decays_then_releases_to_zero() {
    let mut node = fm_genesis(SR, env_patch(20, 14, 6, 0));
    let held = render(&mut node, 1.0, 440.0, 1.0, 0.0, 12_000);

    let attack_win = rms(&held[0..1000]);
    let peak_win = rms(&held[1000..2000]);
    let decayed_win = rms(&held[9000..10_000]);

    assert!(
        peak_win > attack_win,
        "attack should rise: {attack_win} -> {peak_win}"
    );
    assert!(
        decayed_win < peak_win,
        "envelope should decay after the peak: {peak_win} -> {decayed_win}"
    );

    // Key-off → release to silence.
    let released = render(&mut node, 0.0, 440.0, 1.0, 0.0, 12_000);
    let tail = rms(&released[11_000..]);
    assert!(
        tail < 1e-3,
        "release should decay to silence, tail rms={tail}"
    );
}

#[test]
fn key_scaling_makes_higher_notes_decay_faster() {
    // rate_scale = 3 → higher key codes speed every envelope stage up.
    let patch = env_patch(31, 12, 8, 3);
    let mut low = fm_genesis(SR, patch);
    let mut high = fm_genesis(SR, patch);

    let low_s = render(&mut low, 1.0, 110.0, 1.0, 0.0, 8000);
    let high_s = render(&mut high, 1.0, 1760.0, 1.0, 0.0, 8000); // 4 octaves up

    let low_late = rms(&low_s[5000..6000]);
    let high_late = rms(&high_s[5000..6000]);
    assert!(
        high_late < low_late,
        "key-scaled high note should have decayed further: high={high_late} low={low_late}"
    );
}

// ---------------------------------------------------------------------------
// Feedback
// ---------------------------------------------------------------------------

#[test]
fn feedback_raises_harmonic_content_and_stays_bounded() {
    // algo 7 with only op0 (the feedback operator) audible.
    let patch = single_op_loud(7, 0);

    let mut clean = fm_genesis(SR, patch);
    let _ = render(&mut clean, 1.0, 440.0, 1.0, 0.0, 2000);
    let clean_s = render(&mut clean, 1.0, 440.0, 1.0, 0.0, 4000); // fb = 0

    let mut fed = fm_genesis(SR, patch);
    let _ = render(&mut fed, 1.0, 440.0, 1.0, 1.0, 2000);
    let fed_s = render(&mut fed, 1.0, 440.0, 1.0, 1.0, 4000); // fb -> 7

    let clean_tv = total_variation_ratio(&clean_s);
    let fed_tv = total_variation_ratio(&fed_s);
    assert!(
        fed_tv > clean_tv * 3.0,
        "feedback should add harmonics: clean_tv={clean_tv} fed_tv={fed_tv}"
    );

    // Feedback must not run away.
    assert!(
        peak(&fed_s) < 1.0,
        "feedback output must stay bounded, peak={}",
        peak(&fed_s)
    );
}

// ---------------------------------------------------------------------------
// Ladder effect
// ---------------------------------------------------------------------------

#[test]
fn ladder_off_is_symmetric_on_is_biased_and_different() {
    // A pure single-carrier sine (op0, algo7, no feedback) is symmetric, so the
    // clean DC offset is ~0; the ladder adds the crossover positive bias.
    let clean_patch = single_op_loud(7, 0);
    let on = FmPatch {
        ladder: true,
        ..clean_patch
    };
    let off = FmPatch {
        ladder: false,
        ..clean_patch
    };

    let mut non = fm_genesis(SR, on);
    let mut noff = fm_genesis(SR, off);
    let _ = render(&mut non, 1.0, 220.0, 1.0, 0.0, 2000);
    let _ = render(&mut noff, 1.0, 220.0, 1.0, 0.0, 2000);
    let a = render(&mut non, 1.0, 220.0, 1.0, 0.0, 8000);
    let b = render(&mut noff, 1.0, 220.0, 1.0, 0.0, 8000);

    // Off: symmetric sine → near-zero DC.
    assert!(
        mean(&b).abs() < 5e-3,
        "ladder-off should be symmetric, mean={}",
        mean(&b)
    );
    // On: crossover pushes the mean positive relative to off.
    assert!(
        mean(&a) > mean(&b),
        "ladder should add a positive crossover bias: on={} off={}",
        mean(&a),
        mean(&b)
    );
    // The two renders must differ audibly.
    let sumdiff: f32 = a.iter().zip(&b).map(|(x, y)| (x - y).abs()).sum();
    assert!(
        sumdiff > 1.0,
        "ladder on/off must be measurably different, sumdiff={sumdiff}"
    );
    // But the ladder must not push a single voice into clipping.
    assert!(
        peak(&a) < 0.99,
        "ladder output must stay in bounds, peak={}",
        peak(&a)
    );
}

// ---------------------------------------------------------------------------
// Operator frequency multiple
// ---------------------------------------------------------------------------

#[test]
fn mul_zero_is_half_frequency() {
    fn carrier_with_mul(mul: u8) -> FmPatch {
        let mut ops = [FmOp::silent(); 4];
        ops[0] = FmOp {
            mul,
            detune: 0,
            total_level: 0,
            ar: 31,
            d1r: 0,
            sl: 0,
            d2r: 0,
            rr: 15,
            rate_scale: 0,
            ssg_eg: 0,
            am_on: false,
        };
        FmPatch {
            algorithm: 7,
            feedback: 0,
            ops,
            lfo_rate: 0,
            pms: 0,
            ams: 0,
            ladder: false,
        }
    }

    let mut n1 = fm_genesis(SR, carrier_with_mul(1));
    let mut n0 = fm_genesis(SR, carrier_with_mul(0));
    let _ = render(&mut n1, 1.0, 440.0, 1.0, 0.0, 2000);
    let _ = render(&mut n0, 1.0, 440.0, 1.0, 0.0, 2000);
    let s1 = render(&mut n1, 1.0, 440.0, 1.0, 0.0, 24_000);
    let s0 = render(&mut n0, 1.0, 440.0, 1.0, 0.0, 24_000);

    let c1 = positive_crossings(&s1) as f32;
    let c0 = positive_crossings(&s0) as f32;
    // MUL=0 runs the operator an octave down (×0.5).
    let ratio = c0 / c1;
    assert!(
        (0.4..=0.6).contains(&ratio),
        "MUL=0 should be ~half the pitch of MUL=1: crossings {c0} vs {c1} (ratio {ratio})"
    );
}

// ---------------------------------------------------------------------------
// bright macro
// ---------------------------------------------------------------------------

#[test]
fn bright_macro_scales_modulator_brightness() {
    // algo 0 serial chain: op0->op1->op2->op3(carrier). Raising bright lowers
    // the modulators' TL (louder modulators → deeper FM → more harmonics).
    let mut ops = [FmOp {
        mul: 1,
        detune: 0,
        total_level: 40,
        ar: 31,
        d1r: 0,
        sl: 0,
        d2r: 0,
        rr: 15,
        rate_scale: 0,
        ssg_eg: 0,
        am_on: false,
    }; 4];
    ops[3].total_level = 0; // carrier loud
    let patch = FmPatch {
        algorithm: 0,
        feedback: 0,
        ops,
        lfo_rate: 0,
        pms: 0,
        ams: 0,
        ladder: false,
    };

    let mut dark = fm_genesis(SR, patch);
    let mut brightn = fm_genesis(SR, patch);
    let _ = render(&mut dark, 1.0, 330.0, 0.3, 0.0, 2000);
    let _ = render(&mut brightn, 1.0, 330.0, 1.8, 0.0, 2000);
    let dark_s = render(&mut dark, 1.0, 330.0, 0.3, 0.0, 6000);
    let bright_s = render(&mut brightn, 1.0, 330.0, 1.8, 0.0, 6000);

    assert!(
        total_variation_ratio(&bright_s) > total_variation_ratio(&dark_s),
        "higher bright should add harmonic content: dark={} bright={}",
        total_variation_ratio(&dark_s),
        total_variation_ratio(&bright_s)
    );
}

// ---------------------------------------------------------------------------
// Presets + headroom (the clipping diagnostic)
// ---------------------------------------------------------------------------

#[test]
fn presets_are_audible_and_never_clip() {
    for (name, patch) in [
        ("epiano", epiano()),
        ("ebass", ebass()),
        ("brass", brass()),
        ("lead", lead()),
        ("bell", bell()),
        ("drum", drum()),
    ] {
        let mut node = fm_genesis(SR, patch);
        let _ = render(&mut node, 1.0, 220.0, 1.0, 0.5, 2000);
        let s = render(&mut node, 1.0, 220.0, 1.0, 0.5, 20_000);
        let p = peak(&s);
        let r = rms(&s);
        assert!(p > 0.02, "{name} should be audible, peak={p}");
        // Headroom: a single voice must stay clear of the ±1.0 rails, and no
        // sample may sit exactly on a rail (a hard-clip signature).
        assert!(p <= 0.99, "{name} single voice must not clip, peak={p}");
        assert!(
            s.iter().all(|x| x.abs() < 1.0),
            "{name} must have no hard-clipped rail samples"
        );
        assert!(r > 0.0, "{name} should have non-zero energy");
    }
}

#[test]
fn worst_case_loud_voice_stays_within_headroom() {
    // All four operators loud carriers (algo 7) with maximum feedback — the
    // loudest a single voice can get. The ported ±256 DAC clamp + single-voice
    // normalization must keep this within bounds (design §3.4 clipping verdict).
    let ops = [FmOp {
        mul: 1,
        detune: 0,
        total_level: 0,
        ar: 31,
        d1r: 0,
        sl: 0,
        d2r: 0,
        rr: 15,
        rate_scale: 0,
        ssg_eg: 0,
        am_on: false,
    }; 4];
    let patch = FmPatch {
        algorithm: 7,
        feedback: 7,
        ops,
        lfo_rate: 0,
        pms: 0,
        ams: 0,
        ladder: true,
    };
    let mut node = fm_genesis(SR, patch);
    let _ = render(&mut node, 1.0, 110.0, 1.0, 1.0, 2000);
    let s = render(&mut node, 1.0, 110.0, 1.0, 1.0, 16_000);
    let p = peak(&s);
    assert!(p <= 0.99, "worst-case single voice must not clip, peak={p}");
    assert!(
        s.iter().all(|x| x.abs() < 1.0),
        "no hard-clipped rail samples allowed"
    );
}

// ---------------------------------------------------------------------------
// Reset
// ---------------------------------------------------------------------------

#[test]
fn reset_restores_initial_silence() {
    let mut node = fm_genesis(SR, lead());
    let _ = render(&mut node, 1.0, 440.0, 1.0, 0.5, 4000);
    node.reset();
    // After reset with the gate low, the voice is silent again.
    let s = render(&mut node, 0.0, 440.0, 1.0, 0.5, 2000);
    assert!(
        peak(&s) < 1e-4,
        "reset voice should be silent, peak={}",
        peak(&s)
    );
}
