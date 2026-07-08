//! Tests for the state-variable filter node (`svf`).
//!
//! `svf` is a Cytomic (Andrew Simper) TPT state-variable filter: 3 inputs
//! (audio, cutoff\_hz, q), 4 simultaneous outputs (lowpass, highpass,
//! bandpass, notch). Coefficients are recomputed every sample from the input
//! signals, so the cutoff may sweep at audio rate without losing stability —
//! that property is asserted below, alongside frequency-response spot checks
//! and the topology's exact complementarity identity
//! (`lowpass + bandpass + highpass == input`).

#![allow(clippy::cast_precision_loss, clippy::cast_possible_truncation)]

use std::f32::consts::TAU;

use orpheus_dsp::{Node, svf};

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

/// Drives the 3-in/4-out svf over per-sample cutoff and Q signals in
/// `block`-sized chunks. Returns `[lowpass, highpass, bandpass, notch]`.
fn render_svf_signals(
    node: &mut dyn Node,
    audio: &[f32],
    cutoff: &[f32],
    q: &[f32],
    block: usize,
) -> [Vec<f32>; 4] {
    assert_eq!(node.inputs(), 3);
    assert_eq!(node.outputs(), 4);
    let frames = audio.len();
    let mut lp = vec![0.0_f32; frames];
    let mut hp = vec![0.0_f32; frames];
    let mut bp = vec![0.0_f32; frames];
    let mut notch = vec![0.0_f32; frames];
    let mut start = 0;
    while start < frames {
        let end = (start + block).min(frames);
        let inputs: [&[f32]; 3] = [&audio[start..end], &cutoff[start..end], &q[start..end]];
        let mut outputs: [&mut [f32]; 4] = [
            &mut lp[start..end],
            &mut hp[start..end],
            &mut bp[start..end],
            &mut notch[start..end],
        ];
        node.process(&inputs, &mut outputs, end - start);
        start = end;
    }
    [lp, hp, bp, notch]
}

/// Renders with constant cutoff/Q parameters.
fn render_svf(node: &mut dyn Node, audio: &[f32], cutoff_hz: f32, q: f32) -> [Vec<f32>; 4] {
    let cutoff = vec![cutoff_hz; audio.len()];
    let quality = vec![q; audio.len()];
    render_svf_signals(node, audio, &cutoff, &quality, 512)
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
fn svf_channel_counts() {
    let node = svf(SR);
    assert_eq!(node.inputs(), 3);
    assert_eq!(node.outputs(), 4);
}

#[test]
fn svf_lowpass_passes_below_cutoff_and_attenuates_above() {
    let audio = sine_wave(100.0, 48_000);
    let mut node = svf(SR);
    let [lp, ..] = render_svf(&mut node, &audio, 8_000.0, std::f32::consts::FRAC_1_SQRT_2);
    let pass = gain_db(&audio, &lp);
    assert!(
        pass.abs() < 1.0,
        "100 Hz through an 8 kHz lowpass must pass near unity, got {pass:.2} dB"
    );

    let audio = sine_wave(8_000.0, 48_000);
    let mut node = svf(SR);
    let [lp, ..] = render_svf(&mut node, &audio, 100.0, std::f32::consts::FRAC_1_SQRT_2);
    let stop = gain_db(&audio, &lp);
    assert!(
        stop < -40.0,
        "8 kHz through a 100 Hz lowpass must attenuate strongly, got {stop:.2} dB"
    );
}

#[test]
fn svf_highpass_mirrors_the_lowpass() {
    let audio = sine_wave(8_000.0, 48_000);
    let mut node = svf(SR);
    let [_, hp, ..] = render_svf(&mut node, &audio, 100.0, std::f32::consts::FRAC_1_SQRT_2);
    let pass = gain_db(&audio, &hp);
    assert!(
        pass.abs() < 1.0,
        "8 kHz through a 100 Hz highpass must pass near unity, got {pass:.2} dB"
    );

    let audio = sine_wave(100.0, 48_000);
    let mut node = svf(SR);
    let [_, hp, ..] = render_svf(&mut node, &audio, 8_000.0, std::f32::consts::FRAC_1_SQRT_2);
    let stop = gain_db(&audio, &hp);
    assert!(
        stop < -40.0,
        "100 Hz through an 8 kHz highpass must attenuate strongly, got {stop:.2} dB"
    );
}

#[test]
fn svf_bandpass_is_unity_at_center_and_rolls_off_away() {
    // The bandpass output is normalized (k * v1), so the center gain is
    // exactly 0 dB regardless of Q.
    let audio = sine_wave(1_000.0, 48_000);
    let mut node = svf(SR);
    let [_, _, bp, _] = render_svf(&mut node, &audio, 1_000.0, 2.0);
    let center = gain_db(&audio, &bp);
    assert!(
        center.abs() < 0.5,
        "bandpass center gain must be ~0 dB, got {center:.2} dB"
    );

    // Two octaves above center with Q = 2 the analog prototype predicts
    // roughly -17.6 dB; assert a comfortable margin.
    let audio = sine_wave(4_000.0, 48_000);
    let mut node = svf(SR);
    let [_, _, bp, _] = render_svf(&mut node, &audio, 1_000.0, 2.0);
    let skirt = gain_db(&audio, &bp);
    assert!(
        skirt < -12.0,
        "bandpass two octaves off-center must roll off, got {skirt:.2} dB"
    );
}

#[test]
fn svf_notch_kills_the_center_frequency() {
    // The TPT prewarp maps the cutoff exactly, so a sine at the center
    // frequency falls into the notch's null.
    let audio = sine_wave(1_000.0, 48_000);
    let mut node = svf(SR);
    let [.., notch] = render_svf(&mut node, &audio, 1_000.0, 2.0);
    let depth = gain_db(&audio, &notch);
    assert!(
        depth < -40.0,
        "the notch must kill its center frequency, got {depth:.2} dB"
    );

    // Well away from the notch the signal passes.
    let audio = sine_wave(100.0, 48_000);
    let mut node = svf(SR);
    let [.., notch] = render_svf(&mut node, &audio, 1_000.0, 2.0);
    let pass = gain_db(&audio, &notch);
    assert!(
        pass.abs() < 1.0,
        "the notch must pass frequencies far from center, got {pass:.2} dB"
    );
}

#[test]
fn svf_outputs_are_complementary() {
    // For this topology (unity-normalized bandpass) the identity is exact:
    // lowpass + bandpass + highpass reconstructs the input, and the notch is
    // lowpass + highpass. Verified per sample on a broadband signal.
    let frames = 8_192;
    let audio = wiggle(frames);
    let mut node = svf(SR);
    let [lp, hp, bp, notch] = render_svf(&mut node, &audio, 1_500.0, 3.0);

    for i in 0..frames {
        let reconstructed = lp[i] + bp[i] + hp[i];
        assert!(
            (reconstructed - audio[i]).abs() < 1e-3,
            "sample {i}: lp + bp + hp must reconstruct the input \
             ({reconstructed} vs {})",
            audio[i]
        );
        assert!(
            (notch[i] - (lp[i] + hp[i])).abs() < 1e-3,
            "sample {i}: notch must equal lp + hp"
        );
    }
}

#[test]
fn svf_stays_bounded_under_audio_rate_cutoff_modulation() {
    // A 150 Hz LFO sweeps the cutoff across 100..10000 Hz at high resonance
    // for three seconds of broadband input. The TPT structure must stay
    // stable: every sample of every output finite and bounded.
    let frames = 144_000;
    let audio = wiggle(frames);
    let cutoff: Vec<f32> = (0..frames)
        .map(|t| 4_950.0_f32.mul_add((TAU * 150.0 * t as f32 / SR).sin(), 5_050.0))
        .collect();
    let q = vec![10.0_f32; frames];

    let mut node = svf(SR);
    let outputs = render_svf_signals(&mut node, &audio, &cutoff, &q, 512);

    for (name, signal) in ["lowpass", "highpass", "bandpass", "notch"]
        .iter()
        .zip(outputs.iter())
    {
        for (i, &sample) in signal.iter().enumerate() {
            assert!(
                sample.is_finite(),
                "{name} sample {i} must be finite under audio-rate modulation"
            );
            assert!(
                sample.abs() < 100.0,
                "{name} sample {i} must stay bounded, got {sample}"
            );
        }
    }
}

#[test]
fn svf_clamps_hostile_parameters() {
    // Non-finite and out-of-range cutoff/Q values must never produce
    // non-finite output.
    let frames = 4_096;
    let audio = wiggle(frames);
    for (cutoff_value, q_value) in [
        (f32::NAN, 0.707_f32),
        (f32::INFINITY, 0.707),
        (-500.0, 0.707),
        (1.0e9, 0.707),
        (1_000.0, f32::NAN),
        (1_000.0, -4.0),
        (1_000.0, 1.0e9),
        (0.0, 0.0),
    ] {
        let cutoff = vec![cutoff_value; frames];
        let q = vec![q_value; frames];
        let mut node = svf(SR);
        let outputs = render_svf_signals(&mut node, &audio, &cutoff, &q, 256);
        for signal in &outputs {
            assert!(
                signal.iter().all(|s| s.is_finite()),
                "cutoff {cutoff_value}, q {q_value} must yield finite output"
            );
        }
    }
}

#[test]
fn svf_reset_restores_initial_state() {
    let audio = wiggle(2_048);
    let mut node = svf(SR);
    let first = render_svf(&mut node, &audio, 800.0, 4.0);
    node.reset();
    let second = render_svf(&mut node, &audio, 800.0, 4.0);
    assert_eq!(first, second, "reset must clear all filter state");
}
