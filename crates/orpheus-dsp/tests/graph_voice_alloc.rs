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
    GraphVoiceSpec, Node, Processor, VoiceNodeSpec, VoiceSignalRef, adsr, bind,
    builtin_graph_voice_programs, constant, gain_node, pan, par, seq, sine,
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
