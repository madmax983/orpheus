//! Allocation-discipline tests for the NES chiptune nodes.
//!
//! Construction may allocate; the `process()` path must not (Node contract,
//! ADR 0004). A thread-local counting allocator proves it, following the
//! graph\_filter\_alloc.rs precedent.

// A counting `GlobalAlloc` cannot be written without `unsafe`; the blocks
// below only delegate to `System` after bumping a thread-local counter.
#![allow(unsafe_code)]

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

use orpheus_dsp::{Node, noise_nes, pulse_nes, tri_nes};

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
fn pulse_nes_process_does_not_allocate() {
    let mut node = pulse_nes(SR);
    let freq = vec![440.0_f32; BLOCK];
    let duty = vec![2.0_f32; BLOCK];
    let volume = vec![0.75_f32; BLOCK];
    let mut out = vec![0.0_f32; BLOCK];

    let before = allocation_count();
    for _ in 0..64 {
        let inputs: [&[f32]; 3] = [&freq, &duty, &volume];
        let mut outputs: [&mut [f32]; 1] = [&mut out];
        node.process(&inputs, &mut outputs, BLOCK);
    }
    let after = allocation_count();
    assert_eq!(after - before, 0, "pulse_nes process() must not allocate");
}

#[test]
fn tri_nes_process_does_not_allocate() {
    let mut node = tri_nes(SR);
    let freq = vec![220.0_f32; BLOCK];
    let mut out = vec![0.0_f32; BLOCK];

    let before = allocation_count();
    for _ in 0..64 {
        let inputs: [&[f32]; 1] = [&freq];
        let mut outputs: [&mut [f32]; 1] = [&mut out];
        node.process(&inputs, &mut outputs, BLOCK);
    }
    let after = allocation_count();
    assert_eq!(after - before, 0, "tri_nes process() must not allocate");
}

#[test]
fn noise_nes_process_does_not_allocate() {
    let mut node = noise_nes(SR);
    let mode = vec![0.0_f32; BLOCK];
    let freq = vec![8_000.0_f32; BLOCK];
    let volume = vec![1.0_f32; BLOCK];
    let mut out = vec![0.0_f32; BLOCK];

    let before = allocation_count();
    for _ in 0..64 {
        let inputs: [&[f32]; 3] = [&mode, &freq, &volume];
        let mut outputs: [&mut [f32]; 1] = [&mut out];
        node.process(&inputs, &mut outputs, BLOCK);
    }
    let after = allocation_count();
    assert_eq!(after - before, 0, "noise_nes process() must not allocate");
}
