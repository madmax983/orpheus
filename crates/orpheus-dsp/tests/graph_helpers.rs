//! Tests for pipe and bind helpers.

use orpheus_dsp::{
    GraphError, Node, Processor, bind, constant, gain_node, ladder_filter, par, passthrough, pipe,
    saw, sine, soft_sat, sum,
};

const SR: f32 = 48_000.0;
const FRAMES: usize = 256;

// ---------------------------------------------------------------------------
// pipe
// ---------------------------------------------------------------------------

#[test]
fn pipe_equal_channels_is_seq() {
    // constant(5.0) has 0 in, 1 out. passthrough(1) has 1 in, 1 out.
    // pipe should work like seq.
    let mut p = pipe(constant(5.0), passthrough(1)).unwrap();
    assert_eq!(p.inputs(), 0);
    assert_eq!(p.outputs(), 1);

    let mut out = vec![0.0_f32; FRAMES];
    p.process(&[], &mut [&mut out], FRAMES);
    assert!(out.iter().all(|&s| (s - 5.0).abs() < f32::EPSILON));
}

#[test]
fn pipe_threads_extra_inputs() {
    // saw(SR) has 1 in (freq), 1 out (audio).
    // ladder_filter(SR) has 3 in (audio, cutoff, res), 1 out.
    // pipe(saw, ladder_filter): 1 + 3 - 1 = 3 inputs [freq, cutoff, res], 1 output.
    let p = pipe(saw(SR), ladder_filter(SR)).unwrap();
    assert_eq!(p.inputs(), 3);
    assert_eq!(p.outputs(), 1);
}

#[test]
fn pipe_chain_builds_subtractive_synth() {
    // saw → ladder_filter → soft_sat → gain_node
    let s1 = pipe(saw(SR), ladder_filter(SR)).unwrap(); // 3 in, 1 out
    let s2 = pipe(s1, soft_sat()).unwrap(); // 4 in, 1 out
    let s3 = pipe(s2, gain_node()).unwrap(); // 5 in, 1 out

    assert_eq!(s3.inputs(), 5); // [freq, cutoff, res, drive, gain]
    assert_eq!(s3.outputs(), 1);

    let mut proc = Processor::new(s3);
    let freq = vec![220.0_f32; FRAMES];
    let cutoff = vec![2000.0_f32; FRAMES];
    let res = vec![0.3_f32; FRAMES];
    let drive = vec![1.5_f32; FRAMES];
    let gain = vec![0.8_f32; FRAMES];
    let mut out = vec![0.0_f32; FRAMES];

    proc.process(
        &[&freq, &cutoff, &res, &drive, &gain],
        &mut [&mut out],
        FRAMES,
    );

    assert!(out.iter().all(|s| s.is_finite()));
    assert!(out.iter().any(|&s| s.abs() > 0.001));
}

#[test]
fn pipe_rejects_too_many_outputs() {
    // par(constant(1), constant(2)) has 0 in, 2 out.
    // passthrough(1) has 1 in, 1 out.
    // 2 outputs > 1 input: should fail.
    let result = pipe(par(constant(1.0), constant(2.0)), passthrough(1));
    assert!(matches!(result, Err(GraphError::ChannelMismatch { .. })));
}

// ---------------------------------------------------------------------------
// bind
// ---------------------------------------------------------------------------

#[test]
fn bind_all_inputs_produces_source() {
    // saw(SR) has 1 input (freq). Bind it to 440.
    let mut b = bind(saw(SR), &[(0, 440.0)]).unwrap();
    assert_eq!(b.inputs(), 0);
    assert_eq!(b.outputs(), 1);

    let mut out = vec![0.0_f32; FRAMES];
    b.process(&[], &mut [&mut out], FRAMES);

    assert!(out.iter().all(|s| s.is_finite()));
    assert!(out.iter().any(|&s| s.abs() > 0.01));
}

#[test]
fn bind_partial_leaves_free_channels() {
    // ladder_filter(SR) has 3 inputs (audio, cutoff, res).
    // Bind cutoff(1) and res(2), leaving audio(0) as external.
    let b = bind(ladder_filter(SR), &[(1, 2000.0), (2, 0.3)]).unwrap();
    assert_eq!(b.inputs(), 1); // just audio
    assert_eq!(b.outputs(), 1);
}

#[test]
fn bind_partial_processes_correctly() {
    // Bind cutoff and res on ladder_filter, pass audio through.
    let mut b = bind(ladder_filter(SR), &[(1, 2000.0), (2, 0.3)]).unwrap();

    // Feed a constant 0.5 as audio.
    let audio = vec![0.5_f32; FRAMES];
    let mut out = vec![0.0_f32; FRAMES];
    b.process(&[&audio], &mut [&mut out], FRAMES);

    assert!(out.iter().all(|s| s.is_finite()));
    assert!(out.iter().any(|&s| s.abs() > 0.001));
}

#[test]
fn bind_rejects_out_of_range_channel() {
    // saw(SR) has 1 input. Trying to bind channel 5 should fail.
    let result = bind(saw(SR), &[(5, 440.0)]);
    assert!(matches!(result, Err(GraphError::ChannelMismatch { .. })));
}

#[test]
fn bind_empty_bindings_is_passthrough() {
    // Binding nothing should leave all inputs intact.
    let b = bind(passthrough(3), &[]).unwrap();
    assert_eq!(b.inputs(), 3);
    assert_eq!(b.outputs(), 3);
}

#[test]
fn bind_renumbers_free_channels_contiguously() {
    // sum(4) has 4 inputs. Bind channels 1 and 3.
    // Free channels: [0, 2] → renumbered as external [0, 1].
    let mut b = bind(sum(4), &[(1, 10.0), (3, 30.0)]).unwrap();
    assert_eq!(b.inputs(), 2);

    // External input 0 → inner channel 0, external input 1 → inner channel 2.
    // Inner sum = ext[0] + 10 + ext[1] + 30.
    let ext0 = vec![1.0_f32; FRAMES];
    let ext1 = vec![2.0_f32; FRAMES];
    let mut out = vec![0.0_f32; FRAMES];

    b.process(&[&ext0, &ext1], &mut [&mut out], FRAMES);

    // 1 + 10 + 2 + 30 = 43
    assert!(out.iter().all(|&s| (s - 43.0).abs() < f32::EPSILON));
}

// ---------------------------------------------------------------------------
// pipe + bind together
// ---------------------------------------------------------------------------

#[test]
fn pipe_then_bind_builds_fixed_subtractive_synth() {
    let chain = pipe(saw(SR), ladder_filter(SR)).unwrap();
    let chain = pipe(chain, soft_sat()).unwrap();
    let chain = pipe(chain, gain_node()).unwrap();

    // Bind all params except freq (channel 0).
    let mut synth = bind(chain, &[(1, 2000.0), (2, 0.3), (3, 1.5), (4, 0.8)]).unwrap();
    assert_eq!(synth.inputs(), 1); // just freq
    assert_eq!(synth.outputs(), 1);

    let freq = vec![220.0_f32; FRAMES];
    let mut out = vec![0.0_f32; FRAMES];
    synth.process(&[&freq], &mut [&mut out], FRAMES);

    assert!(out.iter().all(|s| s.is_finite()));
    assert!(out.iter().any(|&s| s.abs() > 0.001));
}

#[test]
fn pipe_then_bind_all_makes_zero_input_synth() {
    let chain = pipe(saw(SR), ladder_filter(SR)).unwrap();
    let chain = pipe(chain, soft_sat()).unwrap();
    let chain = pipe(chain, gain_node()).unwrap();

    let mut synth = bind(
        chain,
        &[(0, 220.0), (1, 2000.0), (2, 0.3), (3, 1.5), (4, 0.8)],
    )
    .unwrap();
    assert_eq!(synth.inputs(), 0);

    let mut out = vec![0.0_f32; FRAMES];
    synth.process(&[], &mut [&mut out], FRAMES);

    assert!(out.iter().all(|s| s.is_finite()));
    assert!(out.iter().any(|&s| s.abs() > 0.001));
}

#[test]
fn bind_reset_preserves_determinism() {
    let chain = pipe(constant(440.0), sine(SR)).unwrap();
    let mut synth = bind(chain, &[]).unwrap();

    let mut out1 = vec![0.0_f32; FRAMES];
    synth.process(&[], &mut [&mut out1], FRAMES);
    synth.reset();
    let mut out2 = vec![0.0_f32; FRAMES];
    synth.process(&[], &mut [&mut out2], FRAMES);

    assert_eq!(out1, out2);
}
