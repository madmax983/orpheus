//! Tests for the Processor wrapper.

use orpheus_dsp::graph::{Node, Processor, constant, passthrough, seq};

const FRAMES: usize = 64;

#[test]
fn processor_wraps_simple_node() {
    let mut proc = Processor::new(constant(5.0));
    assert_eq!(proc.inputs(), 0);
    assert_eq!(proc.outputs(), 1);

    let mut out = vec![0.0_f32; FRAMES];
    proc.process(&[], &mut [&mut out], FRAMES);

    assert!(out.iter().all(|&s| (s - 5.0).abs() < f32::EPSILON));
}

#[test]
fn processor_wraps_combinator_chain() {
    let chain = seq(constant(42.0), passthrough(1)).unwrap();
    let mut proc = Processor::new(chain);

    let mut out = vec![0.0_f32; FRAMES];
    proc.process(&[], &mut [&mut out], FRAMES);

    assert!(out.iter().all(|&s| (s - 42.0).abs() < f32::EPSILON));
}

#[test]
fn processor_reset_deterministic() {
    let chain = seq(constant(1.0), passthrough(1)).unwrap();
    let mut proc = Processor::new(chain);

    let mut out1 = vec![0.0_f32; FRAMES];
    proc.process(&[], &mut [&mut out1], FRAMES);

    proc.reset();

    let mut out2 = vec![0.0_f32; FRAMES];
    proc.process(&[], &mut [&mut out2], FRAMES);

    assert_eq!(out1, out2);
}

#[test]
fn processor_handles_zero_frames() {
    let mut proc = Processor::new(constant(1.0));
    let mut out = vec![999.0_f32; FRAMES];

    // Process 0 frames — should not touch any output.
    proc.process(&[], &mut [&mut out], 0);

    assert!(out.iter().all(|&s| (s - 999.0).abs() < f32::EPSILON));
}

#[test]
fn processor_debug_shows_channel_counts() {
    let proc = Processor::new(passthrough(3));
    let dbg = format!("{proc:?}");
    assert!(dbg.contains("Processor"));
    assert!(dbg.contains('3'));
}
