//! Tests for existing-primitive adapters.

#![allow(clippy::cast_precision_loss)]

use orpheus_dsp::{Gain, LadderFilter, Mix, Noise, SawOsc, SoftSat, TriOsc};
use orpheus_dsp::{Node, gain_node, ladder_filter, mix_node, noise, saw, soft_sat, tri};

const SR: f32 = 48_000.0;
const FRAMES: usize = 256;

// ---------------------------------------------------------------------------
// SawNode adapter
// ---------------------------------------------------------------------------

#[test]
fn saw_adapter_matches_raw_saw_osc() {
    let mut node = saw(SR);
    let mut raw = SawOsc::new(SR);

    let freq = vec![440.0_f32; FRAMES];
    let mut node_out = vec![0.0_f32; FRAMES];
    node.process(&[&freq], &mut [&mut node_out], FRAMES);

    let raw_out: Vec<f32> = (0..FRAMES).map(|_| raw.next_sample(440.0)).collect();

    // Bit-identical output.
    assert_eq!(node_out, raw_out);
}

#[test]
fn saw_adapter_reset_matches_raw_reset() {
    let mut node = saw(SR);
    let mut raw = SawOsc::new(SR);

    let freq = vec![440.0_f32; FRAMES];
    let mut buf = vec![0.0_f32; FRAMES];

    node.process(&[&freq], &mut [&mut buf], FRAMES);
    node.reset();

    for _ in 0..FRAMES {
        let _ = raw.next_sample(440.0);
    }
    raw.reset();

    let mut node_out = vec![0.0_f32; FRAMES];
    node.process(&[&freq], &mut [&mut node_out], FRAMES);

    let raw_out: Vec<f32> = (0..FRAMES).map(|_| raw.next_sample(440.0)).collect();

    assert_eq!(node_out, raw_out);
}

// ---------------------------------------------------------------------------
// TriNode adapter
// ---------------------------------------------------------------------------

#[test]
fn tri_adapter_matches_raw_tri_osc() {
    let mut node = tri(SR);
    let mut raw = TriOsc::new(SR);

    let freq = vec![440.0_f32; FRAMES];
    let mut node_out = vec![0.0_f32; FRAMES];
    node.process(&[&freq], &mut [&mut node_out], FRAMES);

    let raw_out: Vec<f32> = (0..FRAMES).map(|_| raw.next_sample(440.0)).collect();

    assert_eq!(node_out, raw_out);
}

// ---------------------------------------------------------------------------
// NoiseNode adapter
// ---------------------------------------------------------------------------

#[test]
fn noise_adapter_deterministic_after_reset() {
    let mut node = noise(12345);

    let mut out1 = vec![0.0_f32; FRAMES];
    node.process(&[], &mut [&mut out1], FRAMES);

    node.reset();

    let mut out2 = vec![0.0_f32; FRAMES];
    node.process(&[], &mut [&mut out2], FRAMES);

    assert_eq!(out1, out2);
}

#[test]
fn noise_adapter_matches_raw_noise() {
    let mut node = noise(42);
    let mut raw = Noise::new(42);

    let mut node_out = vec![0.0_f32; FRAMES];
    node.process(&[], &mut [&mut node_out], FRAMES);

    let raw_out: Vec<f32> = (0..FRAMES).map(|_| raw.next_sample()).collect();

    assert_eq!(node_out, raw_out);
}

// ---------------------------------------------------------------------------
// LadderFilterNode adapter
// ---------------------------------------------------------------------------

#[test]
fn ladder_adapter_filters_signal() {
    let mut node = ladder_filter(SR);

    // High-frequency input through a low cutoff.
    let audio: Vec<f32> = (0..FRAMES)
        .map(|i| if i % 2 == 0 { 1.0 } else { -1.0 })
        .collect();
    let cutoff = vec![200.0_f32; FRAMES];
    let res = vec![0.0_f32; FRAMES];
    let mut out = vec![0.0_f32; FRAMES];

    node.process(&[&audio, &cutoff, &res], &mut [&mut out], FRAMES);

    let input_energy: f32 = audio.iter().map(|s| s * s).sum();
    let output_energy: f32 = out.iter().map(|s| s * s).sum();
    assert!(output_energy < input_energy * 0.1);
}

#[test]
fn ladder_adapter_matches_raw_filter() {
    let mut node = ladder_filter(SR);
    let mut raw = LadderFilter::new(SR);

    let audio = vec![0.5_f32; FRAMES];
    let cutoff = vec![2000.0_f32; FRAMES];
    let res = vec![0.3_f32; FRAMES];
    let mut node_out = vec![0.0_f32; FRAMES];

    node.process(&[&audio, &cutoff, &res], &mut [&mut node_out], FRAMES);

    let raw_out: Vec<f32> = (0..FRAMES).map(|_| raw.process(0.5, 2000.0, 0.3)).collect();

    assert_eq!(node_out, raw_out);
}

// ---------------------------------------------------------------------------
// GainNode adapter
// ---------------------------------------------------------------------------

#[test]
fn gain_adapter_scales_correctly() {
    let mut node = gain_node();

    let audio = vec![1.0_f32; FRAMES];
    let amount = vec![0.5_f32; FRAMES];
    let mut out = vec![0.0_f32; FRAMES];

    node.process(&[&audio, &amount], &mut [&mut out], FRAMES);

    assert!(out.iter().all(|&s| (s - 0.5).abs() < f32::EPSILON));
}

#[test]
fn gain_adapter_matches_raw_gain() {
    let mut node = gain_node();
    let mut raw = Gain::new();

    let audio = vec![0.7_f32; FRAMES];
    let amount = vec![0.3_f32; FRAMES];
    let mut node_out = vec![0.0_f32; FRAMES];

    node.process(&[&audio, &amount], &mut [&mut node_out], FRAMES);

    let raw_out: Vec<f32> = (0..FRAMES).map(|_| raw.process(0.7, 0.3)).collect();

    assert_eq!(node_out, raw_out);
}

// ---------------------------------------------------------------------------
// SoftSatNode adapter
// ---------------------------------------------------------------------------

#[test]
fn soft_sat_adapter_matches_raw() {
    let mut node = soft_sat();
    let mut raw = SoftSat::new();

    let audio = vec![0.8_f32; FRAMES];
    let drive = vec![2.0_f32; FRAMES];
    let mut node_out = vec![0.0_f32; FRAMES];

    node.process(&[&audio, &drive], &mut [&mut node_out], FRAMES);

    let raw_out: Vec<f32> = (0..FRAMES).map(|_| raw.process(0.8, 2.0)).collect();

    assert_eq!(node_out, raw_out);
}

// ---------------------------------------------------------------------------
// MixNode adapter
// ---------------------------------------------------------------------------

#[test]
fn mix_adapter_channel_counts() {
    let node = mix_node();
    assert_eq!(node.inputs(), 3);
    assert_eq!(node.outputs(), 1);
}

#[test]
fn mix_adapter_balance_extremes_select_inputs() {
    let mut node = mix_node();
    let left = vec![0.25_f32; FRAMES];
    let right = vec![-0.75_f32; FRAMES];
    let mut out = vec![0.0_f32; FRAMES];

    // Balance 0 selects the first input entirely.
    let balance = vec![0.0_f32; FRAMES];
    node.process(&[&left, &right, &balance], &mut [&mut out], FRAMES);
    assert!(out.iter().all(|&s| (s - 0.25).abs() < f32::EPSILON));

    // Balance 1 selects the second input entirely.
    let balance = vec![1.0_f32; FRAMES];
    node.process(&[&left, &right, &balance], &mut [&mut out], FRAMES);
    assert!(out.iter().all(|&s| (s - (-0.75)).abs() < f32::EPSILON));
}

#[test]
fn mix_adapter_matches_raw_mix_blend() {
    let mut node = mix_node();

    // Varying signals and a mid-point balance sweep.
    let left: Vec<f32> = (0..FRAMES).map(|i| (i as f32 * 0.013).sin()).collect();
    let right: Vec<f32> = (0..FRAMES).map(|i| (i as f32 * 0.007).cos()).collect();
    let balance: Vec<f32> = (0..FRAMES).map(|i| i as f32 / FRAMES as f32).collect();
    let mut node_out = vec![0.0_f32; FRAMES];

    node.process(&[&left, &right, &balance], &mut [&mut node_out], FRAMES);

    let raw_out: Vec<f32> = (0..FRAMES)
        .map(|i| Mix::blend(left[i], right[i], balance[i]))
        .collect();

    // Bit-identical output.
    assert_eq!(node_out, raw_out);
}

#[test]
fn mix_adapter_midpoint_matches_reference() {
    let mut node = mix_node();
    let left = vec![1.0_f32; FRAMES];
    let right = vec![0.0_f32; FRAMES];
    let balance = vec![0.5_f32; FRAMES];
    let mut out = vec![0.0_f32; FRAMES];

    node.process(&[&left, &right, &balance], &mut [&mut out], FRAMES);

    let reference = Mix::blend(1.0, 0.0, 0.5);
    assert!(out.iter().all(|&s| (s - reference).abs() < f32::EPSILON));
}
