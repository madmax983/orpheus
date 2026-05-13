//! Integration tests for `analog_filter`.
use orpheus_dsp::LadderFilter;

#[test]
fn ladder_filter_zero_input_stays_silent_after_reset() {
    let mut filter = LadderFilter::new(48_000.0);
    filter.reset();
    for _ in 0..256 {
        assert!(filter.process(0.0, 1_000.0, 0.2).abs() <= 1.0e-6);
    }
}

#[test]
fn ladder_filter_output_remains_finite_under_resonant_sweep() {
    let mut filter = LadderFilter::new(48_000.0);
    let mut cutoff = 200.0_f32;
    for _ in 0..512 {
        let output = filter.process(0.5, cutoff, 0.9);
        assert!(output.is_finite());
        cutoff += 10.0;
    }
}

#[test]
fn ladder_filter_reset_clears_prior_state() {
    let mut filter = LadderFilter::new(48_000.0);
    for _ in 0..64 {
        let _ = filter.process(1.0, 2_000.0, 0.7);
    }

    filter.reset();
    let first = filter.process(0.25, 800.0, 0.3);
    filter.reset();
    let second = filter.process(0.25, 800.0, 0.3);

    assert!((first - second).abs() <= 1.0e-6);
}

#[test]
fn ladder_filter_zero_cutoff_leaks_prior_state_toward_silence() {
    let mut filter = LadderFilter::new(48_000.0);
    for _ in 0..128 {
        let _ = filter.process(1.0, 2_000.0, 0.4);
    }

    let first = filter.process(0.0, 0.0, 0.0).abs();
    let mut last = first;
    for _ in 0..63 {
        last = filter.process(0.0, 0.0, 0.0).abs();
    }

    assert!(last < first);
}
