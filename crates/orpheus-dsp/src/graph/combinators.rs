//! Faust-style block diagram combinators for composing DSP graphs.
//!
//! Five operators:
//! - [`Seq`] (sequential): output of A feeds input of B
//! - [`Par`] (parallel): A and B side-by-side, no connection
//! - [`Spl`] (split): A's outputs duplicated into B's inputs
//! - [`Mrg`] (merge): A's outputs summed in groups into B's inputs
//! - [`Rec`] (recursive): feedback loop with one-sample delay

use smallvec::SmallVec;
use std::fmt;

use super::node::{GraphError, Node};

/// Ensure every buffer in the vec is at least `min_len` long.
fn grow_scratch(bufs: &mut [Vec<f32>], min_len: usize) {
    for buf in bufs {
        if buf.len() < min_len {
            buf.resize(min_len, 0.0);
        }
    }
}

// ---------------------------------------------------------------------------
// Sequential
// ---------------------------------------------------------------------------

/// Sequential composition: output of `a` feeds input of `b`.
///
/// Requires `a.outputs() == b.inputs()`.
pub struct Seq {
    a: Box<dyn Node>,
    b: Box<dyn Node>,
    scratch_data: Vec<Vec<f32>>,
}

impl fmt::Debug for Seq {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Seq")
            .field("inputs", &self.inputs())
            .field("outputs", &self.outputs())
            .finish()
    }
}

impl Node for Seq {
    fn inputs(&self) -> u32 {
        self.a.inputs()
    }
    fn outputs(&self) -> u32 {
        self.b.outputs()
    }
    /// Optimization: Internal buffers map slice references using `SmallVec` to avoid
    /// per-frame heap allocations on the hot audio thread.
    fn process(&mut self, inputs: &[&[f32]], outputs: &mut [&mut [f32]], frames: usize) {
        grow_scratch(&mut self.scratch_data, frames);

        {
            // Optimization: Stack-allocate reference slices using SmallVec to avoid
            // real-time heap allocations (`Vec::new()`) in the hot audio processing loop.
            let mut scratch_mut: SmallVec<[&mut [f32]; 8]> = self
                .scratch_data
                .iter_mut()
                .map(|v| &mut v[..frames])
                .collect();
            self.a.process(inputs, &mut scratch_mut, frames);
        }

        let scratch_ref: SmallVec<[&[f32]; 8]> =
            self.scratch_data.iter().map(|v| &v[..frames]).collect();
        self.b.process(&scratch_ref, outputs, frames);
    }
    fn reset(&mut self) {
        self.a.reset();
        self.b.reset();
        for buf in &mut self.scratch_data {
            buf.fill(0.0);
        }
    }
}

/// Creates a sequential composition: `a >> b`.
///
/// # Errors
///
/// Returns [`GraphError::ChannelMismatch`] if `a.outputs() != b.inputs()`.
pub fn seq(a: impl Node + 'static, b: impl Node + 'static) -> Result<Seq, GraphError> {
    let a = Box::new(a);
    let b = Box::new(b);
    if a.outputs() != b.inputs() {
        return Err(GraphError::ChannelMismatch {
            context: "seq: a.outputs() must equal b.inputs()",
            left: a.outputs(),
            right: b.inputs(),
        });
    }
    let scratch_data = vec![Vec::new(); a.outputs() as usize];
    Ok(Seq { a, b, scratch_data })
}

// ---------------------------------------------------------------------------
// Parallel
// ---------------------------------------------------------------------------

/// Parallel composition: A and B side-by-side, no connection between them.
///
/// Always valid. Inputs and outputs are concatenated.
pub struct Par {
    a: Box<dyn Node>,
    b: Box<dyn Node>,
}

impl fmt::Debug for Par {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Par")
            .field("inputs", &self.inputs())
            .field("outputs", &self.outputs())
            .finish()
    }
}

impl Node for Par {
    fn inputs(&self) -> u32 {
        self.a.inputs() + self.b.inputs()
    }
    fn outputs(&self) -> u32 {
        self.a.outputs() + self.b.outputs()
    }
    /// Optimization: Internal buffers map slice references using `SmallVec` to avoid
    /// per-frame heap allocations on the hot audio thread.
    fn process(&mut self, inputs: &[&[f32]], outputs: &mut [&mut [f32]], frames: usize) {
        let a_ins = self.a.inputs() as usize;
        let a_outs = self.a.outputs() as usize;

        let (a_inputs, b_inputs) = inputs.split_at(a_ins);
        let (a_outputs, b_outputs) = outputs.split_at_mut(a_outs);

        self.a.process(a_inputs, a_outputs, frames);
        self.b.process(b_inputs, b_outputs, frames);
    }
    fn reset(&mut self) {
        self.a.reset();
        self.b.reset();
    }
}

/// Creates a parallel composition. Always succeeds.
pub fn par(a: impl Node + 'static, b: impl Node + 'static) -> Par {
    Par {
        a: Box::new(a),
        b: Box::new(b),
    }
}

// ---------------------------------------------------------------------------
// Split
// ---------------------------------------------------------------------------

/// Split composition: A's outputs are duplicated cyclically into B's inputs.
///
/// Requires `a.outputs() > 0` and `b.inputs() % a.outputs() == 0`.
pub struct Spl {
    a: Box<dyn Node>,
    b: Box<dyn Node>,
    scratch_data: Vec<Vec<f32>>,
}

impl fmt::Debug for Spl {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Spl")
            .field("inputs", &self.inputs())
            .field("outputs", &self.outputs())
            .finish()
    }
}

impl Node for Spl {
    fn inputs(&self) -> u32 {
        self.a.inputs()
    }
    fn outputs(&self) -> u32 {
        self.b.outputs()
    }
    /// Optimization: Internal buffers map slice references using `SmallVec` to avoid
    /// per-frame heap allocations on the hot audio thread.
    fn process(&mut self, inputs: &[&[f32]], outputs: &mut [&mut [f32]], frames: usize) {
        grow_scratch(&mut self.scratch_data, frames);

        {
            // Optimization: Stack-allocate reference slices using SmallVec to avoid
            // real-time heap allocations (`Vec::new()`) in the hot audio processing loop.
            let mut scratch_mut: SmallVec<[&mut [f32]; 8]> = self
                .scratch_data
                .iter_mut()
                .map(|v| &mut v[..frames])
                .collect();
            self.a.process(inputs, &mut scratch_mut, frames);
        }

        let a_outs = self.a.outputs() as usize;
        let b_ins = self.b.inputs() as usize;
        let b_input_refs: SmallVec<[&[f32]; 8]> = (0..b_ins)
            .map(|i| &self.scratch_data[i % a_outs][..frames])
            .collect();

        self.b.process(&b_input_refs, outputs, frames);
    }
    fn reset(&mut self) {
        self.a.reset();
        self.b.reset();
        for buf in &mut self.scratch_data {
            buf.fill(0.0);
        }
    }
}

/// Creates a split composition: `a <: b`.
///
/// # Errors
///
/// Returns [`GraphError::EmptySplitSource`] if `a.outputs() == 0`.
/// Returns [`GraphError::ChannelMismatch`] if `!b.inputs().is_multiple_of(a.outputs())`.
pub fn split(a: impl Node + 'static, b: impl Node + 'static) -> Result<Spl, GraphError> {
    let a = Box::new(a);
    let b = Box::new(b);
    if a.outputs() == 0 {
        return Err(GraphError::EmptySplitSource);
    }
    if !b.inputs().is_multiple_of(a.outputs()) {
        return Err(GraphError::ChannelMismatch {
            context: "split: b.inputs() must be divisible by a.outputs()",
            left: a.outputs(),
            right: b.inputs(),
        });
    }
    let scratch_data = vec![Vec::new(); a.outputs() as usize];
    Ok(Spl { a, b, scratch_data })
}

// ---------------------------------------------------------------------------
// Merge
// ---------------------------------------------------------------------------

/// Merge composition: A's outputs are summed in groups into B's inputs.
///
/// Requires `b.inputs() > 0` and `a.outputs() % b.inputs() == 0`.
pub struct Mrg {
    a: Box<dyn Node>,
    b: Box<dyn Node>,
    a_scratch: Vec<Vec<f32>>,
    sum_scratch: Vec<Vec<f32>>,
}

impl fmt::Debug for Mrg {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Mrg")
            .field("inputs", &self.inputs())
            .field("outputs", &self.outputs())
            .finish()
    }
}

impl Node for Mrg {
    fn inputs(&self) -> u32 {
        self.a.inputs()
    }
    fn outputs(&self) -> u32 {
        self.b.outputs()
    }
    /// Optimization: Internal buffers map slice references using `SmallVec` to avoid
    /// per-frame heap allocations on the hot audio thread.
    fn process(&mut self, inputs: &[&[f32]], outputs: &mut [&mut [f32]], frames: usize) {
        let a_outs = self.a.outputs() as usize;
        let b_ins = self.b.inputs() as usize;
        let group_size = a_outs / b_ins;

        grow_scratch(&mut self.a_scratch, frames);
        grow_scratch(&mut self.sum_scratch, frames);

        {
            let mut a_mut: SmallVec<[&mut [f32]; 8]> = self
                .a_scratch
                .iter_mut()
                .map(|v| &mut v[..frames])
                .collect();
            self.a.process(inputs, &mut a_mut, frames);
        }

        for (g, sum_buf) in self.sum_scratch.iter_mut().enumerate() {
            sum_buf[..frames].fill(0.0);
            for k in 0..group_size {
                let src = &self.a_scratch[g * group_size + k];
                for i in 0..frames {
                    sum_buf[i] += src[i];
                }
            }
        }

        let sum_refs: SmallVec<[&[f32]; 8]> =
            self.sum_scratch.iter().map(|v| &v[..frames]).collect();
        self.b.process(&sum_refs, outputs, frames);
    }
    fn reset(&mut self) {
        self.a.reset();
        self.b.reset();
        for buf in &mut self.a_scratch {
            buf.fill(0.0);
        }
        for buf in &mut self.sum_scratch {
            buf.fill(0.0);
        }
    }
}

/// Creates a merge composition: `a :> b`.
///
/// # Errors
///
/// Returns [`GraphError::EmptyMergeTarget`] if `b.inputs() == 0`.
/// Returns [`GraphError::ChannelMismatch`] if `!a.outputs().is_multiple_of(b.inputs())`.
pub fn merge(a: impl Node + 'static, b: impl Node + 'static) -> Result<Mrg, GraphError> {
    let a = Box::new(a);
    let b = Box::new(b);
    if b.inputs() == 0 {
        return Err(GraphError::EmptyMergeTarget);
    }
    if !a.outputs().is_multiple_of(b.inputs()) {
        return Err(GraphError::ChannelMismatch {
            context: "merge: a.outputs() must be divisible by b.inputs()",
            left: a.outputs(),
            right: b.inputs(),
        });
    }
    let a_scratch = vec![Vec::new(); a.outputs() as usize];
    let sum_scratch = vec![Vec::new(); b.inputs() as usize];
    Ok(Mrg {
        a,
        b,
        a_scratch,
        sum_scratch,
    })
}

// ---------------------------------------------------------------------------
// Recursive
// ---------------------------------------------------------------------------

/// Recursive composition with one-sample delay feedback.
///
/// Given `body` (m inputs, n outputs) and `feedback` (p inputs, q outputs):
/// - `p <= n`: feedback reads from the first `p` outputs of body
/// - `q <= m`: feedback writes (delayed one sample) into the first `q` inputs of body
/// - External inputs: `m - q` (the body inputs not fed by feedback)
/// - External outputs: `n` (all body outputs are visible)
pub struct Rec {
    body: Box<dyn Node>,
    feedback: Box<dyn Node>,
    delay_buf: Vec<f32>,
    body_in_scratch: Vec<Vec<f32>>,
    body_out_scratch: Vec<Vec<f32>>,
    fb_out_scratch: Vec<Vec<f32>>,
    body_inputs: u32,
    body_outputs: u32,
    fb_inputs: u32,
    fb_outputs: u32,
}

impl fmt::Debug for Rec {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Rec")
            .field("inputs", &self.inputs())
            .field("outputs", &self.outputs())
            .field(
                "body",
                &format_args!("({} -> {})", self.body_inputs, self.body_outputs),
            )
            .field(
                "feedback",
                &format_args!("({} -> {})", self.fb_inputs, self.fb_outputs),
            )
            .finish_non_exhaustive()
    }
}

impl Node for Rec {
    fn inputs(&self) -> u32 {
        self.body_inputs - self.fb_outputs
    }
    fn outputs(&self) -> u32 {
        self.body_outputs
    }
    /// Optimization: Internal buffers map slice references using `SmallVec` to avoid
    /// per-frame heap allocations on the hot audio thread.
    fn process(&mut self, inputs: &[&[f32]], outputs: &mut [&mut [f32]], frames: usize) {
        let m = self.body_inputs as usize;
        let n = self.body_outputs as usize;
        let p = self.fb_inputs as usize;
        let q = self.fb_outputs as usize;
        let ext_ins = m - q;

        for frame in 0..frames {
            // Build body input: first q channels from feedback delay, rest from external.
            for ch in 0..q {
                self.body_in_scratch[ch][0] = self.delay_buf[ch];
            }
            for (ch, inp) in inputs.iter().enumerate().take(ext_ins) {
                self.body_in_scratch[q + ch][0] = inp[frame];
            }

            // Process body for 1 frame.
            let body_in_refs: SmallVec<[&[f32]; 8]> =
                self.body_in_scratch[..m].iter().map(|v| &v[..1]).collect();
            {
                let mut body_out_refs: SmallVec<[&mut [f32]; 8]> = self.body_out_scratch[..n]
                    .iter_mut()
                    .map(|v| &mut v[..1])
                    .collect();
                self.body.process(&body_in_refs, &mut body_out_refs, 1);
            }

            // Copy body outputs to external outputs.
            for (ch, out) in outputs.iter_mut().enumerate().take(n) {
                out[frame] = self.body_out_scratch[ch][0];
            }

            // Process feedback: reads first p body outputs, produces q outputs.
            let fb_in_refs: SmallVec<[&[f32]; 8]> =
                self.body_out_scratch[..p].iter().map(|v| &v[..1]).collect();
            {
                let mut fb_out_refs: SmallVec<[&mut [f32]; 8]> = self.fb_out_scratch[..q]
                    .iter_mut()
                    .map(|v| &mut v[..1])
                    .collect();
                self.feedback.process(&fb_in_refs, &mut fb_out_refs, 1);
            }

            // Store feedback output in delay buffer for next frame.
            for ch in 0..q {
                self.delay_buf[ch] = self.fb_out_scratch[ch][0];
            }
        }
    }
    fn reset(&mut self) {
        self.body.reset();
        self.feedback.reset();
        self.delay_buf.fill(0.0);
    }
}

/// Creates a recursive composition: `body ~ feedback`.
///
/// # Errors
///
/// Returns [`GraphError::InvalidRecursive`] if `fb.inputs() > body.outputs()`
/// or `fb.outputs() > body.inputs()`.
pub fn feedback(body: impl Node + 'static, fb: impl Node + 'static) -> Result<Rec, GraphError> {
    let body = Box::new(body);
    let feedback = Box::new(fb);
    let m = body.inputs();
    let n = body.outputs();
    let p = feedback.inputs();
    let q = feedback.outputs();

    if p > n || q > m {
        return Err(GraphError::InvalidRecursive {
            body_inputs: m,
            body_outputs: n,
            fb_inputs: p,
            fb_outputs: q,
        });
    }

    let delay_buf = vec![0.0_f32; q as usize];
    let body_in_scratch = vec![vec![0.0_f32; 1]; m as usize];
    let body_out_scratch = vec![vec![0.0_f32; 1]; n as usize];
    let fb_out_scratch = vec![vec![0.0_f32; 1]; q as usize];

    Ok(Rec {
        body,
        feedback,
        delay_buf,
        body_in_scratch,
        body_out_scratch,
        fb_out_scratch,
        body_inputs: m,
        body_outputs: n,
        fb_inputs: p,
        fb_outputs: q,
    })
}
