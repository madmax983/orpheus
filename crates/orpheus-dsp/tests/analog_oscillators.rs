//! Integration tests for individual analog oscillator waveforms (saw, square, triangle, sine).

use orpheus_dsp::{Noise, PulseOsc, SawOsc, TriOsc};

fn naive_saw_step(sample_rate_hz: f32, freq_hz: f32, sample_count: usize) -> f32 {
    let mut phase = 0.0_f32;
    let step = freq_hz / sample_rate_hz;
    let mut previous = 2.0_f32.mul_add(phase, -1.0);
    let mut worst = 0.0_f32;

    for _ in 0..sample_count {
        phase = (phase + step).rem_euclid(1.0);
        let current = 2.0_f32.mul_add(phase, -1.0);
        worst = worst.max((current - previous).abs());
        previous = current;
    }

    worst
}

#[test]
fn saw_osc_reset_is_deterministic() {
    let mut osc = SawOsc::new(48_000.0);
    let first = (0..64).map(|_| osc.next_sample(440.0)).collect::<Vec<_>>();
    osc.reset();
    let second = (0..64).map(|_| osc.next_sample(440.0)).collect::<Vec<_>>();
    assert_eq!(first, second);
}

#[test]
fn pulse_osc_outputs_remain_finite_near_the_top_of_the_band() {
    let mut osc = PulseOsc::new(48_000.0);
    for _ in 0..512 {
        assert!(osc.next_sample(10_000.0, 0.5).is_finite());
    }
}

#[test]
fn triangle_output_stays_bounded() {
    let mut osc = TriOsc::new(48_000.0);
    for _ in 0..512 {
        let sample = osc.next_sample(880.0);
        assert!(sample.is_finite());
        assert!(sample.abs() <= 1.0);
    }
}

#[test]
fn tri_osc_reset_starts_from_a_phase_consistent_negative_ramp() {
    let mut osc = TriOsc::new(48_000.0);
    let first = osc.next_sample(440.0);
    assert!(first < -0.9);

    osc.reset();
    let reset_first = osc.next_sample(440.0);
    assert!(reset_first < -0.9);
}

#[test]
fn noise_source_is_deterministic_for_a_fixed_seed() {
    let mut first = Noise::new(0x0DDC_0FFE_u32);
    let mut second = Noise::new(0x0DDC_0FFE_u32);

    let left = (0..64).map(|_| first.next_sample()).collect::<Vec<_>>();
    let right = (0..64).map(|_| second.next_sample()).collect::<Vec<_>>();

    assert_eq!(left, right);
}

#[test]
fn polyblep_saw_has_smaller_worst_case_step_than_a_naive_saw() {
    let sample_rate_hz = 48_000.0;
    let freq_hz = 8_000.0;
    let mut osc = SawOsc::new(sample_rate_hz);
    let mut previous = osc.next_sample(freq_hz);
    let mut worst_polyblep = 0.0_f32;

    for _ in 0..511 {
        let current = osc.next_sample(freq_hz);
        worst_polyblep = worst_polyblep.max((current - previous).abs());
        previous = current;
    }

    let worst_naive = naive_saw_step(sample_rate_hz, freq_hz, 512);
    assert!(worst_polyblep < worst_naive);
}
