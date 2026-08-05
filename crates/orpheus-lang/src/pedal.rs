//! Language-level parsing and validation for pedal graphs.
//!
//! This module provides the bridge between the user's custom syntax for defining
//! analog pedal effects (using the `Effect` language block) and the lower-level
//! `orpheus_dsp` graph structures.
//!
//! It is responsible for parsing mathematical expressions, topologically sorting
//! effect nodes, handling recursive feedback paths, and validating that nodes
//! don't mix up control and audio rate signals incorrectly.

#![allow(clippy::unnested_or_patterns)]
#![allow(clippy::elidable_lifetime_names)]
#![allow(
    clippy::suboptimal_flops,
    clippy::missing_const_for_fn,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::manual_map,
    clippy::redundant_closure_for_method_calls,
    clippy::default_constructed_unit_structs,
    clippy::needless_pass_by_value,
    clippy::unused_self
)]
use core::fmt::{self, Display, Formatter};
use std::collections::{BTreeMap, BTreeSet};

use comfy_table::Cell;
use crossterm::style::Stylize;

use crate::ast::{BinaryOp, Expr, GraphBinding, StepOp};
use crate::error::EvalError;
use crate::explain::Explain;

/// The coarse signal domain understood by the pedal DSL.
#[derive(Clone, Debug, Eq, PartialEq)]
/// Resolution constraint for graph paths.
pub enum SignalKind {
    /// Runs per-sample.
    Audio,
    /// Runs per-block.
    Control,
}

impl Display for SignalKind {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Audio => formatter.write_str("Audio"),
            Self::Control => formatter.write_str("Control"),
        }
    }
}

/// The node categories used by the validated pedal plan.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PedalNodeKind {
    Input,
    Reference,
    Constant,
    Binary,
    Stage,
    Mix,
    Feedback,
    Output,
}

impl Display for PedalNodeKind {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Input => formatter.write_str("input"),
            Self::Reference => formatter.write_str("reference"),
            Self::Constant => formatter.write_str("constant"),
            Self::Binary => formatter.write_str("binary"),
            Self::Stage => formatter.write_str("stage"),
            Self::Mix => formatter.write_str("mix"),
            Self::Feedback => formatter.write_str("feedback"),
            Self::Output => formatter.write_str("output"),
        }
    }
}

/// A source-level pedal graph wrapper.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PedalGraph {
    source: String,
}

impl PedalGraph {
    #[doc(hidden)]
    #[must_use]
    pub fn new(source: impl Into<String>) -> Self {
        Self {
            source: source.into(),
        }
    }

    #[doc(hidden)]
    #[must_use]
    pub fn source(&self) -> &str {
        &self.source
    }

    #[doc(hidden)]
    #[must_use]
    pub fn format_source(&self) -> String {
        self.source.clone()
    }
}

/// A validated node in the internal pedal plan.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ValidatedPedalNode {
    signal_kind: SignalKind,
    kind: PedalNodeKind,
    summary: String,
}

impl ValidatedPedalNode {
    /// Constructs a validated node with an associated descriptive summary.
    ///
    /// # Examples
    /// ```
    /// use orpheus_lang::{ValidatedPedalNode, SignalKind, PedalNodeKind};
    /// let node = ValidatedPedalNode::new(SignalKind::Control, PedalNodeKind::Input, "Control input");
    /// ```
    #[must_use]
    pub fn new(signal_kind: SignalKind, kind: PedalNodeKind, summary: impl Into<String>) -> Self {
        Self {
            signal_kind,
            kind,
            summary: summary.into(),
        }
    }

    #[doc(hidden)]
    #[must_use]
    pub const fn signal_kind(&self) -> &SignalKind {
        &self.signal_kind
    }

    /// The operational category of this node in the graph.
    #[must_use]
    pub const fn kind(&self) -> &PedalNodeKind {
        &self.kind
    }

    /// A short, human-readable description of the node\'s configuration.
    #[must_use]
    pub fn summary(&self) -> &str {
        &self.summary
    }
}

/// A validated let-bound signal inside the pedal plan.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ValidatedPedalBinding {
    name: String,
    node: ValidatedPedalNode,
}

impl ValidatedPedalBinding {
    /// Pairs a valid graph node with an explicit identifier.
    ///
    /// # Examples
    /// ```
    /// use orpheus_lang::{ValidatedPedalBinding, ValidatedPedalNode, SignalKind, PedalNodeKind};
    /// let node = ValidatedPedalNode::new(SignalKind::Control, PedalNodeKind::Input, "Control input");
    /// let binding = ValidatedPedalBinding::new("input", node);
    /// ```
    #[must_use]
    pub fn new(name: impl Into<String>, node: ValidatedPedalNode) -> Self {
        Self {
            name: name.into(),
            node,
        }
    }

    /// The identifier bound to the node.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The underlying verified pedal graph node.
    #[must_use]
    pub const fn node(&self) -> &ValidatedPedalNode {
        &self.node
    }
}

/// A validated pedal plan, ready for later lowering or rendering.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ValidatedPedalPlan {
    signal_kind: SignalKind,
    bindings: Vec<ValidatedPedalBinding>,
    result: ValidatedPedalNode,
}

impl ValidatedPedalPlan {
    #[doc(hidden)]
    #[must_use]
    pub fn new(bindings: Vec<ValidatedPedalBinding>, result: ValidatedPedalNode) -> Self {
        Self {
            signal_kind: result.signal_kind().clone(),
            bindings,
            result,
        }
    }

    #[doc(hidden)]
    #[must_use]
    pub const fn signal_kind(&self) -> &SignalKind {
        &self.signal_kind
    }

    #[doc(hidden)]
    #[must_use]
    pub fn bindings(&self) -> &[ValidatedPedalBinding] {
        &self.bindings
    }

    #[doc(hidden)]
    #[must_use]
    pub const fn result(&self) -> &ValidatedPedalNode {
        &self.result
    }
}

/// The language-side value for a pedal graph.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PedalValue {
    graph: PedalGraph,
    plan: ValidatedPedalPlan,
}

impl PedalValue {
    /// Instantiate a pedal value combining AST and plan.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use orpheus_lang::{PedalGraph, ValidatedPedalPlan, PedalValue, SignalKind};
    ///
    /// let graph = PedalGraph::new("input |> output");
    /// // In practice, ValidatedPedalNode would be used to build the plan internally
    /// // let plan = ValidatedPedalPlan::new(vec![], ValidatedPedalNode::Input(SignalKind::Audio));
    /// // let value = PedalValue::new(graph, plan);
    /// ```
    #[must_use]
    pub const fn new(graph: PedalGraph, plan: ValidatedPedalPlan) -> Self {
        Self { graph, plan }
    }

    #[doc(hidden)]
    #[must_use]
    pub const fn graph(&self) -> &PedalGraph {
        &self.graph
    }

    #[doc(hidden)]
    #[must_use]
    pub const fn plan(&self) -> &ValidatedPedalPlan {
        &self.plan
    }

    #[doc(hidden)]
    #[must_use]
    pub fn format_source(&self) -> String {
        self.graph.format_source()
    }
}

#[derive(Clone)]
struct GraphCompiler<'a> {
    resolved_signals: &'a BTreeMap<String, SignalKind>,
    binding_names: &'a BTreeSet<String>,
    current_binding: Option<&'a str>,
}

impl GraphCompiler<'_> {
    fn compile_expr(
        &self,
        expr: &Expr,
        allow_output: bool,
    ) -> Result<ValidatedPedalNode, EvalError> {
        match expr {
            Expr::Ident(name) => self.compile_ident(name, allow_output),
            Expr::Number(value) => Ok(ValidatedPedalNode::new(
                SignalKind::Control,
                PedalNodeKind::Constant,
                value.to_string(),
            )),
            Expr::String(value) => Ok(ValidatedPedalNode::new(
                SignalKind::Control,
                PedalNodeKind::Constant,
                format!("{value:?}"),
            )),
            Expr::Binary { lhs, op, rhs } => self.compile_binary(lhs, *op, rhs),
            Expr::Pipe { lhs, rhs } => {
                let lhs = self.compile_expr(lhs, false)?;
                self.compile_pipe_target(lhs, rhs, allow_output)
            }
            Expr::Call { callee, args } => self.compile_call(callee, args, None, allow_output),
            Expr::Group(items) if items.len() == 1 => self.compile_expr(&items[0], allow_output),
            Expr::Seq(_)
            | Expr::Stack(_)
            | Expr::Stream(_)
            | Expr::Graph { .. }
            | Expr::Voice { .. }
            | Expr::At { .. }
            | Expr::Meter { .. }
            | Expr::Beat(_)
            | Expr::Section { .. }
            | Expr::SeqSections(_)
            | Expr::Group(_)
            | Expr::Alternation(_)
            | Expr::Modified { .. }
            | Expr::Polymeter { .. }
            | Expr::Rest => Err(EvalError::new(
                "pedal graphs only support local names, literals, binary control/audio expressions, stage calls, and pipes in Task 3",
            )),
        }
    }

    fn compile_ident(
        &self,
        name: &str,
        allow_output: bool,
    ) -> Result<ValidatedPedalNode, EvalError> {
        if let Some(kind) = self.resolved_signals.get(name) {
            return Ok(ValidatedPedalNode::new(
                kind.clone(),
                PedalNodeKind::Reference,
                name,
            ));
        }

        match name {
            "input" => Ok(ValidatedPedalNode::new(
                SignalKind::Audio,
                PedalNodeKind::Input,
                "input",
            )),
            "output" if allow_output => Err(EvalError::new(
                "`output` must receive an audio signal as the pedal graph final result",
            )),
            "output" => Err(EvalError::new(
                "`output` may only appear in the pedal graph final result",
            )),
            _ if self.binding_names.contains(name) => {
                let owner = self.current_binding.unwrap_or("result");
                Err(EvalError::new(format!(
                    "implicit cycle: `{owner}` references `{name}` before it is defined; use `feedback(...)` for recursive pedal paths"
                )))
            }
            _ => Err(EvalError::new(format!("unbound local signal `{name}`"))),
        }
    }

    fn compile_binary(
        &self,
        lhs: &Expr,
        op: BinaryOp,
        rhs: &Expr,
    ) -> Result<ValidatedPedalNode, EvalError> {
        if op == BinaryOp::Assign {
            return Err(EvalError::new(
                "named pedal parameters are only valid inside stage calls",
            ));
        }

        let lhs = self.compile_expr(lhs, false)?;
        let rhs = self.compile_expr(rhs, false)?;
        let symbol = match op {
            BinaryOp::Add => "+",
            BinaryOp::Mul => "*",
            BinaryOp::Assign => "=",
        };

        match (op, lhs.signal_kind(), rhs.signal_kind()) {
            (BinaryOp::Add | BinaryOp::Mul, SignalKind::Control, SignalKind::Control) => {
                Ok(ValidatedPedalNode::new(
                    SignalKind::Control,
                    PedalNodeKind::Binary,
                    format!("({} {symbol} {})", lhs.summary(), rhs.summary()),
                ))
            }
            (BinaryOp::Add, SignalKind::Audio, SignalKind::Audio) => Ok(ValidatedPedalNode::new(
                SignalKind::Audio,
                PedalNodeKind::Binary,
                format!("({} + {})", lhs.summary(), rhs.summary()),
            )),
            (BinaryOp::Mul, SignalKind::Audio, SignalKind::Control)
            | (BinaryOp::Mul, SignalKind::Control, SignalKind::Audio) => {
                let (audio, control) = if lhs.signal_kind() == &SignalKind::Audio {
                    (lhs.summary(), rhs.summary())
                } else {
                    (rhs.summary(), lhs.summary())
                };
                Ok(ValidatedPedalNode::new(
                    SignalKind::Audio,
                    PedalNodeKind::Binary,
                    format!("({audio} * {control})"),
                ))
            }
            (BinaryOp::Add, _, _) => Err(EvalError::new(
                "`+` inside pedal graphs requires either two audio signals or two control expressions",
            )),
            (BinaryOp::Mul, _, _) => Err(EvalError::new(
                "`*` inside pedal graphs requires control*control or audio*control operands",
            )),
            (BinaryOp::Assign, _, _) => unreachable!(),
        }
    }

    fn compile_pipe_target(
        &self,
        lhs: ValidatedPedalNode,
        rhs: &Expr,
        allow_output: bool,
    ) -> Result<ValidatedPedalNode, EvalError> {
        match rhs {
            Expr::Ident(name) => self.compile_named_stage(name, &[], Some(lhs), allow_output),
            Expr::Call { callee, args } => self.compile_call(callee, args, Some(lhs), allow_output),
            _ => Err(EvalError::new(
                "pedal graph pipe targets must be stage names or stage calls",
            )),
        }
    }

    fn compile_call(
        &self,
        callee: &Expr,
        args: &[Expr],
        piped_input: Option<ValidatedPedalNode>,
        allow_output: bool,
    ) -> Result<ValidatedPedalNode, EvalError> {
        let Expr::Ident(name) = callee else {
            return Err(EvalError::new(
                "pedal graph stage calls require a simple stage identifier",
            ));
        };
        self.compile_named_stage(name, args, piped_input, allow_output)
    }

    fn compile_named_stage(
        &self,
        name: &str,
        args: &[Expr],
        piped_input: Option<ValidatedPedalNode>,
        allow_output: bool,
    ) -> Result<ValidatedPedalNode, EvalError> {
        if name == "output" && piped_input.is_none() {
            return Err(EvalError::new(
                "`output` may only appear as the pedal graph final pipe target",
            ));
        }
        if name == "feedback" {
            return self.compile_feedback_exprs(args, piped_input);
        }

        let mut positional = Vec::new();
        let mut named = Vec::new();

        if let Some(input) = piped_input {
            positional.push(input);
        }

        for arg in args {
            if let Expr::Binary {
                lhs,
                op: BinaryOp::Assign,
                rhs,
            } = arg
            {
                let Expr::Ident(param_name) = lhs.as_ref() else {
                    return Err(EvalError::new(
                        "named pedal parameters require an identifier on the left-hand side",
                    ));
                };
                let compiled = self.compile_named_argument_value(param_name, rhs)?;
                if compiled.signal_kind() == &SignalKind::Audio {
                    return Err(EvalError::new(format!(
                        "parameter `{param_name}` on `{name}` cannot be driven by an audio signal in Task 3"
                    )));
                }
                named.push((param_name.clone(), compiled));
            } else {
                positional.push(self.compile_expr(arg, false)?);
            }
        }

        match name {
            "output" => Self::compile_output_stage(&positional, &named, allow_output),
            "mix" => Self::compile_mix_stage(&positional, &named),
            "lfo" | "constant" => Self::compile_control_source(name, &positional, &named),
            "env_follow" => Self::compile_env_follow(&positional, &named),
            _ => Self::compile_audio_stage(name, &positional, &named),
        }
    }

    fn compile_named_argument_value(
        &self,
        param_name: &str,
        expr: &Expr,
    ) -> Result<ValidatedPedalNode, EvalError> {
        if let Expr::Ident(name) = expr {
            if let Some(kind) = self.resolved_signals.get(name) {
                return Ok(ValidatedPedalNode::new(
                    kind.clone(),
                    PedalNodeKind::Reference,
                    name,
                ));
            }
            if self.binding_names.contains(name) || name == "input" || name == "output" {
                return self.compile_expr(expr, false);
            }
            if is_selector_atom(param_name, name) {
                return Ok(ValidatedPedalNode::new(
                    SignalKind::Control,
                    PedalNodeKind::Constant,
                    name,
                ));
            }
            return Err(EvalError::new(format!("unbound local signal `{name}`")));
        }

        self.compile_expr(expr, false)
    }

    fn compile_feedback_exprs(
        &self,
        args: &[Expr],
        piped_input: Option<ValidatedPedalNode>,
    ) -> Result<ValidatedPedalNode, EvalError> {
        let mut scoped_signals = self.resolved_signals.clone();
        if let Some(current_binding) = self.current_binding {
            scoped_signals.insert(current_binding.to_owned(), SignalKind::Audio);
        }
        let scoped = GraphCompiler {
            resolved_signals: &scoped_signals,
            binding_names: self.binding_names,
            current_binding: self.current_binding,
        };

        let mut positional = Vec::new();
        let mut named = Vec::new();
        if let Some(input) = piped_input {
            positional.push(input);
        }

        for arg in args {
            if let Expr::Binary {
                lhs,
                op: BinaryOp::Assign,
                rhs,
            } = arg
            {
                let Expr::Ident(param_name) = lhs.as_ref() else {
                    return Err(EvalError::new(
                        "named pedal parameters require an identifier on the left-hand side",
                    ));
                };
                let compiled = scoped.compile_named_argument_value(param_name, rhs)?;
                if compiled.signal_kind() == &SignalKind::Audio {
                    return Err(EvalError::new(format!(
                        "parameter `{param_name}` on `feedback` cannot be driven by an audio signal in Task 3"
                    )));
                }
                named.push((param_name.clone(), compiled));
            } else {
                positional.push(scoped.compile_expr(arg, false)?);
            }
        }

        Self::compile_feedback_stage(&positional, &named)
    }

    fn compile_output_stage(
        positional: &[ValidatedPedalNode],
        named: &[(String, ValidatedPedalNode)],
        allow_output: bool,
    ) -> Result<ValidatedPedalNode, EvalError> {
        if !allow_output {
            return Err(EvalError::new(
                "`output` may only appear as the pedal graph final pipe target",
            ));
        }
        if !named.is_empty() || positional.len() != 1 {
            return Err(EvalError::new(
                "`output` requires exactly one audio signal as the pedal graph final pipe target",
            ));
        }
        if positional[0].signal_kind() != &SignalKind::Audio {
            return Err(EvalError::new(
                "`output` requires an audio signal as the pedal graph final pipe target",
            ));
        }

        Ok(ValidatedPedalNode::new(
            SignalKind::Audio,
            PedalNodeKind::Output,
            format!("output({})", positional[0].summary()),
        ))
    }

    fn compile_mix_stage(
        positional: &[ValidatedPedalNode],
        named: &[(String, ValidatedPedalNode)],
    ) -> Result<ValidatedPedalNode, EvalError> {
        if !named.is_empty() {
            return Err(EvalError::new(
                "`mix` does not accept named parameters in Task 3",
            ));
        }
        if positional.len() < 2 {
            return Err(EvalError::new("`mix` requires at least two audio inputs"));
        }
        if positional
            .iter()
            .any(|node| node.signal_kind() != &SignalKind::Audio)
        {
            return Err(EvalError::new(
                "`mix` requires every positional argument to resolve to an audio signal",
            ));
        }

        let mut summary = String::with_capacity(32);
        summary.push_str("mix(");
        let mut first = true;
        for node in positional {
            if !first {
                summary.push_str(", ");
            }
            summary.push_str(node.summary());
            first = false;
        }
        summary.push(')');

        Ok(ValidatedPedalNode::new(
            SignalKind::Audio,
            PedalNodeKind::Mix,
            summary,
        ))
    }

    fn compile_feedback_stage(
        positional: &[ValidatedPedalNode],
        named: &[(String, ValidatedPedalNode)],
    ) -> Result<ValidatedPedalNode, EvalError> {
        if positional.len() != 1 || positional[0].signal_kind() != &SignalKind::Audio {
            return Err(EvalError::new(
                "`feedback(...)` requires exactly one audio signal input",
            ));
        }

        let mut summary = String::with_capacity(32);
        summary.push_str("feedback(");
        summary.push_str(positional[0].summary());
        for (name, node) in named {
            summary.push_str(", ");
            summary.push_str(name);
            summary.push('=');
            summary.push_str(node.summary());
        }
        summary.push(')');

        Ok(ValidatedPedalNode::new(
            SignalKind::Audio,
            PedalNodeKind::Feedback,
            summary,
        ))
    }

    fn compile_control_source(
        name: &str,
        positional: &[ValidatedPedalNode],
        named: &[(String, ValidatedPedalNode)],
    ) -> Result<ValidatedPedalNode, EvalError> {
        if positional
            .iter()
            .any(|node| node.signal_kind() == &SignalKind::Audio)
        {
            return Err(EvalError::new(format!(
                "`{name}` cannot take an audio input; it is a control source"
            )));
        }

        Ok(ValidatedPedalNode::new(
            SignalKind::Control,
            PedalNodeKind::Stage,
            format_stage_summary(name, positional, named),
        ))
    }

    fn compile_env_follow(
        positional: &[ValidatedPedalNode],
        named: &[(String, ValidatedPedalNode)],
    ) -> Result<ValidatedPedalNode, EvalError> {
        if positional.len() != 1 || positional[0].signal_kind() != &SignalKind::Audio {
            return Err(EvalError::new(
                "`env_follow` requires exactly one audio signal input",
            ));
        }

        Ok(ValidatedPedalNode::new(
            SignalKind::Control,
            PedalNodeKind::Stage,
            format_stage_summary("env_follow", positional, named),
        ))
    }

    fn compile_audio_stage(
        name: &str,
        positional: &[ValidatedPedalNode],
        named: &[(String, ValidatedPedalNode)],
    ) -> Result<ValidatedPedalNode, EvalError> {
        let audio_inputs = positional
            .iter()
            .filter(|node| node.signal_kind() == &SignalKind::Audio)
            .count();

        match audio_inputs {
            0 => Err(EvalError::new(format!(
                "`{name}` requires an audio input in Task 3"
            ))),
            1 => Ok(ValidatedPedalNode::new(
                SignalKind::Audio,
                PedalNodeKind::Stage,
                format_stage_summary(name, positional, named),
            )),
            _ => Err(EvalError::new(format!(
                "`{name}` cannot take multiple audio inputs; use `mix(...)` for branch recombination"
            ))),
        }
    }
}

fn format_stage_summary(
    name: &str,
    positional: &[ValidatedPedalNode],
    named: &[(String, ValidatedPedalNode)],
) -> String {
    let mut summary = String::with_capacity(32);
    summary.push_str(name);
    summary.push('(');
    let mut first = true;
    for node in positional {
        if !first {
            summary.push_str(", ");
        }
        summary.push_str(node.summary());
        first = false;
    }
    for (param, node) in named {
        if !first {
            summary.push_str(", ");
        }
        summary.push_str(param);
        summary.push('=');
        summary.push_str(node.summary());
        first = false;
    }
    summary.push(')');
    summary
}

fn is_selector_atom(param_name: &str, ident: &str) -> bool {
    matches!(
        (param_name, ident),
        (
            "model",
            "silicon_hard"
                | "silicon_soft"
                | "germanium_soft"
                | "red_led"
                | "mid_hump"
                | "jfet_clean"
                | "opamp_tight"
        ) | ("kind", "hard" | "soft")
    )
}

fn format_graph_source_into(bindings: &[GraphBinding], result: &Expr, buf: &mut String) {
    format_block_source_into("graph", bindings, result, buf);
}

/// Formats a `voice { ... }` block back into canonical source text.
pub fn format_voice_source_into(bindings: &[GraphBinding], result: &Expr, buf: &mut String) {
    format_block_source_into("voice", bindings, result, buf);
}

fn format_block_source_into(
    keyword: &str,
    bindings: &[GraphBinding],
    result: &Expr,
    buf: &mut String,
) {
    buf.push_str(keyword);
    buf.push_str(" { ");
    let mut first = true;
    for binding in bindings {
        if !first {
            buf.push_str(" ; ");
        }
        buf.push_str(&binding.name);
        buf.push_str(" = ");
        format_expr_source_into(&binding.expr, buf);
        first = false;
    }
    if !first {
        buf.push_str(" ; ");
    }
    format_expr_source_into(result, buf);
    buf.push_str(" }");
}

fn format_separated_exprs_into(items: &[Expr], separator: &str, buf: &mut String) {
    let mut first = true;
    for item in items {
        if !first {
            buf.push_str(separator);
        }
        format_expr_source_into(item, buf);
        first = false;
    }
}

fn format_expr_source_into(expr: &Expr, buf: &mut String) {
    match expr {
        Expr::Seq(items) => {
            format_separated_exprs_into(items, " ", buf);
        }
        Expr::Stack(items) => {
            buf.push_str("stack(");
            format_separated_exprs_into(items, ", ", buf);
            buf.push(')');
        }
        Expr::Stream(items) => {
            buf.push_str("stream(");
            format_separated_exprs_into(items, ", ", buf);
            buf.push(')');
        }
        Expr::Graph { bindings, result } => format_graph_source_into(bindings, result, buf),
        Expr::Voice { bindings, result } => format_voice_source_into(bindings, result, buf),
        Expr::Pipe { lhs, rhs } => {
            format_expr_source_into(lhs, buf);
            buf.push_str(" |> ");
            format_expr_source_into(rhs, buf);
        }
        Expr::Binary { lhs, op, rhs } => {
            format_expr_source_into(lhs, buf);
            let symbol = match op {
                BinaryOp::Add => " + ",
                BinaryOp::Mul => " * ",
                BinaryOp::Assign => "=",
            };
            buf.push_str(symbol);
            format_expr_source_into(rhs, buf);
        }
        Expr::Call { callee, args } => {
            format_expr_source_into(callee, buf);
            buf.push('(');
            format_separated_exprs_into(args, ", ", buf);
            buf.push(')');
        }
        Expr::At { start, pattern } => {
            buf.push_str("at(");
            format_expr_source_into(start, buf);
            buf.push_str(", ");
            format_expr_source_into(pattern, buf);
            buf.push(')');
        }
        Expr::Meter {
            beats,
            unit,
            pattern,
        } => {
            buf.push_str("meter(");
            format_expr_source_into(beats, buf);
            buf.push_str(", ");
            format_expr_source_into(unit, buf);
            buf.push_str(", ");
            format_expr_source_into(pattern, buf);
            buf.push(')');
        }
        Expr::Beat(value) => {
            buf.push_str("beat(");
            format_expr_source_into(value, buf);
            buf.push(')');
        }
        Expr::Section { pattern, cycles } => {
            buf.push_str("section(");
            format_expr_source_into(pattern, buf);
            buf.push_str(", ");
            format_expr_source_into(cycles, buf);
            buf.push(')');
        }
        Expr::SeqSections(items) => {
            buf.push_str("seq_sections(");
            format_separated_exprs_into(items, ", ", buf);
            buf.push(')');
        }
        Expr::Group(items) => {
            buf.push('(');
            format_separated_exprs_into(items, " ", buf);
            buf.push(')');
        }
        Expr::Alternation(items) => {
            buf.push('<');
            format_separated_exprs_into(items, " ", buf);
            buf.push('>');
        }
        Expr::Modified { inner, op } => format_modified_source_into(inner, *op, buf),
        Expr::Polymeter { groups, steps } => format_polymeter_source_into(groups, *steps, buf),
        Expr::Ident(name) => buf.push_str(name),
        Expr::Rest => buf.push('~'),
        Expr::Number(value) => {
            use std::fmt::Write;
            let _ = write!(buf, "{value}");
        }
        Expr::String(value) => {
            use std::fmt::Write;
            let _ = write!(buf, "{value:?}");
        }
    }
}

fn format_modified_source_into(inner: &Expr, op: StepOp, buf: &mut String) {
    use std::fmt::Write;
    format_expr_source_into(inner, buf);
    let _ = match op {
        StepOp::Fast(factor) => write!(buf, "*{factor}"),
        StepOp::Slow(factor) => write!(buf, "/{factor}"),
        StepOp::Replicate(count) => write!(buf, "!{count}"),
        StepOp::Degrade(probability) => {
            if (probability - 0.5).abs() < f64::EPSILON {
                buf.push('?');
                Ok(())
            } else {
                write!(buf, "?{probability}")
            }
        }
    };
}

fn format_polymeter_source_into(groups: &[Vec<Expr>], steps: Option<i64>, buf: &mut String) {
    use std::fmt::Write;
    buf.push('{');
    let mut first = true;
    for group in groups {
        if !first {
            buf.push_str(", ");
        }
        format_separated_exprs_into(group, " ", buf);
        first = false;
    }
    buf.push('}');
    if let Some(steps) = steps {
        let _ = write!(buf, "%{steps}");
    }
}

/// Compiles a source-level pedal graph into a validated plan.
///
/// # Errors
///
/// Returns [`EvalError`] when the graph references an undefined local signal,
/// uses `output` outside the final result position, introduces an implicit cycle,
/// or violates the v1 audio/control classification rules.
pub fn compile_graph(
    bindings: &[GraphBinding],
    result_expr: &Expr,
) -> Result<PedalValue, EvalError> {
    let binding_names = bindings
        .iter()
        .map(|binding| binding.name.clone())
        .collect::<BTreeSet<_>>();
    let mut resolved_signals = BTreeMap::new();
    let mut compiled_bindings = Vec::with_capacity(bindings.len());

    for binding in bindings {
        let compiler = GraphCompiler {
            resolved_signals: &resolved_signals,
            binding_names: &binding_names,
            current_binding: Some(binding.name.as_str()),
        };
        let node = compiler.compile_expr(&binding.expr, false)?;
        resolved_signals.insert(binding.name.clone(), node.signal_kind().clone());
        compiled_bindings.push(ValidatedPedalBinding::new(binding.name.clone(), node));
    }

    let compiler = GraphCompiler {
        resolved_signals: &resolved_signals,
        binding_names: &binding_names,
        current_binding: None,
    };
    let result = compiler.compile_expr(result_expr, true)?;
    if result.kind() != &PedalNodeKind::Output {
        return Err(EvalError::new(
            "pedal graphs must route their final result through `output`",
        ));
    }

    let mut graph_src = String::with_capacity(128);
    format_graph_source_into(bindings, result_expr, &mut graph_src);
    let graph = PedalGraph::new(graph_src);
    let plan = ValidatedPedalPlan::new(compiled_bindings, result);
    Ok(PedalValue::new(graph, plan))
}

impl Explain for ValidatedPedalPlan {
    fn explain(&self, binding_name: &str) -> String {
        let title = format!(
            "{} {binding_name}\nTarget Signal Kind: {}",
            "Pedal Graph Plan:".cyan().bold(),
            self.signal_kind().to_string().yellow()
        );
        let mut table = crate::explain::explain_table(["Binding", "Kind", "Node"]);

        for binding in &self.bindings {
            table.add_row(vec![
                Cell::new(binding.name()).fg(comfy_table::Color::Cyan),
                Cell::new(binding.node().signal_kind().to_string()).fg(comfy_table::Color::Yellow),
                Cell::new(binding.node().summary()).fg(comfy_table::Color::Green),
            ]);
        }

        table.add_row(vec![
            Cell::new("=> result").fg(comfy_table::Color::Cyan),
            Cell::new(self.result.signal_kind().to_string()).fg(comfy_table::Color::Yellow),
            Cell::new(self.result.summary()).fg(comfy_table::Color::Green),
        ]);

        format!("{title}\n{table}")
    }
}

impl Explain for PedalValue {
    fn explain(&self, binding_name: &str) -> String {
        self.plan.explain(binding_name)
    }
}
