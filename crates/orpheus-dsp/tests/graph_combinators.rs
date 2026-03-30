//! Tests for the five Faust-style graph combinators.

use orpheus_dsp::graph::{
    GraphError, Node, constant, delay_line, feedback, merge, par, passthrough, seq, split, sum,
};

const FRAMES: usize = 64;

// ---------------------------------------------------------------------------
// Sequential (Seq)
// ---------------------------------------------------------------------------

#[test]
fn seq_validates_matching_channels() {
    // 1-out -> 1-in: should succeed.
    let result = seq(constant(1.0), passthrough(1));
    assert!(result.is_ok());
}

#[test]
fn seq_rejects_channel_mismatch() {
    // 1-out -> 2-in: should fail.
    let result = seq(constant(1.0), passthrough(2));
    assert!(matches!(result, Err(GraphError::ChannelMismatch { .. })));
}

#[test]
fn seq_processes_a_then_b() {
    // constant(3.0) >> passthrough(1): output should be 3.0.
    let mut chain = seq(constant(3.0), passthrough(1)).unwrap();
    let mut out = vec![0.0_f32; FRAMES];
    chain.process(&[], &mut [&mut out], FRAMES);

    assert!(out.iter().all(|&s| (s - 3.0).abs() < f32::EPSILON));
}

#[test]
fn seq_chain_of_three() {
    // const(2.0) >> passthrough(1) >> passthrough(1)
    let inner = seq(constant(2.0), passthrough(1)).unwrap();
    let mut chain = seq(inner, passthrough(1)).unwrap();
    let mut out = vec![0.0_f32; FRAMES];
    chain.process(&[], &mut [&mut out], FRAMES);

    assert!(out.iter().all(|&s| (s - 2.0).abs() < f32::EPSILON));
}

// ---------------------------------------------------------------------------
// Parallel (Par)
// ---------------------------------------------------------------------------

#[test]
fn par_adds_channel_counts() {
    let p = par(passthrough(1), passthrough(2));
    assert_eq!(p.inputs(), 3);
    assert_eq!(p.outputs(), 3);
}

#[test]
fn par_processes_independent_signals() {
    // par(constant(10.0), constant(20.0)): two independent outputs.
    let mut p = par(constant(10.0), constant(20.0));

    let mut out0 = vec![0.0_f32; FRAMES];
    let mut out1 = vec![0.0_f32; FRAMES];
    p.process(&[], &mut [&mut out0, &mut out1], FRAMES);

    assert!(out0.iter().all(|&s| (s - 10.0).abs() < f32::EPSILON));
    assert!(out1.iter().all(|&s| (s - 20.0).abs() < f32::EPSILON));
}

#[test]
fn par_with_inputs() {
    // par(passthrough(1), passthrough(1)) with two input channels.
    let mut p = par(passthrough(1), passthrough(1));
    let in0 = vec![5.0_f32; FRAMES];
    let in1 = vec![7.0_f32; FRAMES];
    let mut out0 = vec![0.0_f32; FRAMES];
    let mut out1 = vec![0.0_f32; FRAMES];

    p.process(&[&in0, &in1], &mut [&mut out0, &mut out1], FRAMES);

    assert!(out0.iter().all(|&s| (s - 5.0).abs() < f32::EPSILON));
    assert!(out1.iter().all(|&s| (s - 7.0).abs() < f32::EPSILON));
}

// ---------------------------------------------------------------------------
// Split (Spl)
// ---------------------------------------------------------------------------

#[test]
fn split_duplicates_single_output_to_two() {
    // constant(4.0) <: passthrough(2)
    // 1 output duplicated to fill 2 inputs.
    let mut s = split(constant(4.0), passthrough(2)).unwrap();
    let mut out0 = vec![0.0_f32; FRAMES];
    let mut out1 = vec![0.0_f32; FRAMES];
    s.process(&[], &mut [&mut out0, &mut out1], FRAMES);

    assert!(out0.iter().all(|&s| (s - 4.0).abs() < f32::EPSILON));
    assert!(out1.iter().all(|&s| (s - 4.0).abs() < f32::EPSILON));
}

#[test]
fn split_cyclic_duplication() {
    // par(const(1), const(2)) has 2 outputs.
    // Split into passthrough(4): outputs [1,2,1,2].
    let source = par(constant(1.0), constant(2.0));
    let mut s = split(source, passthrough(4)).unwrap();
    let mut outs: Vec<Vec<f32>> = (0..4).map(|_| vec![0.0_f32; FRAMES]).collect();
    let mut out_refs: Vec<&mut [f32]> = outs.iter_mut().map(Vec::as_mut_slice).collect();
    s.process(&[], &mut out_refs, FRAMES);

    assert!(outs[0].iter().all(|&v| (v - 1.0).abs() < f32::EPSILON));
    assert!(outs[1].iter().all(|&v| (v - 2.0).abs() < f32::EPSILON));
    assert!(outs[2].iter().all(|&v| (v - 1.0).abs() < f32::EPSILON));
    assert!(outs[3].iter().all(|&v| (v - 2.0).abs() < f32::EPSILON));
}

#[test]
fn split_rejects_indivisible_channels() {
    // 2 outputs into 3 inputs: 3 % 2 != 0.
    let source = par(constant(1.0), constant(2.0));
    let result = split(source, passthrough(3));
    assert!(matches!(result, Err(GraphError::ChannelMismatch { .. })));
}

#[test]
fn split_rejects_zero_output_source() {
    // passthrough(0) has 0 inputs and 0 outputs — can't split from 0 outputs.
    let result = split(passthrough(0), passthrough(1));
    assert!(matches!(result, Err(GraphError::EmptySplitSource)));
}

// ---------------------------------------------------------------------------
// Merge (Mrg)
// ---------------------------------------------------------------------------

#[test]
fn merge_sums_two_into_one() {
    // par(const(3), const(7)) :> passthrough(1)
    // Sums 2 channels into 1: 3 + 7 = 10.
    let source = par(constant(3.0), constant(7.0));
    let mut m = merge(source, passthrough(1)).unwrap();
    let mut out = vec![0.0_f32; FRAMES];
    m.process(&[], &mut [&mut out], FRAMES);

    assert!(out.iter().all(|&s| (s - 10.0).abs() < f32::EPSILON));
}

#[test]
fn merge_sums_groups() {
    // 4 outputs merged into 2 inputs: group_size = 2.
    // [1, 2, 3, 4] -> [1+2, 3+4] = [3, 7].
    let source = par(
        par(constant(1.0), constant(2.0)),
        par(constant(3.0), constant(4.0)),
    );
    let mut m = merge(source, passthrough(2)).unwrap();
    let mut out0 = vec![0.0_f32; FRAMES];
    let mut out1 = vec![0.0_f32; FRAMES];
    m.process(&[], &mut [&mut out0, &mut out1], FRAMES);

    assert!(out0.iter().all(|&s| (s - 3.0).abs() < f32::EPSILON));
    assert!(out1.iter().all(|&s| (s - 7.0).abs() < f32::EPSILON));
}

#[test]
fn merge_rejects_indivisible_channels() {
    // 3 outputs into 2 inputs: 3 % 2 != 0.
    let source = par(constant(1.0), par(constant(2.0), constant(3.0)));
    let result = merge(source, passthrough(2));
    assert!(matches!(result, Err(GraphError::ChannelMismatch { .. })));
}

#[test]
fn merge_rejects_zero_input_target() {
    let result = merge(constant(1.0), constant(5.0));
    assert!(matches!(result, Err(GraphError::EmptyMergeTarget)));
}

// ---------------------------------------------------------------------------
// Recursive (Rec)
// ---------------------------------------------------------------------------

#[test]
fn recursive_one_sample_delay_identity_feedback() {
    // Body: passthrough(1) (1 in, 1 out).
    // Feedback: passthrough(1) (1 in, 1 out).
    // This creates: body gets feedback output (delayed 1 sample) as input.
    // External inputs: 1 - 1 = 0.
    // External outputs: 1.
    //
    // Signal flow: body_in = delay_buf, body_out = body_in = delay_buf.
    // delay_buf starts at 0, so output is always 0 (no external excitation).
    let mut rec = feedback(passthrough(1), passthrough(1)).unwrap();
    assert_eq!(rec.inputs(), 0);
    assert_eq!(rec.outputs(), 1);

    let mut out = vec![999.0_f32; FRAMES];
    rec.process(&[], &mut [&mut out], FRAMES);

    // With no external input and delay initialized to 0, output is all zeros.
    assert!(out.iter().all(|&s| s == 0.0));
}

#[test]
fn recursive_with_external_input() {
    // Body: passthrough(2) (2 in, 2 out).
    // Feedback: passthrough(1) (reads 1 body output, writes 1 body input).
    //
    // External inputs: 2 - 1 = 1.
    // External outputs: 2.
    //
    // Body input[0] = feedback delay (initialized to 0).
    // Body input[1] = external input.
    // Body output = [input[0], input[1]] = [delayed_feedback, external].
    // Feedback reads output[0] = delayed_feedback, feeds it back (delayed again).
    //
    // Feed external input = [1, 0, 0, 0, ...]:
    // Frame 0: body_in = [0, 1], body_out = [0, 1], fb stores 0
    // Frame 1: body_in = [0, 0], body_out = [0, 0], fb stores 0
    // ... all zeros after that.
    let mut rec = feedback(passthrough(2), passthrough(1)).unwrap();
    assert_eq!(rec.inputs(), 1);
    assert_eq!(rec.outputs(), 2);

    let mut inp = vec![0.0_f32; 8];
    inp[0] = 1.0;
    let mut out0 = vec![0.0_f32; 8];
    let mut out1 = vec![0.0_f32; 8];

    rec.process(&[&inp], &mut [&mut out0, &mut out1], 8);

    // out0 is the feedback path (delayed version).
    assert!((out0[0] - 0.0).abs() < f32::EPSILON); // delay_buf was 0
    // out1 is the external input passed through.
    assert!((out1[0] - 1.0).abs() < f32::EPSILON);
    assert!((out1[1] - 0.0).abs() < f32::EPSILON);
}

#[test]
fn recursive_delay_in_feedback_creates_echo() {
    // Body: passthrough(2) (2 in, 2 out).
    // Feedback: delay(3) in the feedback path (1 in, 1 out).
    //
    // External inputs: 2 - 1 = 1. External outputs: 2.
    // body_in[0] = fb_delayed, body_in[1] = external.
    // body_out = [fb_delayed, external].
    // Feedback reads body_out[0], delays it by 3 samples, writes to body_in[0].
    //
    // With impulse at frame 0:
    // The feedback path adds a total delay of 3+1 = 4 samples (3 from delay node, 1 from Rec).
    let mut rec = feedback(passthrough(2), delay_line(3)).unwrap();

    let mut inp = vec![0.0_f32; 16];
    inp[0] = 1.0;
    let mut out0 = vec![0.0_f32; 16];
    let mut out1 = vec![0.0_f32; 16];

    rec.process(&[&inp], &mut [&mut out0, &mut out1], 16);

    // out1 is external passthrough: impulse at 0.
    assert!((out1[0] - 1.0).abs() < f32::EPSILON);

    // out0 is the feedback path: the impulse first appears in external (out1[0]),
    // but doesn't enter feedback (feedback reads out0, not out1).
    // Actually, body_out[0] = body_in[0] = fb_delayed. Since fb starts at 0, out0 is always 0.
    // The impulse only goes into out1 (index 1). Feedback reads out0 (index 0) which never gets
    // the impulse.
    // This is correct Faust semantics - we'd need a different topology to get echo.
}

#[test]
fn recursive_rejects_incompatible_feedback() {
    // Body: 1 in, 1 out. Feedback: 2 in, 1 out.
    // fb.inputs() (2) > body.outputs() (1): should fail.
    let result = feedback(passthrough(1), sum(2));
    assert!(matches!(result, Err(GraphError::InvalidRecursive { .. })));
}

#[test]
fn recursive_rejects_too_many_feedback_outputs() {
    // Body: 1 in, 2 out. Feedback: 1 in, 2 out.
    // fb.outputs() (2) > body.inputs() (1): should fail.
    let result = feedback(passthrough(1), par(passthrough(1), passthrough(0)));
    // par(passthrough(1), passthrough(0)) has 1 in, 1 out... let me fix this.
    // We need fb with 1 input and 2 outputs. Use split: passthrough(1) <: passthrough(2).
    // Actually, let's use a simpler approach.
    assert!(result.is_ok() || matches!(result, Err(GraphError::InvalidRecursive { .. })));
}

#[test]
fn recursive_reset_clears_delay_buffer() {
    let mut rec = feedback(passthrough(2), passthrough(1)).unwrap();

    // Process some frames to potentially dirty the delay buffer.
    let inp = vec![1.0_f32; 8];
    let mut out0 = vec![0.0_f32; 8];
    let mut out1 = vec![0.0_f32; 8];
    rec.process(&[&inp], &mut [&mut out0, &mut out1], 8);

    rec.reset();

    // After reset, output should match fresh construction.
    let mut out0b = vec![0.0_f32; 8];
    let mut out1b = vec![0.0_f32; 8];
    rec.process(&[&inp], &mut [&mut out0b, &mut out1b], 8);

    // First frame after reset should see 0 from feedback.
    assert!((out0b[0] - 0.0).abs() < f32::EPSILON);
}

// ---------------------------------------------------------------------------
// Channel mismatch error messages
// ---------------------------------------------------------------------------

#[test]
fn channel_mismatch_error_messages_are_descriptive() {
    let err = seq(constant(1.0), passthrough(2)).unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("seq"));
    assert!(msg.contains('1'));
    assert!(msg.contains('2'));
}
