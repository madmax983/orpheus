//! Processor: compiled graph wrapper for zero-allocation rendering.
//!
//! The `Processor` owns a graph (any `Node`) and serves as the compilation
//! boundary. Internal scratch buffers within combinator nodes are allocated
//! at construction or on first `process` call; subsequent calls are
//! allocation-free.

use std::fmt;

use super::node::Node;

/// A compiled DSP graph ready for audio-thread rendering.
///
/// Wraps any [`Node`] and documents the boundary between graph construction
/// (which may allocate) and rendering (which must not). The first call to
/// `process` may trigger one-time scratch buffer growth inside nested
/// combinators; all subsequent calls with the same or smaller `frames` are
/// allocation-free.
pub struct Processor {
    root: Box<dyn Node>,
}

impl fmt::Debug for Processor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Processor")
            .field("inputs", &self.root.inputs())
            .field("outputs", &self.root.outputs())
            .finish_non_exhaustive()
    }
}

impl Processor {
    /// Compile a graph into a `Processor`.
    pub fn new(root: impl Node + 'static) -> Self {
        Self {
            root: Box::new(root),
        }
    }
}

impl Node for Processor {
    fn inputs(&self) -> u32 {
        self.root.inputs()
    }

    fn outputs(&self) -> u32 {
        self.root.outputs()
    }

    fn process(&mut self, inputs: &[&[f32]], outputs: &mut [&mut [f32]], frames: usize) {
        self.root.process(inputs, outputs, frames);
    }

    fn reset(&mut self) {
        self.root.reset();
    }
}
