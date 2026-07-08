//! Tests for the Node trait and leaf primitive nodes.

#![allow(clippy::cast_precision_loss)]

use orpheus_dsp::{
    Node, adsr, ar, constant, delay_line, one_pole, pan, passthrough, sine, sum, wire,
};

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

// ---------------------------------------------------------------------------
// AdsrNode
// ---------------------------------------------------------------------------

/// Renders an ADSR envelope with constant parameters and the given gate signal.
fn render_adsr(
    node: &mut dyn Node,
    gate: &[f32],
    attack_s: f32,
    decay_s: f32,
    sustain_level: f32,
    release_s: f32,
) -> Vec<f32> {
    let frames = gate.len();
    let a = vec![attack_s; frames];
    let d = vec![decay_s; frames];
    let s = vec![sustain_level; frames];
    let r = vec![release_s; frames];
    let mut out = vec![0.0_f32; frames];
    node.process(&[gate, &a, &d, &s, &r], &mut [&mut out], frames);
    out
}

#[test]
fn adsr_node_channel_counts() {
    let node = adsr(SR);
    assert_eq!(node.inputs(), 5);
    assert_eq!(node.outputs(), 1);
}

#[test]
fn adsr_attack_ramps_to_one_in_attack_time() {
    let mut node = adsr(SR);
    let attack_samples = 480_usize; // 10 ms at 48 kHz
    let gate = vec![1.0_f32; 2048];
    let out = render_adsr(
        &mut node,
        &gate,
        attack_samples as f32 / SR,
        0.005,
        0.5,
        0.01,
    );

    let first_peak = out
        .iter()
        .position(|&v| v >= 1.0 - 1e-4)
        .expect("attack should reach 1.0");
    assert!(
        first_peak.abs_diff(attack_samples - 1) <= 2,
        "attack should complete after ~{attack_samples} samples, peaked at {first_peak}"
    );
    // Envelope rises monotonically during the attack segment.
    assert!(out[..first_peak].windows(2).all(|w| w[1] >= w[0]));
    // And starts near zero.
    assert!(out[0] < 0.01);
}

#[test]
fn adsr_decay_settles_to_sustain_level() {
    let mut node = adsr(SR);
    let attack_samples = 48_usize;
    let decay_samples = 480_usize;
    let sustain = 0.5_f32;
    let gate = vec![1.0_f32; 4800];
    let out = render_adsr(
        &mut node,
        &gate,
        attack_samples as f32 / SR,
        decay_samples as f32 / SR,
        sustain,
        0.01,
    );

    let peak = out
        .iter()
        .position(|&v| v >= 1.0 - 1e-4)
        .expect("attack should reach 1.0");
    let expected_settle = attack_samples + decay_samples - 1;
    let settle = peak
        + out[peak..]
            .iter()
            .position(|&v| (v - sustain).abs() < 1e-3)
            .expect("decay should settle to sustain");
    assert!(
        settle.abs_diff(expected_settle) <= 4,
        "decay should settle near sample {expected_settle}, settled at {settle}"
    );
    // Once settled, it stays exactly at sustain while the gate is held.
    assert!(
        out[settle + 8..]
            .iter()
            .all(|&v| (v - sustain).abs() < 1e-3)
    );
}

#[test]
fn adsr_holds_sustain_while_gate_held() {
    let mut node = adsr(SR);
    let gate = vec![1.0_f32; 48_000]; // one full second, gate never falls
    let out = render_adsr(&mut node, &gate, 0.001, 0.002, 0.6, 0.01);

    assert!((out[47_999] - 0.6).abs() < 1e-3);
    assert!((out[24_000] - 0.6).abs() < 1e-3);
}

#[test]
fn adsr_release_decays_to_zero_in_release_time() {
    let mut node = adsr(SR);
    let gate_off = 2400_usize;
    let release_samples = 480_usize;
    let mut gate = vec![0.0_f32; 4800];
    gate[..gate_off].fill(1.0);
    let out = render_adsr(
        &mut node,
        &gate,
        0.001,
        0.002,
        0.5,
        release_samples as f32 / SR,
    );

    // Sustaining right before the gate falls.
    assert!((out[gate_off - 1] - 0.5).abs() < 1e-3);

    let expected_zero = gate_off + release_samples - 1;
    let first_zero = gate_off
        + out[gate_off..]
            .iter()
            .position(|&v| v <= 1e-4)
            .expect("release should reach zero");
    assert!(
        first_zero.abs_diff(expected_zero) <= 2,
        "release should reach zero near sample {expected_zero}, reached at {first_zero}"
    );
    // Stays silent afterwards.
    assert!(out[first_zero..].iter().all(|&v| v <= 1e-4));
}

#[test]
fn adsr_retrigger_restarts_attack_without_discontinuity() {
    let mut node = adsr(SR);
    let attack_samples = 480_usize;
    let release_samples = 960_usize;
    // Gate: on, off mid-note, back on while the release is still audible.
    let mut gate = vec![1.0_f32; 4000];
    gate[2000..2200].fill(0.0);
    let out = render_adsr(
        &mut node,
        &gate,
        attack_samples as f32 / SR,
        480.0 / SR,
        0.5,
        release_samples as f32 / SR,
    );

    // Still mid-release (nonzero) when retriggered.
    assert!(out[2199] > 0.1);
    // Retrigger resumes rising from the current level.
    assert!(out[2210] > out[2199]);

    // No discontinuity anywhere: per-sample delta bounded by the steepest
    // segment slope (attack: 1/480 per sample).
    let max_step = 1.0 / attack_samples as f32 + 1e-5;
    for (i, w) in out.windows(2).enumerate() {
        assert!(
            (w[1] - w[0]).abs() <= max_step,
            "discontinuity at sample {i}: {} -> {}",
            w[0],
            w[1]
        );
    }
}

#[test]
fn adsr_reset_returns_to_idle() {
    let mut node = adsr(SR);
    let gate = vec![1.0_f32; 512];
    let out1 = render_adsr(&mut node, &gate, 0.005, 0.005, 0.5, 0.01);
    node.reset();
    let out2 = render_adsr(&mut node, &gate, 0.005, 0.005, 0.5, 0.01);
    assert_eq!(out1, out2);
}

// ---------------------------------------------------------------------------
// ArNode
// ---------------------------------------------------------------------------

#[test]
fn ar_node_channel_counts() {
    let node = ar(SR);
    assert_eq!(node.inputs(), 3);
    assert_eq!(node.outputs(), 1);
}

#[test]
fn ar_reaches_one_in_attack_time_then_holds() {
    let mut node = ar(SR);
    let attack_samples = 480_usize;
    let frames = 2400;
    let gate = vec![1.0_f32; frames];
    let a = vec![attack_samples as f32 / SR; frames];
    let r = vec![0.01_f32; frames];
    let mut out = vec![0.0_f32; frames];
    node.process(&[&gate, &a, &r], &mut [&mut out], frames);

    let first_peak = out
        .iter()
        .position(|&v| v >= 1.0 - 1e-4)
        .expect("attack should reach 1.0");
    assert!(
        first_peak.abs_diff(attack_samples - 1) <= 2,
        "attack should complete after ~{attack_samples} samples, peaked at {first_peak}"
    );
    // No decay stage: holds at full level while the gate is high.
    assert!(out[first_peak..].iter().all(|&v| v >= 1.0 - 1e-3));
}

#[test]
fn ar_releases_to_zero_after_gate_falls() {
    let mut node = ar(SR);
    let gate_off = 2400_usize;
    let release_samples = 480_usize;
    let frames = 4800;
    let mut gate = vec![0.0_f32; frames];
    gate[..gate_off].fill(1.0);
    let a = vec![0.001_f32; frames];
    let r = vec![release_samples as f32 / SR; frames];
    let mut out = vec![0.0_f32; frames];
    node.process(&[&gate, &a, &r], &mut [&mut out], frames);

    assert!(out[gate_off - 1] >= 1.0 - 1e-3);

    let expected_zero = gate_off + release_samples - 1;
    let first_zero = gate_off
        + out[gate_off..]
            .iter()
            .position(|&v| v <= 1e-4)
            .expect("release should reach zero");
    assert!(
        first_zero.abs_diff(expected_zero) <= 2,
        "release should reach zero near sample {expected_zero}, reached at {first_zero}"
    );
    assert!(out[first_zero..].iter().all(|&v| v <= 1e-4));
}

// ---------------------------------------------------------------------------
// PanNode
// ---------------------------------------------------------------------------

/// Pans a constant-1.0 signal at `position`, returning one (L, R) sample pair.
fn pan_gains(position: f32) -> (f32, f32) {
    let mut node = pan();
    let audio = vec![1.0_f32; 4];
    let pos = vec![position; 4];
    let mut left = vec![0.0_f32; 4];
    let mut right = vec![0.0_f32; 4];
    node.process(&[&audio, &pos], &mut [&mut left, &mut right], 4);
    (left[0], right[0])
}

#[test]
fn pan_node_channel_counts() {
    let node = pan();
    assert_eq!(node.inputs(), 2);
    assert_eq!(node.outputs(), 2);
}

#[test]
fn pan_center_is_minus_three_db_per_side() {
    let (l, r) = pan_gains(0.0);
    assert!((l - std::f32::consts::FRAC_1_SQRT_2).abs() < 1e-4);
    assert!((r - std::f32::consts::FRAC_1_SQRT_2).abs() < 1e-4);
}

#[test]
fn pan_hard_left_silences_right() {
    let (l, r) = pan_gains(-1.0);
    assert!((l - 1.0).abs() < 1e-6);
    assert!(r.abs() < 1e-6);
}

#[test]
fn pan_hard_right_silences_left() {
    let (l, r) = pan_gains(1.0);
    assert!(l.abs() < 1e-6);
    assert!((r - 1.0).abs() < 1e-6);
}

#[test]
fn pan_preserves_energy_across_positions() {
    for i in 0..=20_u16 {
        let position = f32::from(i).mul_add(0.1, -1.0);
        let (l, r) = pan_gains(position);
        let energy = l.mul_add(l, r * r);
        assert!(
            (energy - 1.0).abs() < 1e-4,
            "energy at position {position} was {energy}"
        );
    }
}

#[test]
fn pan_clamps_out_of_range_positions() {
    assert_eq!(pan_gains(-2.0), pan_gains(-1.0));
    assert_eq!(pan_gains(2.0), pan_gains(1.0));
}

#[test]
fn pan_scales_audio_linearly() {
    let mut node = pan();
    let audio = vec![0.5_f32; 4];
    let pos = vec![0.0_f32; 4];
    let mut left = vec![0.0_f32; 4];
    let mut right = vec![0.0_f32; 4];
    node.process(&[&audio, &pos], &mut [&mut left, &mut right], 4);
    let expected = 0.5 * std::f32::consts::FRAC_1_SQRT_2;
    assert!((left[0] - expected).abs() < 1e-4);
    assert!((right[0] - expected).abs() < 1e-4);
}
