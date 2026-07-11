//! Allocation-discipline test for the Genesis FM node.
//!
//! Construction (including the one-time ROM regeneration) may allocate; the
//! `process()` path must not (Node contract, ADR 0004). A thread-local counting
//! allocator proves it, following the `graph_chiptune_alloc.rs` precedent.

// A counting `GlobalAlloc` cannot be written without `unsafe`; the blocks
// below only delegate to `System` after bumping a thread-local counter.
#![allow(unsafe_code)]

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

use orpheus_dsp::{Node, drum, fm_genesis, lead};

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

fn assert_process_zero_alloc(mut node: impl Node) {
    let gate = vec![1.0_f32; BLOCK];
    let freq = vec![220.0_f32; BLOCK];
    let bright = vec![1.0_f32; BLOCK];
    let fb = vec![0.5_f32; BLOCK];
    let mut out = vec![0.0_f32; BLOCK];

    // Warm up (also forces any lazy init to have already happened at construction).
    {
        let inputs: [&[f32]; 4] = [&gate, &freq, &bright, &fb];
        let mut outputs: [&mut [f32]; 1] = [&mut out];
        node.process(&inputs, &mut outputs, BLOCK);
    }

    let before = allocation_count();
    for _ in 0..64 {
        let inputs: [&[f32]; 4] = [&gate, &freq, &bright, &fb];
        let mut outputs: [&mut [f32]; 1] = [&mut out];
        node.process(&inputs, &mut outputs, BLOCK);
    }
    let after = allocation_count();
    assert_eq!(after - before, 0, "fm_genesis process() must not allocate");
}

#[test]
fn fm_genesis_process_does_not_allocate() {
    assert_process_zero_alloc(fm_genesis(SR, lead()));
}

#[test]
fn fm_genesis_ssg_eg_process_does_not_allocate() {
    // The `drum` preset exercises the SSG-EG state machine and full LFO path.
    assert_process_zero_alloc(fm_genesis(SR, drum()));
}
