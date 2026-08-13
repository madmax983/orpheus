//! Tests for foundational mathematical DSP primitives used in building virtual analog circuits.

use orpheus_dsp::{Gain, LadderFilter, Mix, PhaseAccumulator, PulseOsc, SawOsc, SoftSat};

#[test]
fn mix_endpoints_return_the_original_signals() {
    assert!((Mix::blend(-0.5, 0.75, 0.0) + 0.5).abs() <= f32::EPSILON);
    assert!((Mix::blend(-0.5, 0.75, 1.0) - 0.75).abs() <= f32::EPSILON);
}

#[test]
fn gain_scales_and_reset_is_a_noop() {
    let mut gain = Gain::new();
    assert!((gain.process(0.5, 2.0) - 1.0).abs() <= f32::EPSILON);
    gain.reset();
    assert!((gain.process(0.25, 0.5) - 0.125).abs() <= f32::EPSILON);
}

#[test]
fn soft_sat_output_stays_finite_and_bounded() {
    let mut sat = SoftSat::new();
    let output = sat.process(100.0, 4.0);
    assert!(output.is_finite());
    assert!(output.abs() <= 1.0);
}

#[test]
fn phase_accumulator_wraps_into_the_unit_interval() {
    let mut phase = PhaseAccumulator::new();
    let first = phase.advance(1.25);
    let second = phase.advance(0.75);

    assert!((first - 0.25).abs() <= 1.0e-6);
    assert!((second - 0.0).abs() <= 1.0e-6);
    assert!((phase.phase() - 0.0).abs() <= 1.0e-6);
}

#[test]
fn subtractive_chain_produces_finite_audio() {
    let mut osc = SawOsc::new(48_000.0);
    let mut filter = LadderFilter::new(48_000.0);
    let mut sat = SoftSat::new();

    for _ in 0..1_024 {
        let sample = osc.next_sample(110.0);
        let filtered = filter.process(sample, 800.0, 0.4);
        let driven = sat.process(filtered, 1.5);
        assert!(driven.is_finite());
    }
}

#[test]
fn mixed_dual_oscillator_path_stays_finite() {
    let mut saw = SawOsc::new(48_000.0);
    let mut pulse = PulseOsc::new(48_000.0);
    let mut filter = LadderFilter::new(48_000.0);
    let mut mix = Mix::new();

    for _ in 0..1_024 {
        let summed = mix.process(saw.next_sample(220.0), pulse.next_sample(220.0, 0.4), 0.5);
        let filtered = filter.process(summed, 1_200.0, 0.25);
        assert!(filtered.is_finite());
    }
}

#[test]
fn reset_restores_deterministic_subtractive_output() {
    let mut first_osc = SawOsc::new(48_000.0);
    let mut first_filter = LadderFilter::new(48_000.0);
    let mut first_sat = SoftSat::new();
    let first = (0..128)
        .map(|_| {
            let sample = first_osc.next_sample(55.0);
            let filtered = first_filter.process(sample, 500.0, 0.55);
            first_sat.process(filtered, 2.0)
        })
        .collect::<Vec<_>>();

    first_osc.reset();
    first_filter.reset();
    first_sat.reset();

    let second = (0..128)
        .map(|_| {
            let sample = first_osc.next_sample(55.0);
            let filtered = first_filter.process(sample, 500.0, 0.55);
            first_sat.process(filtered, 2.0)
        })
        .collect::<Vec<_>>();

    assert_eq!(first, second);
}
