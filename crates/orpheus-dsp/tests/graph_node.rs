//! Tests for the Node trait and leaf primitive nodes.

#![allow(clippy::cast_precision_loss)]

use orpheus_dsp::graph::{Node, constant, delay_line, one_pole, passthrough, sine, sum, wire};

const SR: f32 = 48_000.0;
const FRAMES: usize = 128;

// ---------------------------------------------------------------------------
// ConstNode
// ---------------------------------------------------------------------------

#[test]
fn const_node_fills_output_with_value() {
    let mut node = constant(0.75);
    assert_eq!(node.inputs(), 0);
    assert_eq!(node.outputs(), 1);

    let mut out = vec![0.0_f32; FRAMES];
    node.process(&[], &mut [&mut out], FRAMES);

    assert!(out.iter().all(|&s| (s - 0.75).abs() < f32::EPSILON));
}

#[test]
fn const_node_negative_value() {
    let mut node = constant(-1.0);
    let mut out = vec![0.0_f32; FRAMES];
    node.process(&[], &mut [&mut out], FRAMES);
    assert!(out.iter().all(|&s| (s - (-1.0)).abs() < f32::EPSILON));
}

// ---------------------------------------------------------------------------
// SineNode
// ---------------------------------------------------------------------------

#[test]
fn sine_node_channel_counts() {
    let node = sine(SR);
    assert_eq!(node.inputs(), 1);
    assert_eq!(node.outputs(), 1);
}

#[test]
fn sine_node_produces_finite_nonzero_output() {
    let mut node = sine(SR);
    let freq = vec![440.0_f32; FRAMES];
    let mut out = vec![0.0_f32; FRAMES];

    node.process(&[&freq], &mut [&mut out], FRAMES);

    assert!(out.iter().all(|s| s.is_finite()));
    assert!(out.iter().any(|&s| s.abs() > 0.001));
}

#[test]
fn sine_node_is_bounded() {
    let mut node = sine(SR);
    let freq = vec![440.0_f32; 4096];
    let mut out = vec![0.0_f32; 4096];

    node.process(&[&freq], &mut [&mut out], 4096);

    assert!(out.iter().all(|&s| (-1.0..=1.0).contains(&s)));
}

#[test]
fn sine_node_reset_restarts_phase() {
    let mut node = sine(SR);
    let freq = vec![440.0_f32; FRAMES];
    let mut out1 = vec![0.0_f32; FRAMES];
    let mut out2 = vec![0.0_f32; FRAMES];

    node.process(&[&freq], &mut [&mut out1], FRAMES);
    node.reset();
    node.process(&[&freq], &mut [&mut out2], FRAMES);

    assert_eq!(out1, out2);
}

// ---------------------------------------------------------------------------
// DelayNode
// ---------------------------------------------------------------------------

#[test]
fn delay_node_delays_impulse_by_n_samples() {
    let delay = 10;
    let mut node = delay_line(delay);
    assert_eq!(node.inputs(), 1);
    assert_eq!(node.outputs(), 1);

    // Send an impulse at sample 0.
    let mut inp = vec![0.0_f32; FRAMES];
    inp[0] = 1.0;
    let mut out = vec![0.0_f32; FRAMES];

    node.process(&[&inp], &mut [&mut out], FRAMES);

    // The impulse should appear at index `delay`.
    assert!(out[..delay].iter().all(|&s| s == 0.0));
    assert!((out[delay] - 1.0).abs() < f32::EPSILON);
    assert!(out[delay + 1..].iter().all(|&s| s == 0.0));
}

#[test]
fn delay_node_one_sample_delay() {
    let mut node = delay_line(1);
    let inp = [1.0_f32, 2.0, 3.0];
    let mut out = [0.0_f32; 3];

    node.process(&[&inp[..]], &mut [&mut out[..]], 3);

    // Delay of 1: output is [0, 1, 2].
    assert!((out[0] - 0.0).abs() < f32::EPSILON);
    assert!((out[1] - 1.0).abs() < f32::EPSILON);
    assert!((out[2] - 2.0).abs() < f32::EPSILON);
}

#[test]
fn delay_node_reset_clears_buffer() {
    let mut node = delay_line(4);
    let inp = vec![1.0_f32; 8];
    let mut out = vec![0.0_f32; 8];

    node.process(&[&inp], &mut [&mut out], 8);
    node.reset();

    // After reset, output should be zeros again initially.
    let inp2 = vec![0.0_f32; 4];
    let mut out2 = vec![999.0_f32; 4];
    node.process(&[&inp2], &mut [&mut out2], 4);
    assert!(out2.iter().all(|&s| s == 0.0));
}

// ---------------------------------------------------------------------------
// OnePoleNode
// ---------------------------------------------------------------------------

#[test]
fn one_pole_channel_counts() {
    let node = one_pole(SR);
    assert_eq!(node.inputs(), 2);
    assert_eq!(node.outputs(), 1);
}

#[test]
fn one_pole_attenuates_high_frequencies() {
    let mut node = one_pole(SR);
    let frames = 4096;

    // Generate a high-frequency signal (Nyquist / 2).
    let high_freq: Vec<f32> = (0..frames)
        .map(|i| if i % 2 == 0 { 1.0 } else { -1.0 })
        .collect();
    let cutoff = vec![200.0_f32; frames]; // Low cutoff.
    let mut out = vec![0.0_f32; frames];

    node.process(&[&high_freq, &cutoff], &mut [&mut out], frames);

    // The output energy should be much lower than input energy.
    let input_energy: f32 = high_freq.iter().map(|s| s * s).sum();
    let output_energy: f32 = out.iter().map(|s| s * s).sum();
    assert!(output_energy < input_energy * 0.1);
}

#[test]
fn one_pole_passes_low_frequencies() {
    let mut node = one_pole(SR);
    let frames = 4096;

    // 100 Hz sine — well below a 10 kHz cutoff.
    let audio: Vec<f32> = (0..frames)
        .map(|i| (i as f32 / SR * 100.0 * std::f32::consts::TAU).sin())
        .collect();
    let cutoff = vec![10_000.0_f32; frames];
    let mut out = vec![0.0_f32; frames];

    node.process(&[&audio, &cutoff], &mut [&mut out], frames);

    // Output energy should be a substantial fraction of input energy.
    let input_energy: f32 = audio.iter().map(|s| s * s).sum();
    let output_energy: f32 = out.iter().map(|s| s * s).sum();
    assert!(output_energy > input_energy * 0.8);
}

// ---------------------------------------------------------------------------
// PassthroughNode
// ---------------------------------------------------------------------------

#[test]
fn passthrough_is_identity() {
    let mut node = passthrough(2);
    assert_eq!(node.inputs(), 2);
    assert_eq!(node.outputs(), 2);

    let in0: Vec<f32> = (0..FRAMES).map(|i| i as f32).collect();
    let in1: Vec<f32> = (0..FRAMES).map(|i| -(i as f32)).collect();
    let mut out0 = vec![0.0_f32; FRAMES];
    let mut out1 = vec![0.0_f32; FRAMES];

    node.process(&[&in0, &in1], &mut [&mut out0, &mut out1], FRAMES);

    assert_eq!(out0, in0);
    assert_eq!(out1, in1);
}

// ---------------------------------------------------------------------------
// SumNode
// ---------------------------------------------------------------------------

#[test]
fn sum_node_sums_inputs() {
    let mut node = sum(3);
    assert_eq!(node.inputs(), 3);
    assert_eq!(node.outputs(), 1);

    let a = vec![1.0_f32; FRAMES];
    let b = vec![2.0_f32; FRAMES];
    let c = vec![3.0_f32; FRAMES];
    let mut out = vec![0.0_f32; FRAMES];

    node.process(&[&a, &b, &c], &mut [&mut out], FRAMES);

    assert!(out.iter().all(|&s| (s - 6.0).abs() < f32::EPSILON));
}

#[test]
fn sum_node_single_input_is_identity() {
    let mut node = sum(1);
    let a = vec![42.0_f32; FRAMES];
    let mut out = vec![0.0_f32; FRAMES];

    node.process(&[&a], &mut [&mut out], FRAMES);

    assert!(out.iter().all(|&s| (s - 42.0).abs() < f32::EPSILON));
}

// ---------------------------------------------------------------------------
// WireNode
// ---------------------------------------------------------------------------

#[test]
fn wire_node_reorders_channels() {
    // 3 inputs, 2 outputs: output[0] = input[2], output[1] = input[0]
    let mut node = wire(&[2, 0]);
    assert_eq!(node.inputs(), 3);
    assert_eq!(node.outputs(), 2);

    let in0 = vec![10.0_f32; FRAMES];
    let in1 = vec![20.0_f32; FRAMES];
    let in2 = vec![30.0_f32; FRAMES];
    let mut out0 = vec![0.0_f32; FRAMES];
    let mut out1 = vec![0.0_f32; FRAMES];

    node.process(&[&in0, &in1, &in2], &mut [&mut out0, &mut out1], FRAMES);

    assert!(out0.iter().all(|&s| (s - 30.0).abs() < f32::EPSILON));
    assert!(out1.iter().all(|&s| (s - 10.0).abs() < f32::EPSILON));
}

#[test]
fn wire_node_duplicates_channel() {
    // 1 input, 3 outputs: all outputs read from input[0]
    let mut node = wire(&[0, 0, 0]);
    assert_eq!(node.inputs(), 1);
    assert_eq!(node.outputs(), 3);

    let inp = vec![7.0_f32; FRAMES];
    let mut out0 = vec![0.0_f32; FRAMES];
    let mut out1 = vec![0.0_f32; FRAMES];
    let mut out2 = vec![0.0_f32; FRAMES];

    node.process(&[&inp], &mut [&mut out0, &mut out1, &mut out2], FRAMES);

    assert!(out0.iter().all(|&s| (s - 7.0).abs() < f32::EPSILON));
    assert!(out1.iter().all(|&s| (s - 7.0).abs() < f32::EPSILON));
    assert!(out2.iter().all(|&s| (s - 7.0).abs() < f32::EPSILON));
}
