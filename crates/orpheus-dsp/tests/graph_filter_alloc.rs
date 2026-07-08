//! Allocation-discipline tests for the bare filter nodes (`svf`, `biquad`).
//!
//! Construction may allocate; the `process()` path must not (Node contract,
//! ADR 0004). A thread-local counting allocator proves it, following the
//! graph\_voice\_alloc.rs precedent.

// A counting `GlobalAlloc` cannot be written without `unsafe`; the blocks
// below only delegate to `System` after bumping a thread-local counter.
#![allow(unsafe_code)]

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

use orpheus_dsp::{BiquadMode, Node, biquad, svf};

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
const BLOCK: usize = 256;

#[test]
fn svf_process_does_not_allocate() {
    let mut node = svf(SR);
    let audio = vec![0.5_f32; BLOCK];
    let cutoff = vec![1_200.0_f32; BLOCK];
    let q = vec![4.0_f32; BLOCK];
    let mut lp = vec![0.0_f32; BLOCK];
    let mut hp = vec![0.0_f32; BLOCK];
    let mut bp = vec![0.0_f32; BLOCK];
    let mut notch = vec![0.0_f32; BLOCK];

    let before = allocation_count();
    for _ in 0..64 {
        let inputs: [&[f32]; 3] = [&audio, &cutoff, &q];
        let mut outputs: [&mut [f32]; 4] = [&mut lp, &mut hp, &mut bp, &mut notch];
        node.process(&inputs, &mut outputs, BLOCK);
    }
    let after = allocation_count();

    assert_eq!(after - before, 0, "svf process() must not allocate");
}

#[test]
fn biquad_process_does_not_allocate_in_any_mode() {
    let audio = vec![0.5_f32; BLOCK];
    let freq = vec![1_200.0_f32; BLOCK];
    let q = vec![4.0_f32; BLOCK];
    let gain = vec![6.0_f32; BLOCK];
    let mut out = vec![0.0_f32; BLOCK];

    for mode in [
        BiquadMode::Lowpass,
        BiquadMode::Highpass,
        BiquadMode::Bandpass,
        BiquadMode::Notch,
    ] {
        let mut node = biquad(SR, mode);
        let before = allocation_count();
        for _ in 0..64 {
            let inputs: [&[f32]; 3] = [&audio, &freq, &q];
            let mut outputs: [&mut [f32]; 1] = [&mut out];
            node.process(&inputs, &mut outputs, BLOCK);
        }
        let after = allocation_count();
        assert_eq!(
            after - before,
            0,
            "biquad {mode:?} process() must not allocate"
        );
    }

    let mut node = biquad(SR, BiquadMode::Peaking);
    let before = allocation_count();
    for _ in 0..64 {
        let inputs: [&[f32]; 4] = [&audio, &freq, &q, &gain];
        let mut outputs: [&mut [f32]; 1] = [&mut out];
        node.process(&inputs, &mut outputs, BLOCK);
    }
    let after = allocation_count();
    assert_eq!(
        after - before,
        0,
        "biquad Peaking process() must not allocate"
    );
}
