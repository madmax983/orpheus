//! Allocation-discipline tests for the graph voice audio-thread path.
//!
//! The render engine constructs and prepares graph voices off the audio
//! thread; the per-frame `process` path must then be allocation-free. A
//! thread-local counting allocator proves it: after `prepare()`, rendering
//! any number of frames performs zero heap allocations on the calling thread.

// A counting `GlobalAlloc` cannot be written without `unsafe`; the blocks
// below only delegate to `System` after bumping a thread-local counter.
#![allow(unsafe_code)]

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

use orpheus_dsp::{
    GraphVoiceBank, GraphVoiceSpec, Node, PlaybackSample, Processor, SampleTrigger, SvfMode,
    TrackId, VoiceNodeSpec, VoiceParamBreakpoint, VoiceSignalRef, adsr, bind,
    builtin_graph_voice_programs, constant, fdelay, gain_node, graph_note_voice_params,
    graph_note_voice_ramps, pan, par, passthrough, sample_player, seq, sine, sum, wire,
};

struct CountingAllocator;

thread_local! {
    static ALLOCATION_COUNT: Cell<u64> = const { Cell::new(0) };
}

fn allocation_count() -> u64 {
    ALLOCATION_COUNT.with(Cell::get)
}

fn record_allocation() {
    // `try_with` so allocator calls during TLS teardown never panic.
    let _ = ALLOCATION_COUNT.try_with(|count| count.set(count.get() + 1));
}

unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        record_allocation();
        unsafe { System.alloc(layout) }
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        record_allocation();
        unsafe { System.alloc_zeroed(layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        record_allocation();
        unsafe { System.realloc(ptr, layout, new_size) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) }
    }
}

#[global_allocator]
static GLOBAL: CountingAllocator = CountingAllocator;

const SR: f32 = 48_000.0;

#[test]
fn prepared_graph_voice_processes_without_allocating() {
    let program = builtin_graph_voice_programs()
        .iter()
        .find(|program| program.token() == "gsine")
        .expect("gsine graph voice program should be built in");
    let mut voice = program.build_voice(SR);
    voice.prepare();

    let before = allocation_count();
    let mut energy = 0.0_f32;
    for frame in 0..4_096_u32 {
        let gate = if frame < 2_048 { 1.0 } else { 0.0 };
        let (left, right) = voice.process_frame(gate, 220.0, 0.8, -0.25);
        energy += left.abs() + right.abs();
    }
    let after = allocation_count();

    assert!(energy > 0.0, "prepared voice should produce audio");
    assert_eq!(
        after - before,
        0,
        "graph voice process_frame must not allocate after prepare()"
    );
}

#[test]
fn prepared_user_spec_voice_processes_without_allocating() {
    // A user-defined program (saw x ADSR through a ladder lowpass) compiled
    // from its declarative spec must follow the same discipline as builtins:
    // construction and prepare() may allocate, process_frame() must not.
    let spec = GraphVoiceSpec::new(
        "pluck",
        0.05,
        vec![
            VoiceNodeSpec::Saw {
                freq: VoiceSignalRef::Freq,
            },
            VoiceNodeSpec::Constant { value: 1_500.0 },
            VoiceNodeSpec::Constant { value: 0.2 },
            VoiceNodeSpec::Lowpass {
                input: VoiceSignalRef::Node(0),
                cutoff_hz: VoiceSignalRef::Node(1),
                resonance: VoiceSignalRef::Node(2),
            },
            VoiceNodeSpec::Adsr {
                gate: VoiceSignalRef::Gate,
                attack_s: 0.001,
                decay_s: 0.02,
                sustain: 0.6,
                release_s: 0.05,
            },
            VoiceNodeSpec::Mul {
                left: VoiceSignalRef::Node(3),
                right: VoiceSignalRef::Node(4),
            },
        ],
        VoiceSignalRef::Node(5),
    )
    .expect("user voice spec should validate");

    let mut voice = spec.build_voice(SR);
    voice.prepare();

    let before = allocation_count();
    let mut energy = 0.0_f32;
    for frame in 0..4_096_u32 {
        let gate = if frame < 2_048 { 1.0 } else { 0.0 };
        let (left, right) = voice.process_frame(gate, 220.0, 0.8, 0.25);
        energy += left.abs() + right.abs();
    }
    let after = allocation_count();

    assert!(energy > 0.0, "prepared user voice should produce audio");
    assert_eq!(
        after - before,
        0,
        "user spec voice process_frame must not allocate after prepare()"
    );
}

#[test]
fn warmed_combinator_processor_processes_without_allocating() {
    // The #1385 gated stereo voice expressed directly with combinators:
    // gate -> [sine(440) x adsr] -> gain -> pan(-0.5). After one warm-up
    // block, block processing must be allocation-free.
    const FRAMES: usize = 512;

    let carrier = seq(constant(440.0), sine(SR)).unwrap();
    let envelope = bind(adsr(SR), &[(1, 0.005), (2, 0.005), (3, 0.6), (4, 0.01)]).unwrap();
    let voice = seq(par(carrier, envelope), gain_node()).unwrap();
    let stereo = seq(voice, bind(pan(), &[(1, -0.5)]).unwrap()).unwrap();

    let mut processor = Processor::new(stereo);
    let gate = vec![1.0_f32; FRAMES];
    let mut left = vec![0.0_f32; FRAMES];
    let mut right = vec![0.0_f32; FRAMES];

    // Warm-up: lazy-but-once scratch growth happens here, off the hot path.
    processor.process(&[&gate], &mut [&mut left, &mut right], FRAMES);

    let before = allocation_count();
    for _ in 0..16 {
        processor.process(&[&gate], &mut [&mut left, &mut right], FRAMES);
    }
    let after = allocation_count();

    assert!(left.iter().any(|&s| s.abs() > 0.01));
    assert_eq!(
        after - before,
        0,
        "combinator processing must not allocate after the first block"
    );
}

#[test]
fn warmed_fdelay_chorus_processor_processes_without_allocating() {
    // The fractional-delay chorus patch: a 1.5 Hz sine LFO sweeping fdelay's
    // delay-time signal input. The delay buffer is allocated at construction
    // and the interpolated read path must be allocation-free after warm-up.
    const FRAMES: usize = 512;

    let lfo = seq(constant(1.5), sine(SR)).unwrap();
    let scaled = seq(par(lfo, constant(0.002)), gain_node()).unwrap();
    let swept = seq(par(scaled, constant(0.0075)), sum(2)).unwrap();
    let wet = seq(par(passthrough(1), swept), fdelay(SR, 0.02)).unwrap();
    let mixed = seq(par(passthrough(1), wet), sum(2)).unwrap();
    let chorus = seq(wire(&[0, 0]), mixed).unwrap();

    let mut processor = Processor::new(chorus);
    let audio = vec![0.5_f32; FRAMES];
    let mut out = vec![0.0_f32; FRAMES];

    // Warm-up: lazy-but-once scratch growth happens here, off the hot path.
    processor.process(&[&audio], &mut [&mut out], FRAMES);

    let before = allocation_count();
    for _ in 0..16 {
        processor.process(&[&audio], &mut [&mut out], FRAMES);
    }
    let after = allocation_count();

    assert!(out.iter().any(|&s| s.abs() > 0.01));
    assert_eq!(
        after - before,
        0,
        "fdelay chorus processing must not allocate after the first block"
    );
}

#[test]
fn voice_with_signal_driven_delay_renders_without_allocating() {
    // A voice-body flanger: an LFO (sine x depth + offset) drives the
    // fractional delay's seconds input. Prepared voices must render the
    // modulated read path without allocating.
    let spec = GraphVoiceSpec::new(
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
    .expect("flange spec should validate");

    let mut voice = spec.build_voice(SR);
    voice.prepare();

    let before = allocation_count();
    let mut energy = 0.0_f32;
    for frame in 0..4_096_u32 {
        let gate = if frame < 2_048 { 1.0 } else { 0.0 };
        let (left, right) = voice.process_frame(gate, 220.0, 0.8, 0.0);
        energy += left.abs() + right.abs();
    }
    let after = allocation_count();

    assert!(energy > 0.0, "prepared flange voice should produce audio");
    assert_eq!(
        after - before,
        0,
        "signal-driven delay voices must not allocate after prepare()"
    );
}

#[test]
fn sample_player_node_processes_without_allocating() {
    // The player holds an `Arc` to the preloaded buffer, resolved at
    // construction; processing (trigger detection, interpolated reads,
    // retriggers) must not allocate or lock.
    const FRAMES: usize = 512;

    #[allow(clippy::cast_precision_loss)]
    let buffer: Vec<f32> = (0..4_800).map(|i| (i as f32 * 0.01).sin()).collect();
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let sample = PlaybackSample::from_mono_frames(buffer, SR as u32);
    let mut node = sample_player(&sample, SR);

    let mut gate = vec![1.0_f32; FRAMES];
    // A retrigger edge inside every block keeps the restart path counted.
    gate[FRAMES / 2] = 0.0;
    let rate = vec![1.5_f32; FRAMES];
    let mut out = vec![0.0_f32; FRAMES];

    let before = allocation_count();
    let mut energy = 0.0_f32;
    for _ in 0..16 {
        node.process(&[&gate, &rate], &mut [&mut out], FRAMES);
        energy += out.iter().map(|sample| sample.abs()).sum::<f32>();
    }
    let after = allocation_count();

    assert!(energy > 0.0, "the sample player should produce audio");
    assert_eq!(
        after - before,
        0,
        "sample playback must not allocate after construction"
    );
}

#[test]
fn voice_with_sample_playback_renders_without_allocating() {
    // A hybrid instrument: a one-shot sample layered with a sine, both
    // shaped by a gate-driven AR envelope. Pool build resolves the buffer
    // handle off-thread; trigger/render must then be allocation-free.
    #[allow(clippy::cast_precision_loss)]
    let buffer: Vec<f32> = (0..2_400)
        .map(|i| ((i as f32) * 0.02).sin() * 0.5)
        .collect();
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let sample = PlaybackSample::from_mono_frames(buffer, SR as u32);

    let spec = GraphVoiceSpec::new(
        "hybrid",
        0.1,
        vec![
            VoiceNodeSpec::Constant { value: 1.0 },
            VoiceNodeSpec::Sample {
                gate: VoiceSignalRef::Gate,
                rate: VoiceSignalRef::Node(0),
                sample,
                looped: false,
                loop_crossfade: false,
                pitch_reference_hz: None,
            },
            VoiceNodeSpec::Sine {
                freq: VoiceSignalRef::Freq,
            },
            VoiceNodeSpec::Add {
                left: VoiceSignalRef::Node(1),
                right: VoiceSignalRef::Node(2),
            },
            VoiceNodeSpec::Ar {
                gate: VoiceSignalRef::Gate,
                attack_s: 0.001,
                release_s: 0.1,
            },
            VoiceNodeSpec::Mul {
                left: VoiceSignalRef::Node(3),
                right: VoiceSignalRef::Node(4),
            },
        ],
        VoiceSignalRef::Node(5),
    )
    .expect("hybrid sample voice spec should validate");

    let mut bank = GraphVoiceBank::with_user_programs(SR, vec![spec]);
    let track = TrackId::new(0);
    let mut mix = vec![(0.0_f32, 0.0_f32); 1];

    let before = allocation_count();
    assert!(bank.trigger("hybrid", track, 2_048, 220.0, 0.8, 0.0));
    let mut energy = 0.0_f32;
    for _ in 0..4_096 {
        mix[0] = (0.0, 0.0);
        bank.render_frame(&mut mix);
        energy += mix[0].0.abs() + mix[0].1.abs();
    }
    let after = allocation_count();

    assert!(energy > 0.0, "the hybrid sample voice should be audible");
    assert_eq!(
        after - before,
        0,
        "sample-playback voices must not allocate after the pool is built"
    );
}

#[test]
fn bank_with_feedback_delay_merge_and_custom_polyphony_renders_without_allocating() {
    // The richer vocabulary (feedback taps via `Rec`, fixed delay lines, and
    // n-ary merge via `Mrg`) must keep the audio-thread discipline: after the
    // bank is built (which compiles and prepares a non-default pool of 3
    // voices), triggering and rendering never allocate.
    let spec = GraphVoiceSpec::new(
        "echoverb",
        0.3,
        vec![
            // Dry blip: sine x AR envelope.
            VoiceNodeSpec::Sine {
                freq: VoiceSignalRef::Freq,
            },
            VoiceNodeSpec::Ar {
                gate: VoiceSignalRef::Gate,
                attack_s: 0.001,
                release_s: 0.01,
            },
            VoiceNodeSpec::Mul {
                left: VoiceSignalRef::Node(0),
                right: VoiceSignalRef::Node(1),
            },
            VoiceNodeSpec::Constant { value: 0.5 },
            // Feedback echo loop: wet = 0.5 * delay(dry + wet).
            VoiceNodeSpec::Add {
                left: VoiceSignalRef::Node(2),
                right: VoiceSignalRef::Feedback(6),
            },
            VoiceNodeSpec::Delay {
                input: VoiceSignalRef::Node(4),
                seconds: 0.01,
            },
            VoiceNodeSpec::Mul {
                left: VoiceSignalRef::Node(5),
                right: VoiceSignalRef::Node(3),
            },
            // Fan-in of dry and wet through the merge combinator.
            VoiceNodeSpec::Merge {
                inputs: vec![VoiceSignalRef::Node(2), VoiceSignalRef::Node(6)],
            },
        ],
        VoiceSignalRef::Node(7),
    )
    .expect("echo spec should validate")
    .with_polyphony(3)
    .expect("polyphony 3 is within bounds");

    let mut bank = GraphVoiceBank::with_user_programs(SR, vec![spec]);
    let track = TrackId::new(0);
    let mut mix = vec![(0.0_f32, 0.0_f32); 1];

    let before = allocation_count();
    for _ in 0..3 {
        assert!(bank.trigger("echoverb", track, 2_048, 220.0, 0.8, 0.0));
    }
    // The pool is exhausted: the fourth note steals the oldest voice. The
    // steal decision, the one-frame gate gap, and the gain/pan handover ramp
    // all sit inside the counted region, so they must be allocation-free too.
    assert!(
        bank.trigger("echoverb", track, 2_048, 440.0, 0.6, 0.25),
        "the poly-3 pool must steal for a fourth simultaneous note"
    );
    let mut energy = 0.0_f32;
    for _ in 0..4_096 {
        mix[0] = (0.0, 0.0);
        bank.render_frame(&mut mix);
        energy += mix[0].0.abs() + mix[0].1.abs();
    }
    let after = allocation_count();

    assert!(energy > 0.0, "pooled voices should produce audio");
    assert_eq!(
        after - before,
        0,
        "bank trigger/render with feedback, delay, merge, poly 3, and a steal must not allocate"
    );
}

#[test]
fn param_driven_pooled_voice_renders_without_allocating() {
    // A pattern-parameter-driven instrument (ADR 0010 addendum): a saw
    // through an SVF lowpass whose cutoff is the per-note `p1` signal.
    // Params are plain f32 fields stamped at trigger time — the post-steal
    // param ramp is pure per-frame field arithmetic (current/target/step) —
    // so triggering with params, a param-RAMPING steal (here over a custom
    // 10 ms window), and rendering must all be allocation-free once the
    // pool is built.
    let spec = GraphVoiceSpec::new(
        "paramlead",
        0.05,
        vec![
            VoiceNodeSpec::Saw {
                freq: VoiceSignalRef::Freq,
            },
            VoiceNodeSpec::Constant { value: 0.7 },
            VoiceNodeSpec::Svf {
                input: VoiceSignalRef::Node(0),
                cutoff_hz: VoiceSignalRef::Param(0),
                q: VoiceSignalRef::Node(1),
                mode: SvfMode::Lowpass,
            },
            VoiceNodeSpec::Ar {
                gate: VoiceSignalRef::Gate,
                attack_s: 0.001,
                release_s: 0.05,
            },
            VoiceNodeSpec::Mul {
                left: VoiceSignalRef::Node(2),
                right: VoiceSignalRef::Node(3),
            },
        ],
        VoiceSignalRef::Node(4),
    )
    .expect("param-driven voice spec should validate")
    .with_polyphony(2)
    .expect("polyphony 2 is within bounds")
    .with_param_ramp_seconds(0.01)
    .expect("10 ms is a valid param ramp window");

    let mut bank = GraphVoiceBank::with_user_programs(SR, vec![spec]);
    let track = TrackId::new(0);
    let mut mix = vec![(0.0_f32, 0.0_f32); 1];

    let before = allocation_count();
    assert!(bank.trigger_with_params(
        "paramlead",
        track,
        2_048,
        110.0,
        0.8,
        0.0,
        [400.0, 0.0, 0.0, 0.0]
    ));
    assert!(bank.trigger_with_params(
        "paramlead",
        track,
        2_048,
        220.0,
        0.8,
        0.0,
        [4_000.0, 0.0, 0.0, 0.0]
    ));
    // Pool exhausted: the third note's steal swaps in its own params.
    assert!(bank.trigger_with_params(
        "paramlead",
        track,
        2_048,
        330.0,
        0.8,
        0.0,
        [1_000.0, 0.0, 0.0, 0.0]
    ));
    let mut energy = 0.0_f32;
    for _ in 0..4_096 {
        mix[0] = (0.0, 0.0);
        bank.render_frame(&mut mix);
        assert!(mix[0].0.is_finite() && mix[0].1.is_finite());
        energy += mix[0].0.abs() + mix[0].1.abs();
    }
    let after = allocation_count();

    assert!(energy > 0.0, "the param-driven voice should be audible");
    assert_eq!(
        after - before,
        0,
        "param-driven trigger/steal/render must not allocate after the pool is built"
    );
}

#[test]
fn breakpoint_automated_param_voice_renders_without_allocating() {
    // Per-note parameter breakpoint automation (ADR 0012): a saw through an
    // SVF lowpass whose cutoff is the automated `p1` signal. The trigger's
    // `Arc`-shipped breakpoints are stamped into the note's fixed-size ramp
    // storage; stamping (`graph_note_voice_ramps`), triggering, and the
    // interpolating playback must all be allocation-free once the pool and
    // the trigger exist.
    let spec = GraphVoiceSpec::new(
        "sweeplead",
        0.05,
        vec![
            VoiceNodeSpec::Saw {
                freq: VoiceSignalRef::Freq,
            },
            VoiceNodeSpec::Constant { value: 0.7 },
            VoiceNodeSpec::Svf {
                input: VoiceSignalRef::Node(0),
                cutoff_hz: VoiceSignalRef::Param(0),
                q: VoiceSignalRef::Node(1),
                mode: SvfMode::Lowpass,
            },
            VoiceNodeSpec::Ar {
                gate: VoiceSignalRef::Gate,
                attack_s: 0.001,
                release_s: 0.05,
            },
            VoiceNodeSpec::Mul {
                left: VoiceSignalRef::Node(2),
                right: VoiceSignalRef::Node(3),
            },
        ],
        VoiceSignalRef::Node(4),
    )
    .expect("automated param voice spec should validate");

    let mut bank = GraphVoiceBank::with_user_programs(SR, vec![spec]);
    let track = TrackId::new(0);
    let mut mix = vec![(0.0_f32, 0.0_f32); 1];

    // Built off the audio thread: the trigger owns its breakpoints behind an
    // `Arc`, exactly as the pattern side ships them.
    let breakpoints: std::sync::Arc<[VoiceParamBreakpoint]> = vec![
        VoiceParamBreakpoint::new(0.0, 400.0),
        VoiceParamBreakpoint::new(0.25, 1_200.0),
        VoiceParamBreakpoint::new(0.5, 4_000.0),
        VoiceParamBreakpoint::new(0.75, 800.0),
    ]
    .into();
    let trigger = SampleTrigger::named("sweeplead")
        .with_voice_param(0, 400.0)
        .with_voice_param_ramp(0, breakpoints);

    let before = allocation_count();
    let params = graph_note_voice_params(&trigger);
    let ramps = graph_note_voice_ramps(&trigger, 2_048, &params);
    assert!(bank.trigger_with_automation(
        "sweeplead",
        track,
        2_048,
        110.0,
        0.8,
        0.0,
        params,
        &ramps
    ));
    let mut energy = 0.0_f32;
    for _ in 0..4_096 {
        mix[0] = (0.0, 0.0);
        bank.render_frame(&mut mix);
        assert!(mix[0].0.is_finite() && mix[0].1.is_finite());
        energy += mix[0].0.abs() + mix[0].1.abs();
    }
    let after = allocation_count();

    assert!(energy > 0.0, "the automated voice should be audible");
    assert_eq!(
        after - before,
        0,
        "breakpoint stamping, trigger, and interpolating playback must not \
         allocate after the pool and trigger are built"
    );
}

#[test]
fn voice_with_svf_and_eq_peak_filters_renders_without_allocating() {
    // The new filter stages: a saw through an LFO-swept SVF lowpass (per-
    // sample coefficient updates) into a peaking EQ boost. Pool build
    // compiles and warms the voices off-thread; trigger/render must then be
    // allocation-free.
    let spec = GraphVoiceSpec::new(
        "acid",
        0.05,
        vec![
            // Saw carrier.
            VoiceNodeSpec::Saw {
                freq: VoiceSignalRef::Freq,
            },
            // Cutoff LFO: sine(2 Hz) * 600 + 900.
            VoiceNodeSpec::Constant { value: 2.0 },
            VoiceNodeSpec::Sine {
                freq: VoiceSignalRef::Node(1),
            },
            VoiceNodeSpec::Constant { value: 600.0 },
            VoiceNodeSpec::Mul {
                left: VoiceSignalRef::Node(2),
                right: VoiceSignalRef::Node(3),
            },
            VoiceNodeSpec::Constant { value: 900.0 },
            VoiceNodeSpec::Add {
                left: VoiceSignalRef::Node(4),
                right: VoiceSignalRef::Node(5),
            },
            // Swept SVF lowpass.
            VoiceNodeSpec::Constant { value: 0.7 },
            VoiceNodeSpec::Svf {
                input: VoiceSignalRef::Node(0),
                cutoff_hz: VoiceSignalRef::Node(6),
                q: VoiceSignalRef::Node(7),
                mode: SvfMode::Lowpass,
            },
            // Peaking EQ boost at 500 Hz.
            VoiceNodeSpec::Constant { value: 500.0 },
            VoiceNodeSpec::Constant { value: 1.5 },
            VoiceNodeSpec::Constant { value: 6.0 },
            VoiceNodeSpec::EqPeak {
                input: VoiceSignalRef::Node(8),
                freq_hz: VoiceSignalRef::Node(9),
                q: VoiceSignalRef::Node(10),
                gain_db: VoiceSignalRef::Node(11),
            },
            // Gate-driven envelope.
            VoiceNodeSpec::Ar {
                gate: VoiceSignalRef::Gate,
                attack_s: 0.001,
                release_s: 0.05,
            },
            VoiceNodeSpec::Mul {
                left: VoiceSignalRef::Node(12),
                right: VoiceSignalRef::Node(13),
            },
        ],
        VoiceSignalRef::Node(14),
    )
    .expect("filtered voice spec should validate");

    let mut bank = GraphVoiceBank::with_user_programs(SR, vec![spec]);
    let track = TrackId::new(0);
    let mut mix = vec![(0.0_f32, 0.0_f32); 1];

    let before = allocation_count();
    assert!(bank.trigger("acid", track, 2_048, 110.0, 0.8, 0.0));
    let mut energy = 0.0_f32;
    for _ in 0..4_096 {
        mix[0] = (0.0, 0.0);
        bank.render_frame(&mut mix);
        assert!(mix[0].0.is_finite() && mix[0].1.is_finite());
        energy += mix[0].0.abs() + mix[0].1.abs();
    }
    let after = allocation_count();

    assert!(energy > 0.0, "the filtered voice should be audible");
    assert_eq!(
        after - before,
        0,
        "SVF/EQ-filtered voices must not allocate after the pool is built"
    );
}

#[test]
fn looped_and_pitched_sample_voices_render_without_allocating() {
    // The sample-stage follow-ups (loop mode and pitch-by-frequency) reuse
    // the same Arc-shared buffer: the wrap and the per-frame reference
    // division must keep the pooled trigger/render path allocation-free.
    #[allow(clippy::cast_precision_loss)]
    let buffer: Vec<f32> = (0..480).map(|i| ((i as f32) * 0.05).sin() * 0.5).collect();
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let sample = PlaybackSample::from_mono_frames(buffer, SR as u32);

    let looper = GraphVoiceSpec::new(
        "looper",
        0.1,
        vec![
            VoiceNodeSpec::Constant { value: 1.25 },
            VoiceNodeSpec::Sample {
                gate: VoiceSignalRef::Gate,
                rate: VoiceSignalRef::Node(0),
                sample: sample.clone(),
                looped: true,
                loop_crossfade: false,
                pitch_reference_hz: None,
            },
        ],
        VoiceSignalRef::Node(1),
    )
    .expect("looped sample voice spec should validate");
    let keys = GraphVoiceSpec::new(
        "keys",
        0.1,
        vec![VoiceNodeSpec::Sample {
            gate: VoiceSignalRef::Gate,
            rate: VoiceSignalRef::Freq,
            sample,
            looped: false,
            loop_crossfade: false,
            pitch_reference_hz: Some(220.0),
        }],
        VoiceSignalRef::Node(0),
    )
    .expect("pitched sample voice spec should validate");

    let mut bank = GraphVoiceBank::with_user_programs(SR, vec![looper, keys]);
    let track = TrackId::new(0);
    let mut mix = vec![(0.0_f32, 0.0_f32); 1];

    let before = allocation_count();
    assert!(bank.trigger("looper", track, 2_048, 220.0, 0.8, 0.0));
    assert!(bank.trigger("keys", track, 2_048, 440.0, 0.8, 0.0));
    // A note below the pitched reference stamps a stretched per-note release
    // at trigger time; that arithmetic must stay allocation-free too.
    assert!(bank.trigger("keys", track, 2_048, 110.0, 0.8, 0.0));
    let mut energy = 0.0_f32;
    for _ in 0..4_096 {
        mix[0] = (0.0, 0.0);
        bank.render_frame(&mut mix);
        assert!(mix[0].0.is_finite() && mix[0].1.is_finite());
        energy += mix[0].0.abs() + mix[0].1.abs();
    }
    let after = allocation_count();

    assert!(energy > 0.0, "looped and pitched voices should be audible");
    assert_eq!(
        after - before,
        0,
        "looped/pitched sample voices must not allocate after the pool is built"
    );
}
