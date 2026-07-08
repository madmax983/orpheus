//! Tests for the fractional/modulatable delay line (`fdelay`).
//!
//! `fdelay` is the chorus/flanger building block: its delay TIME is a signal
//! input (params-as-signals, ADR 0004) that may move at audio rate, and the
//! node interpolates linearly between samples so non-integer delays are
//! rendered smoothly.

// Test signals index with small usizes (exact in f32), and several tests
// deliberately assert EXACT f32 arithmetic at a power-of-two sample rate.
#![allow(
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::float_cmp
)]

use std::f32::consts::TAU;

use orpheus_dsp::{
    GraphVoiceSpec, Node, Processor, VoiceNodeSpec, VoiceSignalRef, constant, delay_line, fdelay,
    gain_node, par, passthrough, seq, sine, sum, wire,
};

/// A power-of-two sample rate so delay times expressed as dyadic fractions
/// convert to sample counts exactly in `f32` (no rounding slack in the
/// exact-arithmetic assertions below).
const EXACT_SR: f32 = 32_768.0;
const SR: f32 = 48_000.0;

/// Drives a 1-input node over `audio` in `block`-sized chunks.
fn process1(node: &mut dyn Node, audio: &[f32], block: usize) -> Vec<f32> {
    assert_eq!(node.inputs(), 1);
    assert_eq!(node.outputs(), 1);
    let frames = audio.len();
    let mut out = vec![0.0_f32; frames];
    let mut start = 0;
    while start < frames {
        let end = (start + block).min(frames);
        let inputs: [&[f32]; 1] = [&audio[start..end]];
        let mut outputs: [&mut [f32]; 1] = [&mut out[start..end]];
        node.process(&inputs, &mut outputs, end - start);
        start = end;
    }
    out
}

/// Drives a 2-input node (audio, delay\_seconds) in `block`-sized chunks.
fn process2(node: &mut dyn Node, audio: &[f32], seconds: &[f32], block: usize) -> Vec<f32> {
    assert_eq!(node.inputs(), 2);
    assert_eq!(node.outputs(), 1);
    let frames = audio.len();
    let mut out = vec![0.0_f32; frames];
    let mut start = 0;
    while start < frames {
        let end = (start + block).min(frames);
        let inputs: [&[f32]; 2] = [&audio[start..end], &seconds[start..end]];
        let mut outputs: [&mut [f32]; 1] = [&mut out[start..end]];
        node.process(&inputs, &mut outputs, end - start);
        start = end;
    }
    out
}

/// A deterministic, non-trivial test signal.
fn wiggle(frames: usize) -> Vec<f32> {
    (0..frames)
        .map(|t| ((t * 37 % 101) as f32 - 50.0) / 50.0)
        .collect()
}

#[test]
fn fdelay_matches_delay_line_at_a_constant_whole_sample_delay() {
    // 64 samples at 32768 Hz is 2^-9 seconds — exactly representable, so the
    // requested delay converts back to exactly 64.0 samples and the linear
    // interpolation weight is exactly zero: outputs must match bit-for-bit.
    let frames = 512;
    let audio = wiggle(frames);
    let seconds = vec![64.0 / EXACT_SR; frames];

    let mut fractional = fdelay(EXACT_SR, 0.01);
    let modulatable = process2(&mut fractional, &audio, &seconds, 128);

    let mut fixed = delay_line(64);
    let reference = process1(&mut fixed, &audio, 128);

    for (i, (a, b)) in modulatable.iter().zip(reference.iter()).enumerate() {
        assert_eq!(
            a.to_bits(),
            b.to_bits(),
            "sample {i}: fdelay at a whole-sample delay must match delay_line ({a} vs {b})"
        );
    }
}

#[test]
fn fdelay_half_sample_delay_interpolates_a_ramp_exactly() {
    // A 0.5-sample delay of the ramp x[t] = t must produce t - 0.5 for t >= 1
    // under linear interpolation, with exact f32 arithmetic at this scale.
    let frames = 256;
    let audio: Vec<f32> = (0..frames).map(|t| t as f32).collect();
    let seconds = vec![0.5 / EXACT_SR; frames];

    let mut node = fdelay(EXACT_SR, 0.01);
    let out = process2(&mut node, &audio, &seconds, 64);

    assert_eq!(out[0], 0.0, "sample 0 interpolates into silent history");
    for (t, &value) in out.iter().enumerate().skip(1) {
        assert_eq!(
            value,
            t as f32 - 0.5,
            "sample {t}: 0.5-sample delay of a ramp must be the midpoint"
        );
    }
}

#[test]
fn fdelay_clamps_delay_time_to_zero_and_capacity() {
    let frames = 256;
    let audio = wiggle(frames);
    // Capacity: 64 samples (2^-9 s at 32768 Hz).
    let max_seconds = 64.0 / EXACT_SR;

    // Negative and non-finite delay times clamp to zero delay (identity).
    for bad in [-1.0_f32, f32::NAN, f32::NEG_INFINITY] {
        let mut node = fdelay(EXACT_SR, max_seconds);
        let out = process2(&mut node, &audio, &vec![bad; frames], 64);
        assert_eq!(out, audio, "delay time {bad} must clamp to zero delay");
    }

    // Times beyond the capacity clamp to the maximum delay.
    let mut clamped = fdelay(EXACT_SR, max_seconds);
    let clamped_out = process2(&mut clamped, &audio, &vec![1.0; frames], 64);
    let mut reference = delay_line(64);
    let reference_out = process1(&mut reference, &audio, 64);
    assert_eq!(
        clamped_out, reference_out,
        "an over-long delay request must clamp to the line's capacity"
    );
}

#[test]
fn fdelay_capacity_is_capped_at_ten_seconds() {
    // Requesting a 100 s line caps the capacity at 10 s (like voice delays);
    // an impulse driven with an over-long delay request emerges after exactly
    // 10 s worth of samples.
    let sr = 1_000.0;
    let frames = 10_050;
    let mut audio = vec![0.0_f32; frames];
    audio[0] = 1.0;
    let seconds = vec![50.0_f32; frames];

    let mut node = fdelay(sr, 100.0);
    let out = process2(&mut node, &audio, &seconds, 512);

    for (t, &value) in out.iter().enumerate() {
        let expected = if t == 10_000 { 1.0 } else { 0.0 };
        assert_eq!(
            value, expected,
            "impulse must emerge at exactly 10 s (t={t})"
        );
    }
}

#[test]
fn fdelay_reset_clears_history() {
    let frames = 16;
    let seconds = vec![2.0 / EXACT_SR; frames];
    let mut node = fdelay(EXACT_SR, 0.01);
    let _ = process2(&mut node, &vec![1.0; frames], &seconds, 16);

    node.reset();
    let out = process2(&mut node, &vec![0.0; frames], &seconds, 16);
    assert!(
        out.iter().all(|&s| s == 0.0),
        "reset must clear the delay history"
    );
}

#[test]
fn rising_delay_time_lowers_pitch_without_discontinuities() {
    // Doppler: while the delay time rises, the read head recedes from the
    // write head and the output is pitched down by (1 - d/dt delay). A ramp
    // from 0 to 50 ms over half a second is a slope of 0.1, so a 440 Hz sine
    // should emerge near 396 Hz — measurably fewer zero crossings.
    let frames = 24_000;
    let audio: Vec<f32> = (0..frames)
        .map(|t| (TAU * 440.0 * t as f32 / SR).sin())
        .collect();
    let ramp: Vec<f32> = (0..frames)
        .map(|t| 0.05 * t as f32 / frames as f32)
        .collect();
    let fixed_time = vec![0.025_f32; frames];

    let mut node = fdelay(SR, 0.06);
    let modulated = process2(&mut node, &audio, &ramp, 512);
    let mut node = fdelay(SR, 0.06);
    let fixed = process2(&mut node, &audio, &fixed_time, 512);

    let crossings = |signal: &[f32]| {
        signal[2_000..]
            .windows(2)
            .filter(|pair| pair[0] * pair[1] < 0.0)
            .count()
    };
    let modulated_crossings = crossings(&modulated);
    let fixed_crossings = crossings(&fixed);

    assert!(
        (modulated_crossings as f32) < fixed_crossings as f32 * 0.95,
        "rising delay must lower the pitch: {modulated_crossings} vs {fixed_crossings} crossings"
    );
    assert!(
        (modulated_crossings as f32) > fixed_crossings as f32 * 0.8,
        "the doppler shift must stay near the predicted ratio: \
         {modulated_crossings} vs {fixed_crossings} crossings"
    );

    // Artifact sanity: finite, bounded (linear interpolation is a convex
    // combination of inputs), and free of clicks.
    let max_step = TAU * 440.0 / SR * 1.5;
    for (i, pair) in modulated.windows(2).enumerate().skip(2_000) {
        assert!(pair[1].is_finite(), "sample {i} must be finite");
        assert!(pair[1].abs() <= 1.0001, "sample {i} must stay bounded");
        assert!(
            (pair[1] - pair[0]).abs() <= max_step,
            "sample {i} jumps by {} — modulated delay must not click",
            (pair[1] - pair[0]).abs()
        );
    }
}

/// Builds a wet-plus-dry chorus around `delay_time` (a 0-in/1-out node
/// producing the delay time in seconds): the mono input is split into a dry
/// path and an `fdelay` wet path, and the two are summed.
fn chorus(delay_time: impl Node + 'static) -> Processor {
    let wet =
        seq(par(passthrough(1), delay_time), fdelay(SR, 0.02)).expect("wet path must compose");
    let mixed = seq(par(passthrough(1), wet), sum(2)).expect("dry/wet mix must compose");
    let graph = seq(wire(&[0, 0]), mixed).expect("input split must compose");
    Processor::new(graph)
}

fn render(processor: &mut Processor, audio: &[f32]) -> Vec<f32> {
    let frames = audio.len();
    let mut out = vec![0.0_f32; frames];
    let mut start = 0;
    while start < frames {
        let end = (start + 512).min(frames);
        let inputs: [&[f32]; 1] = [&audio[start..end]];
        let mut outputs: [&mut [f32]; 1] = [&mut out[start..end]];
        processor.process(&inputs, &mut outputs, end - start);
        start = end;
    }
    out
}

#[test]
fn chorus_patch_with_lfo_modulated_fdelay_differs_from_fixed_delay_and_stays_bounded() {
    // The demonstration patch from the parity roadmap: a 1.5 Hz sine LFO
    // sweeps the wet path's delay time across 5.5..9.5 ms. The composed graph
    // uses only existing nodes (sine, constant, gain, sum) plus fdelay.
    let frames = 48_000;
    let audio: Vec<f32> = (0..frames)
        .map(|t| (TAU * 220.0 * t as f32 / SR).sin())
        .collect();

    let lfo = seq(constant(1.5), sine(SR)).expect("LFO must compose");
    let scaled = seq(par(lfo, constant(0.002)), gain_node()).expect("LFO depth must compose");
    let swept = seq(par(scaled, constant(0.0075)), sum(2)).expect("LFO offset must compose");

    let mut chorused = chorus(swept);
    let mut static_delay = chorus(constant(0.0075));

    let wet = render(&mut chorused, &audio);
    let still = render(&mut static_delay, &audio);

    let mut difference = 0.0_f64;
    for (i, (a, b)) in wet.iter().zip(still.iter()).enumerate() {
        assert!(a.is_finite(), "chorus sample {i} must be finite");
        assert!(
            a.abs() <= 2.0001,
            "chorus output must stay within dry + wet bounds, got {a} at {i}"
        );
        difference += f64::from((a - b).abs());
    }
    let mean_difference = difference / f64::from(frames as u32);
    assert!(
        mean_difference > 0.01,
        "the LFO-modulated chorus must audibly differ from a fixed delay \
         (mean |diff| = {mean_difference})"
    );

    let energy: f32 = wet.iter().map(|s| s.abs()).sum();
    assert!(energy > 1.0, "the chorus patch must be audible");
}

#[test]
fn voice_spec_fractional_delay_is_modulatable_by_a_bound_signal() {
    // Engine-side proof that a voice-body LFO can drive the delay time: the
    // FractionalDelay spec node reads its seconds input as a signal.
    let flanged = GraphVoiceSpec::new(
        "flange",
        0.05,
        vec![
            VoiceNodeSpec::Sine {
                freq: VoiceSignalRef::Freq,
            },
            VoiceNodeSpec::Ar {
                gate: VoiceSignalRef::Gate,
                attack_s: 0.001,
                release_s: 0.05,
            },
            VoiceNodeSpec::Mul {
                left: VoiceSignalRef::Node(0),
                right: VoiceSignalRef::Node(1),
            },
            VoiceNodeSpec::Constant { value: 2.0 },
            VoiceNodeSpec::Sine {
                freq: VoiceSignalRef::Node(3),
            },
            VoiceNodeSpec::Constant { value: 0.002 },
            VoiceNodeSpec::Mul {
                left: VoiceSignalRef::Node(4),
                right: VoiceSignalRef::Node(5),
            },
            VoiceNodeSpec::Constant { value: 0.005 },
            VoiceNodeSpec::Add {
                left: VoiceSignalRef::Node(6),
                right: VoiceSignalRef::Node(7),
            },
            VoiceNodeSpec::FractionalDelay {
                input: VoiceSignalRef::Node(2),
                seconds: VoiceSignalRef::Node(8),
                max_seconds: 0.02,
            },
            VoiceNodeSpec::Add {
                left: VoiceSignalRef::Node(2),
                right: VoiceSignalRef::Node(9),
            },
        ],
        VoiceSignalRef::Node(10),
    )
    .expect("flange spec must validate");

    // The same voice with the LFO detached (fixed 5 ms delay time).
    let fixed = GraphVoiceSpec::new(
        "flange",
        0.05,
        vec![
            VoiceNodeSpec::Sine {
                freq: VoiceSignalRef::Freq,
            },
            VoiceNodeSpec::Ar {
                gate: VoiceSignalRef::Gate,
                attack_s: 0.001,
                release_s: 0.05,
            },
            VoiceNodeSpec::Mul {
                left: VoiceSignalRef::Node(0),
                right: VoiceSignalRef::Node(1),
            },
            VoiceNodeSpec::Constant { value: 0.005 },
            VoiceNodeSpec::FractionalDelay {
                input: VoiceSignalRef::Node(2),
                seconds: VoiceSignalRef::Node(3),
                max_seconds: 0.02,
            },
            VoiceNodeSpec::Add {
                left: VoiceSignalRef::Node(2),
                right: VoiceSignalRef::Node(4),
            },
        ],
        VoiceSignalRef::Node(5),
    )
    .expect("fixed spec must validate");

    let mut modulated_voice = flanged.build_voice(SR);
    modulated_voice.prepare();
    let mut fixed_voice = fixed.build_voice(SR);
    fixed_voice.prepare();

    let mut difference = 0.0_f32;
    let mut energy = 0.0_f32;
    for _ in 0..9_600 {
        let (ml, _) = modulated_voice.process_frame(1.0, 220.0, 1.0, 0.0);
        let (fl, _) = fixed_voice.process_frame(1.0, 220.0, 1.0, 0.0);
        assert!(ml.is_finite(), "modulated voice output must be finite");
        difference += (ml - fl).abs();
        energy += ml.abs();
    }
    assert!(energy > 1.0, "the flanged voice must be audible");
    assert!(
        difference > 1.0,
        "the LFO must audibly move the delay time (total |diff| = {difference})"
    );
}

#[test]
fn voice_spec_rejects_fractional_delay_with_invalid_capacity() {
    for bad in [0.0_f32, -1.0, f32::NAN, 11.0] {
        let result = GraphVoiceSpec::new(
            "bad",
            0.05,
            vec![
                VoiceNodeSpec::Sine {
                    freq: VoiceSignalRef::Freq,
                },
                VoiceNodeSpec::Constant { value: 0.005 },
                VoiceNodeSpec::FractionalDelay {
                    input: VoiceSignalRef::Node(0),
                    seconds: VoiceSignalRef::Node(1),
                    max_seconds: bad,
                },
            ],
            VoiceSignalRef::Node(2),
        );
        assert!(
            result.is_err(),
            "a fractional delay capacity of {bad} must be rejected"
        );
    }
}
