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

#[test]
fn ladder_filter_handles_non_finite_inputs_gracefully() {
    let mut filter = LadderFilter::new(48000.0);
    // Non-finite drive
    let out = filter.process(f32::NAN, 1000.0, 0.5);
    // Nan drive -> 0.0 drive. filter will just settle.
    assert!(out.is_finite());

    // Non-finite cutoff
    let out = filter.process(1.0, f32::INFINITY, 0.5);
    assert!(out.is_finite());

    // Non-finite resonance
    let out = filter.process(1.0, 1000.0, f32::NAN);
    assert!(out.is_finite());
}

#[test]
fn ladder_filter_zero_cutoff_decays() {
    let mut filter = LadderFilter::new(48000.0);
    filter.process(1.0, 1000.0, 0.5); // Push some signal into it
    let state_before = filter.process(0.0, 1000.0, 0.5);

    // Now push 0.0 cutoff, it should decay towards 0
    let state_after = filter.process(0.0, 0.0, 0.5);
    // Should be strictly smaller in magnitude, unless already zero
    assert!(state_after.abs() <= state_before.abs());
}
