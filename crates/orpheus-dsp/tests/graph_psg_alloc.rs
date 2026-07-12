//! Allocation-discipline test for the Genesis PSG nodes.
//!
//! Construction (including the one-time volume-table regeneration) may allocate;
//! the `process()` path must not (Node contract, ADR 0004). A thread-local
//! counting allocator proves it, following the `graph_fm_alloc.rs` precedent.

// A counting `GlobalAlloc` cannot be written without `unsafe`; the blocks
// below only delegate to `System` after bumping a thread-local counter.
#![allow(unsafe_code)]

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

use orpheus_dsp::{Node, psg_noise, psg_tone};

struct CountingAllocator;

thread_local! {
    static ALLOCATION_COUNT: Cell<u64> = const { Cell::new(0) };
}

fn allocation_count() -> u64 {
    ALLOCATION_COUNT.with(Cell::get)
}

fn record_allocation() {
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
fn psg_tone_process_does_not_allocate() {
    let mut node = psg_tone(SR);
    let gate = vec![1.0_f32; BLOCK];
    let freq = vec![440.0_f32; BLOCK];
    let level = vec![1.0_f32; BLOCK];
    let mut out = vec![0.0_f32; BLOCK];

    {
        let inputs: [&[f32]; 3] = [&gate, &freq, &level];
        let mut outputs: [&mut [f32]; 1] = [&mut out];
        node.process(&inputs, &mut outputs, BLOCK);
    }

    let before = allocation_count();
    for _ in 0..64 {
        let inputs: [&[f32]; 3] = [&gate, &freq, &level];
        let mut outputs: [&mut [f32]; 1] = [&mut out];
        node.process(&inputs, &mut outputs, BLOCK);
    }
    let after = allocation_count();
    assert_eq!(after - before, 0, "psg_tone process() must not allocate");
}

#[test]
fn psg_noise_process_does_not_allocate() {
    let mut node = psg_noise(SR);
    let gate = vec![1.0_f32; BLOCK];
    let mode = vec![1.0_f32; BLOCK];
    let freq = vec![12_000.0_f32; BLOCK];
    let level = vec![1.0_f32; BLOCK];
    let mut out = vec![0.0_f32; BLOCK];

    {
        let inputs: [&[f32]; 4] = [&gate, &mode, &freq, &level];
        let mut outputs: [&mut [f32]; 1] = [&mut out];
        node.process(&inputs, &mut outputs, BLOCK);
    }

    let before = allocation_count();
    for _ in 0..64 {
        let inputs: [&[f32]; 4] = [&gate, &mode, &freq, &level];
        let mut outputs: [&mut [f32]; 1] = [&mut out];
        node.process(&inputs, &mut outputs, BLOCK);
    }
    let after = allocation_count();
    assert_eq!(after - before, 0, "psg_noise process() must not allocate");
}
