//! Ergonomic helpers built on top of the core combinators.
//!
//! - [`pipe`]: chains two nodes, automatically threading extra parameter inputs
//! - [`bind`]: fixes specific input channels to constant values

use std::fmt;

use super::combinators::{Seq, par, seq};
use super::node::{GraphError, Node};
use super::primitives::passthrough;
use smallvec::SmallVec;

// ---------------------------------------------------------------------------
// pipe
// ---------------------------------------------------------------------------

/// Pipes `a` into `b`, connecting `a`'s outputs to `b`'s first inputs.
///
/// If `b` requires more inputs than `a` produces, the extra inputs become
/// additional external inputs (appended after `a`'s inputs). This eliminates
/// the manual `par(a, passthrough(N))` boilerplate when chaining nodes with
/// different channel counts.
///
/// ```text
/// a: (a_in) → (a_out)
/// b: (b_in) → (b_out)     where a_out <= b_in
///
/// pipe(a, b): (a_in + b_in - a_out) → (b_out)
/// ```
///
/// # Errors
///
/// Returns [`GraphError::ChannelMismatch`] if `a.outputs() > b.inputs()`.
pub fn pipe(a: impl Node + 'static, b: impl Node + 'static) -> Result<Seq, GraphError> {
    let a_outs = a.outputs();
    let b_ins = b.inputs();

    if a_outs > b_ins {
        return Err(GraphError::ChannelMismatch {
            context: "pipe: a.outputs() must be <= b.inputs()",
            left: a_outs,
            right: b_ins,
        });
    }

    let extra = b_ins - a_outs;
    if extra == 0 {
        seq(a, b)
    } else {
        seq(par(a, passthrough(extra)), b)
    }
}

// ---------------------------------------------------------------------------
// bind
// ---------------------------------------------------------------------------

/// A node that fixes specific input channels to constant values.
///
/// Bound channels are fed internally from constants; unbound channels become
/// the node's external inputs, renumbered contiguously.
pub struct Bind {
    inner: Box<dyn Node>,
    /// For each of the inner node's input channels: `Some(value)` if bound,
    /// `None` if it remains an external input.
    bindings: Vec<Option<f32>>,
    /// Maps external input index → inner channel index.
    free_channels: Vec<u32>,
    /// Scratch buffer: full input for the inner node (one Vec per channel).
    full_input: Vec<Vec<f32>>,
}

impl fmt::Debug for Bind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Bind")
            .field("inputs", &self.inputs())
            .field("outputs", &self.outputs())
            .field(
                "bound_count",
                &self.bindings.iter().filter(|b| b.is_some()).count(),
            )
            .finish_non_exhaustive()
    }
}

impl Node for Bind {
    fn inputs(&self) -> u32 {
        #[allow(clippy::cast_possible_truncation)] // bounded by inner node's input count (u32)
        {
            self.free_channels.len() as u32
        }
    }

    fn outputs(&self) -> u32 {
        self.inner.outputs()
    }

    fn process(&mut self, inputs: &[&[f32]], outputs: &mut [&mut [f32]], frames: usize) {
        // Ensure scratch is large enough.
        for buf in &mut self.full_input {
            if buf.len() < frames {
                buf.resize(frames, 0.0);
            }
        }

        // Fill bound channels with constant values, free channels from external inputs.
        for (ch, binding) in self.bindings.iter().enumerate() {
            if let Some(value) = binding {
                self.full_input[ch][..frames].fill(*value);
            }
        }
        for (ext_idx, &inner_ch) in self.free_channels.iter().enumerate() {
            self.full_input[inner_ch as usize][..frames]
                .copy_from_slice(&inputs[ext_idx][..frames]);
        }

        // Process the inner node with the fully-assembled input.

        // Optimization: Stack-allocate scratch buffers to avoid heap allocations on hot paths.
        let input_refs: SmallVec<[&[f32]; 8]> =
            self.full_input.iter().map(|v| &v[..frames]).collect();
        self.inner.process(&input_refs, outputs, frames);
    }

    fn reset(&mut self) {
        self.inner.reset();
    }
}

/// Binds specific input channels of a node to constant values.
///
/// Bound channels are removed from the external interface; unbound channels
/// are renumbered contiguously starting from 0.
///
/// # Examples
///
/// ```
/// use orpheus_dsp::{bind, one_pole, Node};
///
/// // one_pole has 2 inputs (audio, cutoff_hz)
/// let filter = one_pole(48_000.0);
/// assert_eq!(filter.inputs(), 2);
///
/// // Bind cutoff_hz (index 1) to 1000.0 Hz
/// let bound_filter = bind(filter, &[(1, 1000.0)]).unwrap();
///
/// // Now it has only 1 input (audio)
/// assert_eq!(bound_filter.inputs(), 1);
/// ```
///
/// # Errors
///
/// Returns [`GraphError::ChannelMismatch`] if any binding index is out of range.
pub fn bind(node: impl Node + 'static, bindings: &[(u32, f32)]) -> Result<Bind, GraphError> {
    let node = Box::new(node);
    let n_inputs = node.inputs();

    let mut binding_map: Vec<Option<f32>> = vec![None; n_inputs as usize];
    for &(ch, value) in bindings {
        if ch >= n_inputs {
            return Err(GraphError::ChannelMismatch {
                context: "bind: channel index out of range",
                left: ch,
                right: n_inputs,
            });
        }
        binding_map[ch as usize] = Some(value);
    }

    let free_channels: Vec<u32> = (0..n_inputs)
        .filter(|&ch| binding_map[ch as usize].is_none())
        .collect();
    let full_input = vec![Vec::new(); n_inputs as usize];

    Ok(Bind {
        inner: node,
        bindings: binding_map,
        free_channels,
        full_input,
    })
}
