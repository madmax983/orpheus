//! Tests for the RBJ-cookbook biquad node (`biquad`).
//!
//! `biquad` implements the Audio EQ Cookbook (Robert Bristow-Johnson)
//! second-order sections in transposed direct form II. Inputs are
//! (audio, freq\_hz, q) — plus gain\_db for the peaking mode — and
//! coefficients are recomputed once per block from the block-start values of
//! the parameter signals.

#![allow(clippy::cast_precision_loss, clippy::cast_possible_truncation)]

use std::f32::consts::TAU;

use orpheus_dsp::{BiquadMode, Node, biquad};

const SR: f32 = 48_000.0;

/// A unit-amplitude sine at `freq_hz`.
fn sine_wave(freq_hz: f32, frames: usize) -> Vec<f32> {
    (0..frames)
        .map(|t| (TAU * freq_hz * t as f32 / SR).sin())
        .collect()
}

/// A deterministic, broadband test signal (same recipe as the fdelay tests).
fn wiggle(frames: usize) -> Vec<f32> {
    (0..frames)
        .map(|t| ((t * 37 % 101) as f32 - 50.0) / 50.0)
        .collect()
}

/// Drives a 3-input biquad over per-sample freq/Q signals in `block`-sized
/// chunks.
fn render3(node: &mut dyn Node, audio: &[f32], freq: &[f32], q: &[f32], block: usize) -> Vec<f32> {
    assert_eq!(node.inputs(), 3);
    assert_eq!(node.outputs(), 1);
    let frames = audio.len();
    let mut out = vec![0.0_f32; frames];
    let mut start = 0;
    while start < frames {
        let end = (start + block).min(frames);
        let inputs: [&[f32]; 3] = [&audio[start..end], &freq[start..end], &q[start..end]];
        let mut outputs: [&mut [f32]; 1] = [&mut out[start..end]];
        node.process(&inputs, &mut outputs, end - start);
        start = end;
    }
    out
}

/// Drives a 4-input (peaking) biquad in `block`-sized chunks.
fn render4(
    node: &mut dyn Node,
    audio: &[f32],
    freq: &[f32],
    q: &[f32],
    gain_db: &[f32],
    block: usize,
) -> Vec<f32> {
    assert_eq!(node.inputs(), 4);
    assert_eq!(node.outputs(), 1);
    let frames = audio.len();
    let mut out = vec![0.0_f32; frames];
    let mut start = 0;
    while start < frames {
        let end = (start + block).min(frames);
        let inputs: [&[f32]; 4] = [
            &audio[start..end],
            &freq[start..end],
            &q[start..end],
            &gain_db[start..end],
        ];
        let mut outputs: [&mut [f32]; 1] = [&mut out[start..end]];
        node.process(&inputs, &mut outputs, end - start);
        start = end;
    }
    out
}

/// Renders a 3-input mode with constant parameters.
fn render_mode(mode: BiquadMode, audio: &[f32], freq_hz: f32, q: f32) -> Vec<f32> {
    let mut node = biquad(SR, mode);
    let freq = vec![freq_hz; audio.len()];
    let quality = vec![q; audio.len()];
    render3(&mut node, audio, &freq, &quality, 512)
}

/// Renders the peaking mode with constant parameters.
fn render_peaking(audio: &[f32], freq_hz: f32, q: f32, gain: f32) -> Vec<f32> {
    let mut node = biquad(SR, BiquadMode::Peaking);
    let freq = vec![freq_hz; audio.len()];
    let quality = vec![q; audio.len()];
    let gain_db = vec![gain; audio.len()];
    render4(&mut node, audio, &freq, &quality, &gain_db, 512)
}

/// Steady-state gain in dB: output RMS over input RMS, measured over the
/// second half of the signals so filter transients have settled.
fn gain_db(input: &[f32], output: &[f32]) -> f64 {
    let start = input.len() / 2;
    let rms = |signal: &[f32]| {
        (signal
            .iter()
            .map(|&x| f64::from(x) * f64::from(x))
            .sum::<f64>()
            / signal.len() as f64)
            .sqrt()
    };
    20.0 * (rms(&output[start..]) / rms(&input[start..])).log10()
}

#[test]
fn biquad_channel_counts_per_mode() {
    for mode in [
        BiquadMode::Lowpass,
        BiquadMode::Highpass,
        BiquadMode::Bandpass,
        BiquadMode::Notch,
    ] {
        let node = biquad(SR, mode);
        assert_eq!(node.inputs(), 3, "{mode:?} takes (audio, freq_hz, q)");
        assert_eq!(node.outputs(), 1);
    }
    let node = biquad(SR, BiquadMode::Peaking);
    assert_eq!(
        node.inputs(),
        4,
        "Peaking takes (audio, freq_hz, q, gain_db)"
    );
    assert_eq!(node.outputs(), 1);
}

#[test]
fn biquad_lowpass_passes_below_cutoff_and_attenuates_above() {
    let audio = sine_wave(100.0, 48_000);
    let out = render_mode(
        BiquadMode::Lowpass,
        &audio,
        8_000.0,
        std::f32::consts::FRAC_1_SQRT_2,
    );
    let pass = gain_db(&audio, &out);
    assert!(
        pass.abs() < 1.0,
        "100 Hz through an 8 kHz lowpass must pass near unity, got {pass:.2} dB"
    );

    let audio = sine_wave(8_000.0, 48_000);
    let out = render_mode(
        BiquadMode::Lowpass,
        &audio,
        100.0,
        std::f32::consts::FRAC_1_SQRT_2,
    );
    let stop = gain_db(&audio, &out);
    assert!(
        stop < -40.0,
        "8 kHz through a 100 Hz lowpass must attenuate strongly, got {stop:.2} dB"
    );
}

#[test]
fn biquad_highpass_mirrors_the_lowpass() {
    let audio = sine_wave(8_000.0, 48_000);
    let out = render_mode(
        BiquadMode::Highpass,
        &audio,
        100.0,
        std::f32::consts::FRAC_1_SQRT_2,
    );
    let pass = gain_db(&audio, &out);
    assert!(
        pass.abs() < 1.0,
        "8 kHz through a 100 Hz highpass must pass near unity, got {pass:.2} dB"
    );

    let audio = sine_wave(100.0, 48_000);
    let out = render_mode(
        BiquadMode::Highpass,
        &audio,
        8_000.0,
        std::f32::consts::FRAC_1_SQRT_2,
    );
    let stop = gain_db(&audio, &out);
    assert!(
        stop < -40.0,
        "100 Hz through an 8 kHz highpass must attenuate strongly, got {stop:.2} dB"
    );
}

#[test]
fn biquad_bandpass_peaks_at_unity_on_center() {
    // The cookbook's "constant 0 dB peak gain" bandpass: exactly unity at the
    // center frequency for any Q.
    let audio = sine_wave(1_000.0, 48_000);
    let out = render_mode(BiquadMode::Bandpass, &audio, 1_000.0, 4.0);
    let center = gain_db(&audio, &out);
    assert!(
        center.abs() < 0.5,
        "bandpass center gain must be ~0 dB, got {center:.2} dB"
    );

    let audio = sine_wave(4_000.0, 48_000);
    let out = render_mode(BiquadMode::Bandpass, &audio, 1_000.0, 4.0);
    let skirt = gain_db(&audio, &out);
    assert!(
        skirt < -12.0,
        "bandpass two octaves off-center must roll off, got {skirt:.2} dB"
    );
}

#[test]
fn biquad_notch_kills_the_center_frequency() {
    // The cookbook notch places its zeros exactly on the unit circle at w0,
    // so a sine at the center frequency nulls out.
    let audio = sine_wave(1_000.0, 48_000);
    let out = render_mode(BiquadMode::Notch, &audio, 1_000.0, 2.0);
    let depth = gain_db(&audio, &out);
    assert!(
        depth < -40.0,
        "the notch must kill its center frequency, got {depth:.2} dB"
    );

    let audio = sine_wave(100.0, 48_000);
    let out = render_mode(BiquadMode::Notch, &audio, 1_000.0, 2.0);
    let pass = gain_db(&audio, &out);
    assert!(
        pass.abs() < 1.0,
        "the notch must pass frequencies far from center, got {pass:.2} dB"
    );
}

#[test]
fn biquad_peaking_gain_matches_gain_db_at_center() {
    // The cookbook peaking EQ's center gain is exactly A^2 = 10^(gain_db/20).
    let audio = sine_wave(1_000.0, 48_000);

    let boosted = render_peaking(&audio, 1_000.0, 1.0, 6.0);
    let boost = gain_db(&audio, &boosted);
    assert!(
        (boost - 6.0).abs() < 0.5,
        "+6 dB peaking boost at center measured {boost:.2} dB"
    );

    let cut = render_peaking(&audio, 1_000.0, 1.0, -9.0);
    let dip = gain_db(&audio, &cut);
    assert!(
        (dip + 9.0).abs() < 0.5,
        "-9 dB peaking cut at center measured {dip:.2} dB"
    );

    // Far from the bell the response returns to unity.
    let audio = sine_wave(60.0, 48_000);
    let far = render_peaking(&audio, 1_000.0, 1.0, 6.0);
    let shoulder = gain_db(&audio, &far);
    assert!(
        shoulder.abs() < 1.0,
        "peaking EQ must be ~0 dB far from center, got {shoulder:.2} dB"
    );
}

#[test]
fn biquad_coefficients_snapshot_at_block_start() {
    // Documented trade-off: parameters are sampled once per block. A freq
    // signal that ramps WITHIN a single block must behave exactly like its
    // block-start value held constant.
    let frames = 256;
    let audio = wiggle(frames);
    let q = vec![1.0_f32; frames];
    let ramp: Vec<f32> = (0..frames)
        .map(|t| 6.0_f32.mul_add(t as f32, 500.0))
        .collect();
    let constant = vec![500.0_f32; frames];

    let mut ramped_node = biquad(SR, BiquadMode::Lowpass);
    let ramped = render3(&mut ramped_node, &audio, &ramp, &q, frames);
    let mut constant_node = biquad(SR, BiquadMode::Lowpass);
    let held = render3(&mut constant_node, &audio, &constant, &q, frames);
    assert_eq!(
        ramped, held,
        "within one block only the block-start freq value may matter"
    );

    // Across a block boundary the new value takes effect.
    let low_then_high: Vec<f32> = (0..frames)
        .map(|t| if t < frames / 2 { 500.0 } else { 8_000.0 })
        .collect();
    let mut stepped_node = biquad(SR, BiquadMode::Lowpass);
    let stepped = render3(&mut stepped_node, &audio, &low_then_high, &q, frames / 2);
    assert_eq!(
        stepped[..frames / 2],
        held[..frames / 2],
        "the first block matches the constant-freq render"
    );
    assert_ne!(
        stepped[frames / 2..],
        held[frames / 2..],
        "the second block must pick up the new block-start freq"
    );
}

#[test]
fn biquad_stays_bounded_under_swept_parameters() {
    // Three seconds of broadband input while an LFO sweeps the center
    // frequency across 100..10000 Hz at high Q, with block-rate coefficient
    // updates (64-sample blocks). No NaN/inf, bounded output.
    let frames = 144_000;
    let audio = wiggle(frames);
    let freq: Vec<f32> = (0..frames)
        .map(|t| 4_950.0_f32.mul_add((TAU * 150.0 * t as f32 / SR).sin(), 5_050.0))
        .collect();
    let q = vec![8.0_f32; frames];

    for mode in [
        BiquadMode::Lowpass,
        BiquadMode::Highpass,
        BiquadMode::Bandpass,
        BiquadMode::Notch,
    ] {
        let mut node = biquad(SR, mode);
        let out = render3(&mut node, &audio, &freq, &q, 64);
        for (i, &sample) in out.iter().enumerate() {
            assert!(
                sample.is_finite(),
                "{mode:?} sample {i} must be finite under a swept center frequency"
            );
            assert!(
                sample.abs() < 100.0,
                "{mode:?} sample {i} must stay bounded, got {sample}"
            );
        }
    }

    let gain = vec![12.0_f32; frames];
    let mut node = biquad(SR, BiquadMode::Peaking);
    let out = render4(&mut node, &audio, &freq, &q, &gain, 64);
    for (i, &sample) in out.iter().enumerate() {
        assert!(sample.is_finite(), "Peaking sample {i} must be finite");
        assert!(
            sample.abs() < 100.0,
            "Peaking sample {i} must stay bounded, got {sample}"
        );
    }
}

#[test]
fn biquad_clamps_hostile_parameters() {
    let frames = 4_096;
    let audio = wiggle(frames);
    for (freq_value, q_value, gain_value) in [
        (f32::NAN, 1.0_f32, 6.0_f32),
        (f32::INFINITY, 1.0, 6.0),
        (-500.0, 1.0, 6.0),
        (1.0e9, 1.0, 6.0),
        (1_000.0, f32::NAN, 6.0),
        (1_000.0, -4.0, 6.0),
        (1_000.0, 1.0e9, 6.0),
        (1_000.0, 1.0, f32::NAN),
        (1_000.0, 1.0, 1.0e9),
        (0.0, 0.0, 0.0),
    ] {
        let freq = vec![freq_value; frames];
        let q = vec![q_value; frames];
        let gain = vec![gain_value; frames];
        let mut node = biquad(SR, BiquadMode::Peaking);
        let out = render4(&mut node, &audio, &freq, &q, &gain, 256);
        assert!(
            out.iter().all(|s| s.is_finite()),
            "freq {freq_value}, q {q_value}, gain {gain_value} must yield finite output"
        );
    }
}

#[test]
fn biquad_reset_restores_initial_state() {
    let audio = wiggle(2_048);
    let mut node = biquad(SR, BiquadMode::Bandpass);
    let freq = vec![900.0_f32; audio.len()];
    let q = vec![5.0_f32; audio.len()];
    let first = render3(&mut node, &audio, &freq, &q, 512);
    node.reset();
    let second = render3(&mut node, &audio, &freq, &q, 512);
    assert_eq!(first, second, "reset must clear all filter state");
}
