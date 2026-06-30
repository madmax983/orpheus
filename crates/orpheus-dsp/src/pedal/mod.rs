//! Virtual analog guitar pedal effects system.
//!
//! This module provides a framework for defining and executing custom, graph-based
//! DSP effects inspired by analog guitar pedals. It bridges the gap between high-level
//! declarative graphs (programs) and stateful, lock-free real-time execution.
//!
//! # Core Concepts
//!
//! - **Program (`program`):** The declarative, immutable definition of a pedal's signal routing and processing nodes.
//! - **Runtime (`runtime`):** The stateful execution engine that instantiates a program and processes audio frame-by-frame.
//!
//! # Example
//!
//! ```rust
//! use orpheus_dsp::{PedalNode, PedalGraphProgram, NodeRef, SignalKind};
//!
//! // Create a simple pedal program that just passes input to output
//! let input_node = NodeRef::Input;
//! let program = PedalGraphProgram::new(vec![], input_node);
//!
//! assert!(program.is_bypass());
//! ```

mod program;
mod runtime;

pub use program::{
    ClipModel, FilterMode, NodeRef, PedalGraphProgram, PedalNode, PedalNodeKind, PedalProgram,
    PedalStage, PreampModel, SignalKind, ToneModel,
};
pub use runtime::{PEDAL_CONTROL_INTERVAL_SAMPLES, PedalInstance};
