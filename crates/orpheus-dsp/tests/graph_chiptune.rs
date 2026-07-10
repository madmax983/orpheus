//! Behavior tests for the NES-authentic chiptune nodes.
//!
//! These pin the ported APU cores (`madmax983/nes` `apu.rs`): duty high-time
//! ratios and hard two-level output for `pulse_nes`, the exact 32-step / 16-level
//! staircase and ultrasonic mute for `tri_nes`, and — the key authenticity pin —
//! the bit-exact 15-bit LFSR stream and its periods (32767 long / 93 short) for
//! `noise_nes`.

#![allow(
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::float_cmp
)]

use orpheus_dsp::{Node, advance_lfsr, noise_nes, pulse_nes, tri_nes};

const SR: f32 = 48_000.0;

// ---------------------------------------------------------------------------
// pulse_nes
// ---------------------------------------------------------------------------

/// Ported `DUTY_TABLE` — the reference the tests check against.
const DUTY_TABLE: [[u8; 8]; 4] = [
    [0, 1, 0, 0, 0, 0, 0, 0],
    [0, 1, 1, 0, 0, 0, 0, 0],
    [0, 1, 1, 1, 1, 0, 0, 0],
    [1, 0, 0, 1, 1, 1, 1, 1],
];

#[test]
fn pulse_nes_channel_counts() {
    let node = pulse_nes(SR);
    assert_eq!(node.inputs(), 3);
    assert_eq!(node.outputs(), 1);
}

#[test]
fn pulse_nes_step_sequence_matches_duty_table() {
    // freq = SR / 8 makes each of the 8 samples advance the duty step by exactly
    // one, so one period's output traces DUTY_TABLE[duty] directly.
    for (duty_idx, row) in DUTY_TABLE.iter().enumerate() {
        let mut node = pulse_nes(SR);
        let freq = vec![SR / 8.0; 8];
        let duty = vec![duty_idx as f32; 8];
        let volume = vec![1.0_f32; 8]; // level 15 -> non-zero output 1.0
        let mut out = vec![0.0_f32; 8];
        node.process(&[&freq, &duty, &volume], &mut [&mut out], 8);

        let expected: Vec<f32> = row.iter().map(|&b| f32::from(b)).collect();
        assert_eq!(out, expected, "duty {duty_idx} step sequence");
    }
}

#[test]
fn pulse_nes_high_time_ratio_per_pattern() {
    // Over a whole number of periods the fraction of high samples equals the
    // count of 1-bits in the duty row / 8: 1/8, 2/8, 4/8, 6/8.
    let periods = 64;
    let frames = periods * 8;
    for (duty_idx, row) in DUTY_TABLE.iter().enumerate() {
        let mut node = pulse_nes(SR);
        let freq = vec![SR / 8.0; frames];
        let duty = vec![duty_idx as f32; frames];
        let volume = vec![1.0_f32; frames];
        let mut out = vec![0.0_f32; frames];
        node.process(&[&freq, &duty, &volume], &mut [&mut out], frames);

        let high = out.iter().filter(|&&s| s > 0.0).count();
        let ones: usize = row.iter().map(|&b| usize::from(b)).sum();
        assert_eq!(high, ones * periods, "duty {duty_idx} high-time count");
    }
}

#[test]
fn pulse_nes_output_takes_only_two_values() {
    // A hard two-level square: exactly {0, quantized-volume}, never anything in
    // between (no band-limiting).
    let frames = 4096;
    let mut node = pulse_nes(SR);
    let freq = vec![440.0_f32; frames];
    let duty = vec![2.0_f32; frames];
    let volume = vec![0.6_f32; frames];
    let mut out = vec![0.0_f32; frames];
    node.process(&[&freq, &duty, &volume], &mut [&mut out], frames);

    // 0.6 * 15 = 9.0 -> level 9 -> 9/15.
    let high = 9.0_f32 / 15.0;
    for &s in &out {
        assert!(s == 0.0 || s == high, "unexpected pulse value {s}");
    }
    assert!(out.contains(&0.0));
    assert!(out.contains(&high));
}

#[test]
fn pulse_nes_volume_quantizes_to_4_bits() {
    // A request between grid points snaps to the nearest 4-bit level.
    let frames = 64;
    let mut node = pulse_nes(SR);
    let freq = vec![SR / 8.0; frames];
    let duty = vec![2.0_f32; frames]; // 50% has a step-1 high sample
    // 0.5 * 15 = 7.5 -> rounds to 8 -> 8/15.
    let volume = vec![0.5_f32; frames];
    let mut out = vec![0.0_f32; frames];
    node.process(&[&freq, &duty, &volume], &mut [&mut out], frames);

    let expected_high = 8.0_f32 / 15.0;
    assert!(out.contains(&expected_high));
    assert!(out.iter().all(|&s| s == 0.0 || s == expected_high));
}

// ---------------------------------------------------------------------------
// tri_nes
// ---------------------------------------------------------------------------

const TRIANGLE_TABLE: [u8; 32] = [
    15, 14, 13, 12, 11, 10, 9, 8, 7, 6, 5, 4, 3, 2, 1, 0, 0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12,
    13, 14, 15,
];

#[test]
fn tri_nes_channel_counts() {
    let node = tri_nes(SR);
    assert_eq!(node.inputs(), 1);
    assert_eq!(node.outputs(), 1);
}

#[test]
fn tri_nes_sequence_matches_triangle_table() {
    // freq = SR / 32 advances one sequence step per sample.
    let mut node = tri_nes(SR);
    let freq = vec![SR / 32.0; 32];
    let mut out = vec![0.0_f32; 32];
    node.process(&[&freq], &mut [&mut out], 32);

    let expected: Vec<f32> = TRIANGLE_TABLE
        .iter()
        .map(|&l| f32::from(l) / 15.0)
        .collect();
    assert_eq!(out, expected);
}

#[test]
fn tri_nes_has_exactly_16_distinct_levels() {
    let mut node = tri_nes(SR);
    let freq = vec![SR / 32.0; 32];
    let mut out = vec![0.0_f32; 32];
    node.process(&[&freq], &mut [&mut out], 32);

    let mut levels: Vec<u32> = out.iter().map(|&s| (s * 15.0).round() as u32).collect();
    levels.sort_unstable();
    levels.dedup();
    assert_eq!(levels.len(), 16, "triangle must expose 16 distinct levels");
}

#[test]
fn tri_nes_ultrasonic_frequency_is_muted() {
    // Above ~18.6 kHz the equivalent NES timer_reload drops below 2 and the
    // channel silences (matching apu.rs). 25 kHz is well past that.
    let frames = 256;
    let mut node = tri_nes(SR);
    let freq = vec![25_000.0_f32; frames];
    let mut out = vec![0.0_f32; frames];
    node.process(&[&freq], &mut [&mut out], frames);
    assert!(
        out.iter().all(|&s| s == 0.0),
        "ultrasonic triangle must mute"
    );

    // A normal note is not muted.
    let mut node = tri_nes(SR);
    let freq = vec![440.0_f32; frames];
    let mut out = vec![0.0_f32; frames];
    node.process(&[&freq], &mut [&mut out], frames);
    assert!(out.iter().any(|&s| s > 0.0), "440 Hz triangle must sound");
}

// ---------------------------------------------------------------------------
// noise_nes — the authenticity pin
// ---------------------------------------------------------------------------

/// First 16 register states starting from the reset value `1`, long mode
/// (tap bit 1). Derived directly from the apu.rs LFSR rule and checked in here
/// as the bit-exact reference.
const LFSR_LONG_FIRST_16: [u16; 16] = [
    1, 16384, 8192, 4096, 2048, 1024, 512, 256, 128, 64, 32, 16, 8, 4, 2, 16385,
];

/// First 16 register states starting from `1`, short mode (tap bit 6).
const LFSR_SHORT_FIRST_16: [u16; 16] = [
    1, 16384, 8192, 4096, 2048, 1024, 512, 256, 128, 64, 16416, 8208, 4104, 2052, 1026, 513,
];

#[test]
fn noise_nes_channel_counts() {
    let node = noise_nes(SR);
    assert_eq!(node.inputs(), 3);
    assert_eq!(node.outputs(), 1);
}

#[test]
fn lfsr_long_stream_matches_reference() {
    let mut sr = 1_u16;
    for (i, &expected) in LFSR_LONG_FIRST_16.iter().enumerate() {
        assert_eq!(sr, expected, "long LFSR state {i}");
        sr = advance_lfsr(sr, false);
    }
}

#[test]
fn lfsr_short_stream_matches_reference() {
    let mut sr = 1_u16;
    for (i, &expected) in LFSR_SHORT_FIRST_16.iter().enumerate() {
        assert_eq!(sr, expected, "short LFSR state {i}");
        sr = advance_lfsr(sr, true);
    }
}

#[test]
fn lfsr_long_mode_period_is_32767() {
    let mut sr = advance_lfsr(1, false);
    let mut steps = 1_u32;
    while sr != 1 {
        sr = advance_lfsr(sr, false);
        steps += 1;
    }
    assert_eq!(steps, 32_767, "long-mode LFSR period");
}

#[test]
fn lfsr_short_mode_period_is_93() {
    let mut sr = advance_lfsr(1, true);
    let mut steps = 1_u32;
    while sr != 1 {
        sr = advance_lfsr(sr, true);
        steps += 1;
    }
    assert_eq!(steps, 93, "short-mode LFSR period");
}

#[test]
fn lfsr_never_reaches_zero() {
    // The 15-bit LFSR is non-zero for its whole cycle (a zero state would be a
    // fixed point). Walk a full long-mode period and assert.
    let mut sr = 1_u16;
    for _ in 0..32_767 {
        assert_ne!(sr, 0, "LFSR must never hit the zero fixed point");
        sr = advance_lfsr(sr, false);
    }
    assert_eq!(sr, 1, "LFSR returns to seed after a full period");
}

#[test]
fn noise_nes_output_stream_follows_lfsr() {
    // Driving at freq = SR advances the LFSR exactly once per sample, so the
    // node output must be volume when (register & 1) == 0 and 0 otherwise. With
    // volume 1.0 the non-zero level is 1.0.
    let frames = 200;
    let mut node = noise_nes(SR);
    let mode = vec![0.0_f32; frames];
    let freq = vec![SR; frames];
    let volume = vec![1.0_f32; frames];
    let mut out = vec![0.0_f32; frames];
    node.process(&[&mode, &freq, &volume], &mut [&mut out], frames);

    // Reference: advance first (matching the node), then read the low bit.
    let mut sr = 1_u16;
    for (i, &sample) in out.iter().enumerate() {
        sr = advance_lfsr(sr, false);
        let expected = if sr & 1 == 0 { 1.0 } else { 0.0 };
        assert_eq!(sample, expected, "noise output sample {i}");
    }
}

#[test]
fn noise_nes_output_takes_only_two_values() {
    let frames = 4096;
    let mut node = noise_nes(SR);
    let mode = vec![0.0_f32; frames];
    let freq = vec![8_000.0_f32; frames];
    let volume = vec![0.8_f32; frames]; // 0.8 * 15 = 12 -> 12/15
    let mut out = vec![0.0_f32; frames];
    node.process(&[&mode, &freq, &volume], &mut [&mut out], frames);

    let high = 12.0_f32 / 15.0;
    for &s in &out {
        assert!(s == 0.0 || s == high, "unexpected noise value {s}");
    }
}

#[test]
fn noise_nes_reset_restores_seed() {
    let frames = 128;
    let mut node = noise_nes(SR);
    let mode = vec![0.0_f32; frames];
    let freq = vec![SR; frames];
    let volume = vec![1.0_f32; frames];

    let mut out1 = vec![0.0_f32; frames];
    node.process(&[&mode, &freq, &volume], &mut [&mut out1], frames);
    node.reset();
    let mut out2 = vec![0.0_f32; frames];
    node.process(&[&mode, &freq, &volume], &mut [&mut out2], frames);

    assert_eq!(out1, out2, "reset must return to the seed state");
}
