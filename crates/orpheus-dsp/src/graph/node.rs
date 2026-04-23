//! Core trait and error types for the DSP graph combinator system.

use thiserror::Error;

/// Errors raised during graph construction or compilation.
///
/// **Recovery:**
/// Most of these errors indicate logical flaws in the structural composition of your DSP graph.
/// They are typically resolved by:
/// 1. Inspecting the number of inputs and outputs of the nodes you are trying to connect using `.inputs()` and `.outputs()`.
/// 2. Injecting adapter nodes (like `passthrough` or `sum`) to balance channel counts.
/// 3. Ensuring feedback loop sizes are valid and properly delayed.
///
/// ## Examples
///
/// ```rust
/// use orpheus_dsp::{GraphError, pipe, passthrough, pulse};
///
/// // `passthrough(3)` produces 3 outputs. `pulse` requires 2 inputs.
/// // Trying to pipe them directly will cause a channel mismatch.
/// let error = pipe(passthrough(3), pulse(48000.0)).unwrap_err();
///
/// assert_eq!(
///     error,
///     GraphError::ChannelMismatch {
///         context: "pipe: a.outputs() must be <= b.inputs()",
///         left: 3,
///         right: 2,
///     }
/// );
/// ```
#[derive(Debug, Clone, Error, PartialEq, Eq)]
pub enum GraphError {
    /// Channel count mismatch between connected nodes.
    #[error("channel mismatch ({context}): left has {left}, right has {right}")]
    ChannelMismatch {
        /// Which combinator detected the mismatch.
        context: &'static str,
        /// Channel count on the left/source side.
        left: u32,
        /// Channel count on the right/target side.
        right: u32,
    },

    /// Recursive combinator has incompatible feedback path.
    #[error(
        "recursive combinator: feedback inputs ({fb_inputs}) must be <= body outputs ({body_outputs}) and feedback outputs ({fb_outputs}) must be <= body inputs ({body_inputs})"
    )]
    InvalidRecursive {
        /// Body node input count.
        body_inputs: u32,
        /// Body node output count.
        body_outputs: u32,
        /// Feedback node input count.
        fb_inputs: u32,
        /// Feedback node output count.
        fb_outputs: u32,
    },

    /// Split source must produce at least one output channel.
    #[error("split source has zero outputs")]
    EmptySplitSource,

    /// Merge target must consume at least one input channel.
    #[error("merge target has zero inputs")]
    EmptyMergeTarget,
}

/// The core block-processing trait for all DSP graph nodes.
///
/// Every node declares its input and output channel counts, processes blocks
/// of audio frames, and can be reset to initial state. Parameters flow as
/// signal inputs (Faust model): a filter's cutoff is an input channel, not a
/// configuration field.
///
/// # Contract
///
/// - `inputs` and `outputs` must not change after construction.
/// - `process` receives exactly `self.inputs()` input slices and
///   `self.outputs()` output slices, each of length `frames`.
/// - `process` must not allocate or lock.
/// - After `reset`, the node behaves identically to a freshly constructed instance.
///
/// ## Examples
///
/// ```rust
/// use orpheus_dsp::Node;
///
/// /// A node that multiplies an input by 2.0
/// struct Doubler;
///
/// impl Node for Doubler {
///     fn inputs(&self) -> u32 { 1 }
///     fn outputs(&self) -> u32 { 1 }
///
///     fn process(&mut self, inputs: &[&[f32]], outputs: &mut [&mut [f32]], frames: usize) {
///         for i in 0..frames {
///             outputs[0][i] = inputs[0][i] * 2.0;
///         }
///     }
///
///     fn reset(&mut self) {}
/// }
///
/// let mut node = Doubler;
/// let input = vec![1.0, 2.0, 3.0];
/// let mut output = vec![0.0, 0.0, 0.0];
///
/// node.process(&[&input[..]], &mut [&mut output[..]], 3);
/// assert_eq!(output, vec![2.0, 4.0, 6.0]);
/// ```
pub trait Node: Send {
    /// Number of input channels this node consumes.
    fn inputs(&self) -> u32;

    /// Number of output channels this node produces.
    fn outputs(&self) -> u32;

    /// Process a block of `frames` audio samples.
    ///
    /// `inputs` has `self.inputs()` slices, each of length >= `frames`.
    /// `outputs` has `self.outputs()` slices, each of length >= `frames`.
    fn process(&mut self, inputs: &[&[f32]], outputs: &mut [&mut [f32]], frames: usize);

    /// Reset all internal state to initial conditions.
    fn reset(&mut self);
}
