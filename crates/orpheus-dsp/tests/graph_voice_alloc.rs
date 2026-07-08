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
    GraphVoiceBank, GraphVoiceSpec, Node, Processor, TrackId, VoiceNodeSpec, VoiceSignalRef, adsr,
    bind, builtin_graph_voice_programs, constant, fdelay, gain_node, pan, par, passthrough, seq,
    sine, sum, wire,
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
    assert!(
        !bank.trigger("echoverb", track, 2_048, 220.0, 0.8, 0.0),
        "the poly-3 pool must drop a fourth simultaneous note"
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
        "bank trigger/render with feedback, delay, merge, and poly 3 must not allocate"
    );
}
