//! Integration tests exercising the full graph combinator system.

#![allow(
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss
)]

use orpheus_dsp::{
    Node, Processor, constant, delay_line, feedback, gain_node, ladder_filter, merge, noise, par,
    passthrough, saw, seq, sine, soft_sat, split, sum,
};

const SR: f32 = 48_000.0;
const FRAMES: usize = 1024;

/// Helper: render a 0-input graph and return its outputs.
fn render_source(node: &mut dyn Node, frames: usize) -> Vec<Vec<f32>> {
    let n_out = node.outputs() as usize;
    let mut bufs: Vec<Vec<f32>> = (0..n_out).map(|_| vec![0.0_f32; frames]).collect();
    let mut refs: Vec<&mut [f32]> = bufs.iter_mut().map(Vec::as_mut_slice).collect();
    node.process(&[], &mut refs, frames);
    bufs
}

// ---------------------------------------------------------------------------
// Subtractive synth: saw → ladder_filter → soft_sat → gain
// ---------------------------------------------------------------------------

#[test]
fn full_subtractive_synth_renders_audio() {
    // Build a monophonic subtractive voice:
    //   Inputs: [freq_hz, cutoff_hz, resonance, drive, gain_amount]
    //
    //   saw(freq_hz) → ladder_filter(audio, cutoff_hz, resonance)
    //                → soft_sat(audio, drive)
    //                → gain(audio, gain_amount)
    //
    // Step 1: saw takes input[0] (freq_hz), produces 1 output.
    // Step 2: We need to route [saw_out, input[1], input[2]] into ladder_filter.
    //         Use wire to select: [5 inputs total → pick indices for each stage]
    //
    // Actually, let's build this as a flat pipeline with explicit wiring.
    //
    // External inputs: 5 channels [freq, cutoff, res, drive, gain_amount]
    //
    // Stage 1: par(saw(sr), passthrough(4))
    //   Takes [freq, cutoff, res, drive, gain_amt]
    //   Produces [audio, cutoff, res, drive, gain_amt]
    //
    // Stage 2: wire to route [audio, cutoff, res] → ladder_filter, pass [drive, gain_amt]
    //   wire([0, 1, 2, 3, 4]) is identity, but we need par(ladder_filter, passthrough(2))
    //   which takes [audio, cutoff, res, drive, gain_amt] → [filtered, drive, gain_amt]
    //
    // Stage 3: par(soft_sat, passthrough(1))
    //   Takes [filtered, drive, gain_amt] → [saturated, gain_amt]
    //
    // Stage 4: gain_node
    //   Takes [saturated, gain_amt] → [output]

    let stage1 = par(saw(SR), passthrough(4));
    let stage2 = par(ladder_filter(SR), passthrough(2));
    let stage3 = par(soft_sat(), passthrough(1));
    let stage4 = gain_node();

    let chain = seq(stage1, seq(stage2, seq(stage3, stage4).unwrap()).unwrap()).unwrap();
    let mut proc = Processor::new(chain);

    assert_eq!(proc.inputs(), 5);
    assert_eq!(proc.outputs(), 1);

    // Constant parameter values.
    let freq = vec![220.0_f32; FRAMES];
    let cutoff = vec![2000.0_f32; FRAMES];
    let res = vec![0.3_f32; FRAMES];
    let drive = vec![1.5_f32; FRAMES];
    let gain_amt = vec![0.8_f32; FRAMES];
    let mut output = vec![0.0_f32; FRAMES];

    proc.process(
        &[&freq, &cutoff, &res, &drive, &gain_amt],
        &mut [&mut output],
        FRAMES,
    );

    // Verify: finite, non-silent, bounded.
    assert!(output.iter().all(|s| s.is_finite()));
    assert!(output.iter().any(|&s| s.abs() > 0.001));
    assert!(output.iter().all(|&s| s.abs() <= 1.0));
}

// ---------------------------------------------------------------------------
// Feedback delay echo
// ---------------------------------------------------------------------------

#[test]
fn feedback_delay_creates_echo() {
    // Build: input → body → output, with body output feeding back through a
    // gain + delay and summing back into the body input.
    //
    // Body: sum(2) — sums external input + feedback.
    //   2 inputs → 1 output.
    // Feedback: seq(gain(0.5), delay(100)) — attenuated delayed copy.
    //   But gain_node takes 2 inputs (audio, amount). We need a fixed-gain adapter.
    //   Use: seq(split(passthrough(1), par(passthrough(1), constant(0.5))), gain_node())
    //   That splits the signal and pairs it with constant 0.5 for gain.
    //
    // Simpler approach: just use a passthrough with manual mixing.
    // Actually, simplest: feedback path is just delay_line(100).
    // The body is sum(2): 2 in, 1 out.
    // feedback is delay_line(100): 1 in, 1 out.
    //
    // Rec: body(2 in, 1 out), feedback(1 in, 1 out).
    //   fb.inputs() = 1 <= body.outputs() = 1 ✓
    //   fb.outputs() = 1 <= body.inputs() = 2 ✓
    //   External inputs = 2 - 1 = 1.
    //   External outputs = 1.

    let body = sum(2);
    let fb = delay_line(100);
    let mut rec = feedback(body, fb).unwrap();

    assert_eq!(rec.inputs(), 1);
    assert_eq!(rec.outputs(), 1);

    // Send impulse at frame 0.
    let frames = 512;
    let mut inp = vec![0.0_f32; frames];
    inp[0] = 1.0;
    let mut out = vec![0.0_f32; frames];

    rec.process(&[&inp], &mut [&mut out], frames);

    // Frame 0: body_in = [delay_buf(0), inp(1)] → sum = 1.0.
    // body_out = 1.0. Feedback receives 1.0, delays by 100 → stores for frame 100.
    assert!((out[0] - 1.0).abs() < f32::EPSILON);

    // The delayed feedback of 1.0 appears at the body input after 100+1 = 101 frames
    // (100 from delay_line, 1 from Rec's inherent one-sample delay).
    // At frame 101: body_in = [delayed_1.0, 0] → sum = 1.0.
    // Wait — delay_line(100) delays by 100 samples. The Rec already applies a 1-sample
    // delay inherently. So total delay = 100 + 1 = 101 samples.
    // But that's the delay from when feedback stores it to when body reads it.
    // Actually: frame 0 → body produces 1.0 → fb receives 1.0 → delay_line stores 1.0.
    // The delay_line will output it 100 frames later (relative to when it was stored).
    // In frame 0: delay_line gets input 1.0, output is 0 (buffer was empty).
    //   fb_out_scratch = 0, stored in delay_buf.
    // In frame 1: body_in = [delay_buf=0, inp=0], body_out = 0. fb input = 0.
    // ... The impulse propagates through the delay_line over 100 frames.
    // In frame 99: delay_line finally outputs 1.0 (after 100 frames internal to delay).
    // But wait, the delay_line's internal processing happens frame by frame within Rec.
    // Frame 0: delay input=1.0, delay output=0.0. Stored 0.0 in delay_buf.
    // Frame 1: body_in=[0, 0], body_out=0. delay input=0, delay output=0. Store 0.
    // ...
    // Frame 100: delay has cycled through its buffer. At frame 100 of the delay_line's
    // processing, the impulse at position 0 has traveled through the 100-slot buffer.
    // delay output = 1.0. Stored in delay_buf = 1.0.
    // Frame 101: body_in = [delay_buf=1.0, inp=0], body_out=1.0. Echo!
    assert!(
        (out[101] - 1.0).abs() < f32::EPSILON,
        "echo at frame 101, got {}",
        out[101]
    );

    // Frames between should be silent.
    assert!(
        out[1..101].iter().all(|&s| s == 0.0),
        "expected silence between 1..101"
    );
}

// ---------------------------------------------------------------------------
// Parallel stereo
// ---------------------------------------------------------------------------

#[test]
fn parallel_stereo_produces_two_channels() {
    // Two sine oscillators at different frequencies → stereo output.
    let stereo = par(sine(SR), sine(SR));
    let mut proc = Processor::new(stereo);

    assert_eq!(proc.inputs(), 2); // two freq inputs
    assert_eq!(proc.outputs(), 2); // two audio outputs

    let freq_l = vec![440.0_f32; FRAMES];
    let freq_r = vec![880.0_f32; FRAMES];
    let mut out_l = vec![0.0_f32; FRAMES];
    let mut out_r = vec![0.0_f32; FRAMES];

    proc.process(&[&freq_l, &freq_r], &mut [&mut out_l, &mut out_r], FRAMES);

    // Both channels should produce audio.
    assert!(out_l.iter().any(|&s| s.abs() > 0.01));
    assert!(out_r.iter().any(|&s| s.abs() > 0.01));

    // They should differ (different frequencies).
    assert_ne!(out_l, out_r);
}

// ---------------------------------------------------------------------------
// All five combinators in one graph
// ---------------------------------------------------------------------------

#[test]
fn graph_using_all_five_combinators() {
    // Build a graph that uses seq, par, split, merge, and feedback.
    //
    // 1. par(constant(440), constant(880)) → 0 in, 2 out
    // 2. split into 4 channels: 2 out → passthrough(4) [2x cyclic duplication]
    // 3. merge 4 channels into 2: merge(:> passthrough(2))
    // 4. seq through a passthrough(2) (identity, just to use seq)
    // 5. merge 2 into 1: merge(:> passthrough(1))
    // 6. feedback: sum(2) ~ delay(1), just to exercise the recursive combinator

    // Steps 1-5: reduce two constants to one summed value.
    let source = par(constant(440.0), constant(880.0));

    // split: 2 out → 4 channels (cyclic: [440, 880, 440, 880])
    let split_step = split(source, passthrough(4)).unwrap();

    // merge 4→2: groups of 2 summed → [440+440, 880+880] = [880, 1760]
    let merge_step = merge(split_step, passthrough(2)).unwrap();

    // seq through identity
    let seq_step = seq(merge_step, passthrough(2)).unwrap();

    // merge 2→1: sum → 880 + 1760 = 2640
    let final_merge = merge(seq_step, passthrough(1)).unwrap();

    // feedback: sum(2) body with delay(1) feedback.
    // The input is the constant 2640. The feedback adds the delayed version.
    // body = sum(2): 2 in, 1 out. fb = delay(1): 1 in, 1 out.
    // External inputs = 2 - 1 = 1. But final_merge has 0 inputs and 1 output.
    // We need to seq final_merge into the feedback body's external input.
    //
    // Actually, let's just build the feedback separately and seq the source into it.
    let rec = feedback(sum(2), delay_line(1)).unwrap();
    // rec: 1 external input, 1 output.

    let full = seq(final_merge, rec).unwrap();
    let mut proc = Processor::new(full);

    assert_eq!(proc.inputs(), 0);
    assert_eq!(proc.outputs(), 1);

    let out = render_source(&mut proc, 32);

    // Frame 0: body_in=[delay_buf=0, 2640] → 2640. fb delay in=2640, out=0. delay_buf=0.
    // Frame 1: body_in=[0, 2640] → 2640. fb delay in=2640, out=2640. delay_buf=2640.
    // Frame 2: body_in=[2640, 2640] → 5280. fb delay in=5280, out=2640. delay_buf=2640.
    // Frame 3: body_in=[2640, 2640] → 5280. fb delay in=5280, out=5280. delay_buf=5280.
    // Frame 4: body_in=[5280, 2640] → 7920.
    // Total latency: delay_line(1) + Rec's 1-sample = 2 frames before feedback arrives.
    assert!((out[0][0] - 2640.0).abs() < 0.01);
    assert!((out[0][1] - 2640.0).abs() < 0.01);
    assert!((out[0][2] - 5280.0).abs() < 0.01);
    assert!((out[0][4] - 7920.0).abs() < 0.01);
}

// ---------------------------------------------------------------------------
// FM synth: two oscillators with frequency modulation
// ---------------------------------------------------------------------------

#[test]
fn fm_synth_two_oscillator_patch() {
    // FM synthesis: modulator oscillator modulates the frequency of a carrier.
    //
    // carrier_freq = base_freq + mod_depth * modulator(mod_freq)
    //
    // Since we need to do arithmetic on signals, we'll build this with
    // a custom node that performs the FM math, or use the existing primitives
    // creatively.
    //
    // Approach: use constant inputs and wire nodes.
    // External inputs: none (all constants for this test).
    //
    // 1. Modulator: constant(mod_freq) → sine → output is [-1, 1]
    // 2. Scale mod output: we need mod_depth * sin. Use gain_node(audio=sin, amount=depth).
    // 3. Add carrier base: we need base_freq + scaled_mod. Use sum(2).
    // 4. Carrier: result → sine → audio output.

    let mod_freq = 110.0_f32;
    let mod_depth = 200.0_f32;
    let carrier_base = 440.0_f32;

    // Modulator path: constant(mod_freq) → sine → audio
    let modulator = seq(constant(mod_freq), sine(SR)).unwrap();
    // modulator: 0 in, 1 out (mod audio)

    // Scale: par(modulator, constant(mod_depth)) → gain_node
    let scaled_mod = seq(par(modulator, constant(mod_depth)), gain_node()).unwrap();
    // scaled_mod: 0 in, 1 out (mod_depth * sin(mod_freq))

    // Add carrier base: par(scaled_mod, constant(carrier_base)) → sum(2)
    let carrier_freq_signal = seq(par(scaled_mod, constant(carrier_base)), sum(2)).unwrap();
    // carrier_freq_signal: 0 in, 1 out (carrier_base + mod_depth * sin)

    // Carrier: → sine
    let fm_synth = seq(carrier_freq_signal, sine(SR)).unwrap();
    // fm_synth: 0 in, 1 out

    let mut proc = Processor::new(fm_synth);
    assert_eq!(proc.inputs(), 0);
    assert_eq!(proc.outputs(), 1);

    let out = render_source(&mut proc, FRAMES);

    // Verify: finite, non-silent, bounded.
    assert!(out[0].iter().all(|s| s.is_finite()));
    assert!(out[0].iter().any(|&s| s.abs() > 0.01));
    assert!(out[0].iter().all(|&s| (-1.0..=1.0).contains(&s)));

    // FM produces a richer spectrum than a pure sine — check that the output
    // isn't just a simple sine by verifying it crosses zero more frequently
    // than a 440 Hz sine would (FM sidebands create additional zero crossings).
    let zero_crossings = out[0]
        .windows(2)
        .filter(|w| w[0].signum() != w[1].signum())
        .count();
    let expected_pure_sine_crossings = (2.0 * carrier_base * FRAMES as f32 / SR) as usize;
    // FM should create more zero crossings than a pure carrier.
    assert!(
        zero_crossings > expected_pure_sine_crossings,
        "FM should produce more zero crossings ({zero_crossings}) than pure sine ({expected_pure_sine_crossings})"
    );
}

// ---------------------------------------------------------------------------
// Noise → filter: tests adapter + combinator together
// ---------------------------------------------------------------------------

#[test]
fn noise_through_filter_produces_colored_noise() {
    // noise → par with cutoff+res constants → ladder_filter
    let source_with_params = par(noise(42), par(constant(500.0), constant(0.5)));
    // 0 in, 3 out: [noise, cutoff, res]

    let chain = seq(source_with_params, ladder_filter(SR)).unwrap();
    let mut proc = Processor::new(chain);

    assert_eq!(proc.inputs(), 0);
    assert_eq!(proc.outputs(), 1);

    let out = render_source(&mut proc, FRAMES);

    assert!(out[0].iter().all(|s| s.is_finite()));
    assert!(out[0].iter().any(|&s| s.abs() > 0.001));
}

// ---------------------------------------------------------------------------
// Processor reset preserves graph semantics
// ---------------------------------------------------------------------------

#[test]
fn processor_reset_preserves_graph_determinism() {
    let chain = seq(constant(440.0), sine(SR)).unwrap();
    let mut proc = Processor::new(chain);

    let out1 = render_source(&mut proc, FRAMES);
    proc.reset();
    let out2 = render_source(&mut proc, FRAMES);

    assert_eq!(out1, out2);
}
