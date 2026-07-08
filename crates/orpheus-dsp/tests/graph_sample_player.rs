//! Integration tests for the sample-playback graph node (parity roadmap:
//! sample-playback node; ADR 0004 follow-up).
//!
//! `SamplePlayerNode` plays a preloaded, `Arc`-shared sample-bank buffer as a
//! composable graph node: 2 inputs (gate, rate) -> 1 output (the bank stores
//! mono buffers). A rising gate edge restarts playback from the top; the gate
//! level is otherwise ignored (one-shot, matching the engine's sample
//! voices). The rate input is a signal read every frame; fractional playhead
//! positions read with linear interpolation, and playback ends at the buffer
//! end (no looping).

// Assertions spell out `value - expected * scale` for readability; `mul_add`
// would only obscure the arithmetic under test. Exact float equality is
// deliberate where playback is bit-exact (whole-sample reads copy the
// buffer verbatim).
#![allow(clippy::suboptimal_flops, clippy::float_cmp)]

use orpheus_dsp::{
    GraphVoiceSpec, Node, PlaybackSample, VoiceNodeSpec, VoiceSignalRef, sample_player,
};

const SR: f32 = 48_000.0;

/// Builds a player over `frames` recorded at the node's own output rate, so
/// a rate of 1.0 advances exactly one source sample per output frame.
fn native_player(frames: &[f32]) -> orpheus_dsp::SamplePlayerNode {
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let sample = PlaybackSample::from_mono_frames(frames.to_vec(), SR as u32);
    sample_player(&sample, SR)
}

/// Runs the node over per-frame gate and rate signals, returning the output.
fn run(node: &mut orpheus_dsp::SamplePlayerNode, gate: &[f32], rate: &[f32]) -> Vec<f32> {
    assert_eq!(gate.len(), rate.len());
    let mut out = vec![0.0_f32; gate.len()];
    node.process(&[gate, rate], &mut [&mut out], gate.len());
    out
}

#[test]
fn sample_player_declares_gate_and_rate_inputs_and_one_output() {
    let node = native_player(&[0.5]);
    assert_eq!(node.inputs(), 2, "inputs are (gate, rate)");
    assert_eq!(node.outputs(), 1, "bank buffers are mono");
}

#[test]
fn unit_rate_plays_the_buffer_exactly_then_goes_silent() {
    let buffer = [0.1_f32, -0.2, 0.3, -0.4, 0.5];
    let mut node = native_player(&buffer);

    let gate = [1.0_f32; 8];
    let rate = [1.0_f32; 8];
    let out = run(&mut node, &gate, &rate);

    assert_eq!(&out[..5], &buffer, "rate 1.0 must reproduce the buffer");
    assert_eq!(
        &out[5..],
        &[0.0, 0.0, 0.0],
        "playback ends at the buffer end"
    );
}

#[test]
fn double_rate_halves_the_duration() {
    #[allow(clippy::cast_precision_loss)]
    let buffer: Vec<f32> = (0..8).map(|i| i as f32 * 0.1).collect();
    let mut node = native_player(&buffer);

    let gate = [1.0_f32; 8];
    let rate = [2.0_f32; 8];
    let out = run(&mut node, &gate, &rate);

    // Positions 0, 2, 4, 6 land on whole samples; position 8 is past the end.
    assert_eq!(&out[..4], &[0.0, 0.2, 0.4, 0.6]);
    assert!(
        out[4..].iter().all(|&s| s == 0.0),
        "rate 2.0 must finish in half the frames, got {out:?}"
    );
}

#[test]
fn fractional_positions_read_with_linear_interpolation() {
    let buffer = [0.0_f32, 1.0, 0.0];
    let mut node = native_player(&buffer);

    let gate = [1.0_f32; 7];
    let rate = [0.5_f32; 7];
    let out = run(&mut node, &gate, &rate);

    // Positions 0, 0.5, 1.0, 1.5, 2.0, 2.5 (interpolating toward silence
    // past the final sample), then past the end.
    let expected = [0.0_f32, 0.5, 1.0, 0.5, 0.0, 0.0, 0.0];
    for (i, (&got, &want)) in out.iter().zip(expected.iter()).enumerate() {
        assert!(
            (got - want).abs() < 1e-6,
            "frame {i}: expected {want}, got {got} (all: {out:?})"
        );
    }
}

#[test]
fn source_rate_conversion_scales_the_playhead_step() {
    // A buffer recorded at half the output rate advances half a source
    // sample per output frame at rate 1.0.
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let sample = PlaybackSample::from_mono_frames(vec![0.0_f32, 1.0], (SR / 2.0) as u32);
    let mut node = sample_player(&sample, SR);

    let gate = [1.0_f32; 5];
    let rate = [1.0_f32; 5];
    let out = run(&mut node, &gate, &rate);

    // Positions 0, 0.5, 1.0, 1.5 (toward silence), 2.0 (done).
    let expected = [0.0_f32, 0.5, 1.0, 0.5, 0.0];
    for (i, (&got, &want)) in out.iter().zip(expected.iter()).enumerate() {
        assert!(
            (got - want).abs() < 1e-6,
            "frame {i}: expected {want}, got {got} (all: {out:?})"
        );
    }
}

#[test]
fn rising_gate_edge_restarts_playback_from_the_top() {
    let buffer = [0.5_f32, 0.25, 0.125, 0.0625];
    let mut node = native_player(&buffer);

    let gate = [1.0_f32, 1.0, 0.0, 1.0, 1.0, 1.0, 1.0];
    let rate = [1.0_f32; 7];
    let out = run(&mut node, &gate, &rate);

    // Frames 0-2 play from the top (the falling gate does not stop the
    // one-shot); frame 3's rising edge restarts from the top.
    assert_eq!(&out[..3], &buffer[..3]);
    assert_eq!(&out[3..], &buffer, "retrigger must restart from sample 0");
}

#[test]
fn gate_falling_mid_sample_plays_to_the_end() {
    let buffer = [0.1_f32, 0.2, 0.3, 0.4, 0.5];
    let mut node = native_player(&buffer);

    let mut gate = [0.0_f32; 7];
    gate[0] = 1.0;
    let rate = [1.0_f32; 7];
    let out = run(&mut node, &gate, &rate);

    assert_eq!(
        &out[..5],
        &buffer,
        "one-shot playback must survive a falling gate"
    );
    assert_eq!(&out[5..], &[0.0, 0.0]);
}

#[test]
fn holding_the_gate_high_after_the_end_does_not_loop() {
    let buffer = [0.5_f32, 0.5];
    let mut node = native_player(&buffer);

    let gate = [1.0_f32; 6];
    let rate = [1.0_f32; 6];
    let out = run(&mut node, &gate, &rate);

    assert_eq!(&out[..2], &buffer);
    assert!(
        out[2..].iter().all(|&s| s == 0.0),
        "no looping: a held gate must not replay the buffer, got {out:?}"
    );
}

#[test]
fn silent_before_the_first_gate_edge() {
    let buffer = [0.5_f32, 0.5];
    let mut node = native_player(&buffer);

    let gate = [0.0_f32; 4];
    let rate = [1.0_f32; 4];
    let out = run(&mut node, &gate, &rate);
    assert!(out.iter().all(|&s| s == 0.0));
}

#[test]
fn reset_returns_the_player_to_idle() {
    let buffer = [0.9_f32, 0.8, 0.7];
    let mut node = native_player(&buffer);

    let _ = run(&mut node, &[1.0, 1.0], &[1.0, 1.0]);
    node.reset();

    let out = run(&mut node, &[0.0, 1.0, 1.0], &[1.0, 1.0, 1.0]);
    assert_eq!(out[0], 0.0, "reset must return to idle (no sounding tail)");
    assert_eq!(
        &out[1..],
        &buffer[..2],
        "post-reset trigger replays the top"
    );
}

#[test]
fn non_positive_or_non_finite_rates_hold_the_playhead() {
    let buffer = [0.1_f32, 0.2, 0.3];
    let mut node = native_player(&buffer);

    let gate = [1.0_f32; 6];
    let rate = [1.0_f32, 0.0, -1.0, f32::NAN, 1.0, 1.0];
    let out = run(&mut node, &gate, &rate);

    // Frame 0 plays sample 0 and advances; frames 1-4 hold at sample 1 (the
    // read happens before the step, so the resuming frame still reads the
    // held sample); frame 5 reaches sample 2.
    assert_eq!(&out[..5], &[0.1, 0.2, 0.2, 0.2, 0.2]);
    assert_eq!(out[5], 0.3, "a positive rate resumes the playhead");
}

// ---------------------------------------------------------------------------
// Voice-spec integration: `VoiceNodeSpec::Sample` lowers onto the node.
// ---------------------------------------------------------------------------

/// A voice spec that is nothing but the sample, gated by the note gate.
fn bare_sample_spec(buffer: &[f32]) -> GraphVoiceSpec {
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let sample = PlaybackSample::from_mono_frames(buffer.to_vec(), SR as u32);
    GraphVoiceSpec::new(
        "hit",
        0.5,
        vec![
            VoiceNodeSpec::Constant { value: 1.0 },
            VoiceNodeSpec::Sample {
                gate: VoiceSignalRef::Gate,
                rate: VoiceSignalRef::Node(0),
                sample,
            },
        ],
        VoiceSignalRef::Node(1),
    )
    .expect("sample voice spec should validate")
}

#[test]
fn voice_spec_sample_node_plays_the_buffer_through_the_voice_interface() {
    let buffer = [0.1_f32, -0.2, 0.3, -0.4];
    let spec = bare_sample_spec(&buffer);
    let mut voice = spec.build_voice(SR);
    voice.prepare();

    // Gain 1, pan 0: the equal-power panner scales both sides by cos(pi/4).
    let center = std::f32::consts::FRAC_1_SQRT_2;
    for (i, &expected) in buffer.iter().enumerate() {
        let (left, right) = voice.process_frame(1.0, 220.0, 1.0, 0.0);
        assert!(
            (left - expected * center).abs() < 1e-6,
            "frame {i}: left {left} vs expected {}",
            expected * center
        );
        assert!(
            (right - expected * center).abs() < 1e-6,
            "frame {i}: right {right}"
        );
    }
    let (left, right) = voice.process_frame(1.0, 220.0, 1.0, 0.0);
    assert_eq!((left, right), (0.0, 0.0), "silent after the buffer end");
}

#[test]
fn voice_spec_sample_survives_gate_fall_and_retriggers_on_the_next_note() {
    let buffer = [0.4_f32, 0.3, 0.2, 0.1];
    let spec = bare_sample_spec(&buffer);
    let mut voice = spec.build_voice(SR);
    voice.prepare();

    // One-frame gate: the one-shot still plays out fully.
    let center = std::f32::consts::FRAC_1_SQRT_2;
    let (first, _) = voice.process_frame(1.0, 220.0, 1.0, 0.0);
    assert!((first - buffer[0] * center).abs() < 1e-6);
    for &expected in &buffer[1..] {
        let (left, _) = voice.process_frame(0.0, 220.0, 1.0, 0.0);
        assert!(
            (left - expected * center).abs() < 1e-6,
            "one-shot must continue after the gate falls"
        );
    }

    // A new gate edge restarts from the top.
    let (retriggered, _) = voice.process_frame(1.0, 220.0, 1.0, 0.0);
    assert!((retriggered - buffer[0] * center).abs() < 1e-6);
}
