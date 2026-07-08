//! Language-level compilation for `voice { ... }` instrument definitions.
//!
//! A voice block describes a playable instrument over the graph vocabulary
//! that `orpheus-dsp` exposes (oscillators, gate-driven envelopes, the ladder
//! low-pass filter, saturation, and arithmetic mixing). The compiler lowers
//! the block into a flat, declarative [`VoiceNodeSpec`] DAG; the session
//! attaches the binding name as the pattern token and ships the finished
//! [`GraphVoiceSpec`] to the engine (ADR 0009/0010).
//!
//! Two ambient signals are in scope inside a voice body:
//!
//! - `gate` — 1 while the pattern event span holds, then 0
//! - `freq` — the note frequency in Hertz (reference frequency x event rate)
//!
//! The result expression is the mono voice signal; the engine appends the
//! per-trigger gain and equal-power pan stages automatically.

use std::collections::BTreeMap;

use comfy_table::Cell;
use crossterm::style::Stylize;
use orpheus_dsp::{
    DEFAULT_GRAPH_VOICE_POLYPHONY, GraphVoiceSpec, MAX_GRAPH_VOICE_POLYPHONY,
    MAX_VOICE_DELAY_SECONDS, MODULATED_VOICE_DELAY_MAX_SECONDS, VoiceNodeSpec, VoiceSignalRef,
};

use crate::ast::{BinaryOp, Expr, GraphBinding};
use crate::error::EvalError;
use crate::explain::Explain;

/// The release tail applied when a voice body contains no envelope, so gate
/// ends never hard-cut the waveform.
const DEFAULT_VOICE_RELEASE_SECONDS: f32 = 0.02;

/// The default pulse width used when `pulse(freq)` omits the width argument.
const DEFAULT_PULSE_WIDTH: f32 = 0.5;

/// The PRNG seed for `noise()` nodes; fixed so voices stay deterministic.
const VOICE_NOISE_SEED: u32 = 0x9E37_79B9;

/// The largest release floor a `release = ...` pragma may request, in
/// seconds, keeping note lifetimes (and pool residency) bounded.
const MAX_VOICE_RELEASE_FLOOR_SECONDS: f64 = 30.0;

/// The base value for feedback-loop placeholder references. Each nesting
/// depth uses `BASE - depth`; the placeholders are rewritten to the loop's
/// root node index once the loop body has compiled, so they can never
/// collide with real node indices (which are bounded by the node count).
const FEEDBACK_PLACEHOLDER_BASE: u32 = u32::MAX;

/// The language-side value for a compiled voice definition.
///
/// Holds the validated node DAG without a pattern token: the token is the
/// binding name, attached by [`Self::to_spec`] at registration time.
#[derive(Clone, Debug, PartialEq)]
pub struct VoiceValue {
    source: String,
    nodes: Vec<VoiceNodeSpec>,
    output: VoiceSignalRef,
    release_seconds: f32,
    polyphony: Option<usize>,
}

impl VoiceValue {
    /// The formatted `voice { ... }` source this value was compiled from.
    #[must_use]
    pub fn format_source(&self) -> String {
        self.source.clone()
    }

    /// The compiled node DAG.
    #[must_use]
    pub fn nodes(&self) -> &[VoiceNodeSpec] {
        &self.nodes
    }

    /// How long the voice keeps sounding after its gate falls, in seconds.
    ///
    /// Derived from the longest envelope release in the body, or a small
    /// default when the body has no envelope.
    #[must_use]
    pub const fn release_seconds(&self) -> f32 {
        self.release_seconds
    }

    /// The pooled polyphony requested by a `poly = ...` pragma, when present.
    ///
    /// `None` means the engine default (ADR 0009's pool of
    /// [`DEFAULT_GRAPH_VOICE_POLYPHONY`]).
    #[must_use]
    pub const fn polyphony(&self) -> Option<usize> {
        self.polyphony
    }

    /// Builds the engine-side program spec, using `token` (the binding name)
    /// as the pattern token.
    ///
    /// # Errors
    ///
    /// Returns [`EvalError`] when `token` is not a valid single-word pattern
    /// token or the DSP layer rejects the spec.
    pub fn to_spec(&self, token: &str) -> Result<GraphVoiceSpec, EvalError> {
        let spec =
            GraphVoiceSpec::new(token, self.release_seconds, self.nodes.clone(), self.output)
                .map_err(|error| {
                    EvalError::new(format!("voice `{token}` is not playable: {error}"))
                })?;
        match self.polyphony {
            Some(polyphony) => spec.with_polyphony(polyphony).map_err(|error| {
                EvalError::new(format!("voice `{token}` is not playable: {error}"))
            }),
            None => Ok(spec),
        }
    }
}

/// Compiles a `voice { ... }` block into a [`VoiceValue`].
///
/// # Errors
///
/// Returns [`EvalError`] when the body references an unbound name, calls an
/// unknown stage, passes the wrong number of arguments, or drives an envelope
/// segment with anything but a number literal.
pub fn compile_voice(bindings: &[GraphBinding], result: &Expr) -> Result<VoiceValue, EvalError> {
    let mut compiler = VoiceCompiler::default();

    for binding in bindings {
        match binding.name.as_str() {
            "poly" => {
                compiler.set_polyphony(&binding.expr)?;
                continue;
            }
            "release" => {
                compiler.set_release_floor(&binding.expr)?;
                continue;
            }
            "gate" | "freq" => {
                return Err(EvalError::new(format!(
                    "`{}` is a built-in voice input and cannot be redefined",
                    binding.name
                )));
            }
            "fb" => {
                return Err(EvalError::new(
                    "`fb` is reserved for the loop signal inside feedback(...) bodies \
                     and cannot be redefined",
                ));
            }
            _ => {}
        }
        if compiler.resolved.contains_key(&binding.name) {
            return Err(EvalError::new(format!(
                "voice signal `{}` is defined twice",
                binding.name
            )));
        }
        let reference = compiler.compile_expr(&binding.expr)?;
        compiler.resolved.insert(binding.name.clone(), reference);
    }

    let output = compiler.compile_expr(result)?;

    let mut source = String::with_capacity(128);
    crate::pedal::format_voice_source_into(bindings, result, &mut source);

    let release_floor = compiler.release_floor.unwrap_or(0.0);
    Ok(VoiceValue {
        source,
        nodes: compiler.nodes,
        output,
        release_seconds: compiler
            .max_release
            .max(release_floor)
            .max(DEFAULT_VOICE_RELEASE_SECONDS),
        polyphony: compiler.polyphony,
    })
}

#[derive(Default)]
struct VoiceCompiler {
    resolved: BTreeMap<String, VoiceSignalRef>,
    nodes: Vec<VoiceNodeSpec>,
    max_release: f32,
    /// Placeholder references for the enclosing `feedback(...)` loops, one
    /// per nesting level; `fb` resolves to the innermost entry.
    feedback_scopes: Vec<u32>,
    /// The pool size requested by a `poly = ...` pragma binding.
    polyphony: Option<usize>,
    /// The release-tail floor requested by a `release = ...` pragma binding.
    release_floor: Option<f32>,
}

impl VoiceCompiler {
    /// Handles the `poly = <integer literal>` pragma binding.
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    fn set_polyphony(&mut self, expr: &Expr) -> Result<(), EvalError> {
        if self.polyphony.is_some() {
            return Err(EvalError::new("voice `poly` is set twice"));
        }
        // MAX_GRAPH_VOICE_POLYPHONY (64) is far below f64's exact-integer
        // ceiling, so the comparison bound is precise.
        #[allow(clippy::cast_precision_loss)]
        let max_polyphony = MAX_GRAPH_VOICE_POLYPHONY as f64;
        match expr {
            Expr::Number(value)
                if value.fract() == 0.0 && (1.0..=max_polyphony).contains(value) =>
            {
                self.polyphony = Some(*value as usize);
                Ok(())
            }
            _ => Err(EvalError::new(format!(
                "voice `poly` must be an integer literal between 1 and \
                 {MAX_GRAPH_VOICE_POLYPHONY} (the pool is built before the audio thread runs)"
            ))),
        }
    }

    /// Handles the `release = <number literal>` pragma binding, which floors
    /// the release tail (useful when a feedback echo must ring out past the
    /// longest envelope release).
    #[allow(clippy::cast_possible_truncation)]
    fn set_release_floor(&mut self, expr: &Expr) -> Result<(), EvalError> {
        if self.release_floor.is_some() {
            return Err(EvalError::new("voice `release` is set twice"));
        }
        match expr {
            Expr::Number(value) if (0.0..=MAX_VOICE_RELEASE_FLOOR_SECONDS).contains(value) => {
                self.release_floor = Some(*value as f32);
                Ok(())
            }
            _ => Err(EvalError::new(format!(
                "voice `release` must be a number literal between 0 and \
                 {MAX_VOICE_RELEASE_FLOOR_SECONDS} seconds"
            ))),
        }
    }
    fn push(&mut self, node: VoiceNodeSpec) -> Result<VoiceSignalRef, EvalError> {
        let index = u32::try_from(self.nodes.len())
            .map_err(|_| EvalError::new("voice body exceeded the supported node count"))?;
        self.nodes.push(node);
        Ok(VoiceSignalRef::Node(index))
    }

    fn compile_expr(&mut self, expr: &Expr) -> Result<VoiceSignalRef, EvalError> {
        match expr {
            Expr::Ident(name) => self.compile_ident(name),
            Expr::Number(value) => self.compile_number(*value),
            Expr::Binary { lhs, op, rhs } => self.compile_binary(lhs, *op, rhs),
            Expr::Pipe { lhs, rhs } => {
                let piped = self.compile_expr(lhs)?;
                self.compile_pipe_target(piped, rhs)
            }
            Expr::Call { callee, args } => self.compile_call(callee, args, None),
            Expr::Group(items) if items.len() == 1 => self.compile_expr(&items[0]),
            _ => Err(EvalError::new(
                "voice bodies only support the ambient `gate`/`freq` signals, local names, \
                 number literals, `+`/`*` arithmetic, stage calls, and pipes",
            )),
        }
    }

    fn compile_ident(&self, name: &str) -> Result<VoiceSignalRef, EvalError> {
        match name {
            "gate" => Ok(VoiceSignalRef::Gate),
            "freq" => Ok(VoiceSignalRef::Freq),
            "fb" => self
                .feedback_scopes
                .last()
                .map(|&placeholder| VoiceSignalRef::Feedback(placeholder))
                .ok_or_else(|| {
                    EvalError::new(
                        "`fb` is only available inside a feedback(...) loop body, where it \
                         is the loop's previous output",
                    )
                }),
            _ => self.resolved.get(name).copied().ok_or_else(|| {
                EvalError::new(format!(
                    "unbound voice signal `{name}`; voice bodies see `gate`, `freq`, and \
                     names bound earlier in the block"
                ))
            }),
        }
    }

    #[allow(clippy::cast_possible_truncation)]
    fn compile_number(&mut self, value: f64) -> Result<VoiceSignalRef, EvalError> {
        if !value.is_finite() {
            return Err(EvalError::new("voice constants must be finite numbers"));
        }
        self.push(VoiceNodeSpec::Constant {
            value: value as f32,
        })
    }

    fn compile_binary(
        &mut self,
        lhs: &Expr,
        op: BinaryOp,
        rhs: &Expr,
    ) -> Result<VoiceSignalRef, EvalError> {
        if op == BinaryOp::Assign {
            return Err(EvalError::new(
                "assignments inside voice bodies must be `name = expr` block bindings",
            ));
        }
        let left = self.compile_expr(lhs)?;
        let right = self.compile_expr(rhs)?;
        match op {
            BinaryOp::Mul => self.push(VoiceNodeSpec::Mul { left, right }),
            BinaryOp::Add => self.push(VoiceNodeSpec::Add { left, right }),
            BinaryOp::Assign => unreachable!(),
        }
    }

    fn compile_pipe_target(
        &mut self,
        piped: VoiceSignalRef,
        rhs: &Expr,
    ) -> Result<VoiceSignalRef, EvalError> {
        match rhs {
            Expr::Call { callee, args } => self.compile_call(callee, args, Some(piped)),
            Expr::Ident(name) => self.compile_stage(name, &[], Some(piped)),
            _ => Err(EvalError::new(
                "voice pipe targets must be stage names or stage calls",
            )),
        }
    }

    fn compile_call(
        &mut self,
        callee: &Expr,
        args: &[Expr],
        piped: Option<VoiceSignalRef>,
    ) -> Result<VoiceSignalRef, EvalError> {
        let Expr::Ident(name) = callee else {
            return Err(EvalError::new(
                "voice stage calls require a simple stage identifier",
            ));
        };
        self.compile_stage(name, args, piped)
    }

    fn compile_stage(
        &mut self,
        name: &str,
        args: &[Expr],
        piped: Option<VoiceSignalRef>,
    ) -> Result<VoiceSignalRef, EvalError> {
        match name {
            "sine" | "saw" | "tri" => self.compile_oscillator(name, args, piped),
            "pulse" => self.compile_pulse(args, piped),
            "noise" => self.compile_noise(args, piped),
            "adsr" => self.compile_adsr(args, piped),
            "ar" => self.compile_ar(args, piped),
            "lowpass" => self.compile_lowpass(args, piped),
            "drive" => self.compile_drive(args, piped),
            "gain" => self.compile_gain(args, piped),
            "delay" => self.compile_delay(args, piped),
            "feedback" => self.compile_feedback(args, piped),
            "fan" => self.compile_fan(args, piped),
            other => Err(EvalError::new(format!(
                "unknown voice stage `{other}`; available stages are `sine`, `saw`, `tri`, \
                 `pulse`, `noise`, `adsr`, `ar`, `lowpass`, `drive`, `gain`, `delay`, \
                 `feedback`, and `fan`"
            ))),
        }
    }

    /// Collects the effective positional signal arguments: the piped input
    /// (if any) followed by each argument compiled to a signal reference.
    fn compile_signal_args(
        &mut self,
        stage: &str,
        args: &[Expr],
        piped: Option<VoiceSignalRef>,
        expected: usize,
    ) -> Result<Vec<VoiceSignalRef>, EvalError> {
        let mut signals = Vec::with_capacity(args.len() + 1);
        if let Some(piped) = piped {
            signals.push(piped);
        }
        for arg in args {
            signals.push(self.compile_expr(arg)?);
        }
        if signals.len() != expected {
            return Err(EvalError::new(format!(
                "`{stage}` expects {expected} signal argument(s) (counting a piped input), \
                 got {}",
                signals.len()
            )));
        }
        Ok(signals)
    }

    fn compile_oscillator(
        &mut self,
        name: &str,
        args: &[Expr],
        piped: Option<VoiceSignalRef>,
    ) -> Result<VoiceSignalRef, EvalError> {
        let signals = self.compile_signal_args(name, args, piped, 1)?;
        let freq = signals[0];
        let node = match name {
            "sine" => VoiceNodeSpec::Sine { freq },
            "saw" => VoiceNodeSpec::Saw { freq },
            "tri" => VoiceNodeSpec::Tri { freq },
            _ => unreachable!("oscillator dispatch is exhaustive"),
        };
        self.push(node)
    }

    fn compile_pulse(
        &mut self,
        args: &[Expr],
        piped: Option<VoiceSignalRef>,
    ) -> Result<VoiceSignalRef, EvalError> {
        let provided = args.len() + usize::from(piped.is_some());
        let signals = match provided {
            1 => {
                let mut signals = self.compile_signal_args("pulse", args, piped, 1)?;
                signals.push(self.push(VoiceNodeSpec::Constant {
                    value: DEFAULT_PULSE_WIDTH,
                })?);
                signals
            }
            _ => self.compile_signal_args("pulse", args, piped, 2)?,
        };
        self.push(VoiceNodeSpec::Pulse {
            freq: signals[0],
            width: signals[1],
        })
    }

    fn compile_noise(
        &mut self,
        args: &[Expr],
        piped: Option<VoiceSignalRef>,
    ) -> Result<VoiceSignalRef, EvalError> {
        if !args.is_empty() || piped.is_some() {
            return Err(EvalError::new("`noise` is a source and takes no arguments"));
        }
        self.push(VoiceNodeSpec::Noise {
            seed: VOICE_NOISE_SEED,
        })
    }

    fn compile_adsr(
        &mut self,
        args: &[Expr],
        piped: Option<VoiceSignalRef>,
    ) -> Result<VoiceSignalRef, EvalError> {
        let (gate, params) = Self::split_envelope_args("adsr", args, piped, 4)?;
        let gate = self.compile_expr_or_ref(gate)?;
        let attack_s = envelope_literal("adsr", "attack", &params[0])?;
        let decay_s = envelope_literal("adsr", "decay", &params[1])?;
        let sustain = envelope_literal("adsr", "sustain", &params[2])?;
        let release_s = envelope_literal("adsr", "release", &params[3])?;
        self.max_release = self.max_release.max(release_s);
        self.push(VoiceNodeSpec::Adsr {
            gate,
            attack_s,
            decay_s,
            sustain,
            release_s,
        })
    }

    fn compile_ar(
        &mut self,
        args: &[Expr],
        piped: Option<VoiceSignalRef>,
    ) -> Result<VoiceSignalRef, EvalError> {
        let (gate, params) = Self::split_envelope_args("ar", args, piped, 2)?;
        let gate = self.compile_expr_or_ref(gate)?;
        let attack_s = envelope_literal("ar", "attack", &params[0])?;
        let release_s = envelope_literal("ar", "release", &params[1])?;
        self.max_release = self.max_release.max(release_s);
        self.push(VoiceNodeSpec::Ar {
            gate,
            attack_s,
            release_s,
        })
    }

    /// Splits an envelope call into its gate signal and segment literals. The
    /// gate is the piped input when present, otherwise the first argument.
    fn split_envelope_args<'a>(
        stage: &str,
        args: &'a [Expr],
        piped: Option<VoiceSignalRef>,
        segments: usize,
    ) -> Result<(GateSource<'a>, &'a [Expr]), EvalError> {
        match piped {
            Some(reference) if args.len() == segments => {
                Ok((GateSource::Reference(reference), args))
            }
            None if args.len() == segments + 1 => {
                Ok((GateSource::Expression(&args[0]), &args[1..]))
            }
            _ => Err(EvalError::new(format!(
                "`{stage}` expects a gate signal plus {segments} number literals \
                 (e.g. `{stage}(gate, ...)`)"
            ))),
        }
    }

    fn compile_expr_or_ref(&mut self, source: GateSource<'_>) -> Result<VoiceSignalRef, EvalError> {
        match source {
            GateSource::Reference(reference) => Ok(reference),
            GateSource::Expression(expr) => self.compile_expr(expr),
        }
    }

    fn compile_lowpass(
        &mut self,
        args: &[Expr],
        piped: Option<VoiceSignalRef>,
    ) -> Result<VoiceSignalRef, EvalError> {
        let signals = self.compile_signal_args("lowpass", args, piped, 3)?;
        self.push(VoiceNodeSpec::Lowpass {
            input: signals[0],
            cutoff_hz: signals[1],
            resonance: signals[2],
        })
    }

    fn compile_drive(
        &mut self,
        args: &[Expr],
        piped: Option<VoiceSignalRef>,
    ) -> Result<VoiceSignalRef, EvalError> {
        let signals = self.compile_signal_args("drive", args, piped, 2)?;
        self.push(VoiceNodeSpec::Drive {
            input: signals[0],
            amount: signals[1],
        })
    }

    /// Compiles `gain(input, amount)` — multiplication as a pipeable stage,
    /// so pipe chains can scale a signal (`x |> delay(0.25) |> gain(0.6)`).
    fn compile_gain(
        &mut self,
        args: &[Expr],
        piped: Option<VoiceSignalRef>,
    ) -> Result<VoiceSignalRef, EvalError> {
        let signals = self.compile_signal_args("gain", args, piped, 2)?;
        self.push(VoiceNodeSpec::Mul {
            left: signals[0],
            right: signals[1],
        })
    }

    /// Compiles `delay(input, seconds)`. A number-literal time compiles to
    /// the fixed whole-sample delay line (capacity == time, allocated before
    /// the audio thread runs). Any other time expression is a SIGNAL driving
    /// the fractional delay line's time input — `delay(x, lfo)` is the
    /// chorus/flanger form — with a fixed capacity of
    /// [`MODULATED_VOICE_DELAY_MAX_SECONDS`]; requested times outside
    /// \[0, capacity\] clamp at render time.
    fn compile_delay(
        &mut self,
        args: &[Expr],
        piped: Option<VoiceSignalRef>,
    ) -> Result<VoiceSignalRef, EvalError> {
        let (input, seconds_expr) = match piped {
            Some(input) if args.len() == 1 => (input, &args[0]),
            None if args.len() == 2 => (self.compile_expr(&args[0])?, &args[1]),
            _ => {
                return Err(EvalError::new(
                    "`delay` expects an input signal plus a delay time in seconds — \
                     a number literal for a fixed line, or a signal to modulate it \
                     (e.g. `x |> delay(0.25)` or `x |> delay(lfo)`)",
                ));
            }
        };
        if matches!(seconds_expr, Expr::Number(_)) {
            let seconds = delay_literal(seconds_expr)?;
            self.push(VoiceNodeSpec::Delay { input, seconds })
        } else {
            let seconds = self.compile_expr(seconds_expr)?;
            self.push(VoiceNodeSpec::FractionalDelay {
                input,
                seconds,
                max_seconds: MODULATED_VOICE_DELAY_MAX_SECONDS,
            })
        }
    }

    /// Compiles `feedback(body)` — a one-sample feedback loop around `body`.
    ///
    /// Inside the body the ambient name `fb` is the loop's previous output;
    /// the whole expression's value is the loop's current output. It lowers
    /// onto the recursive graph combinator: a placeholder feedback reference
    /// stands in for `fb` while the body compiles and is patched to the
    /// body's root node once that index is known.
    fn compile_feedback(
        &mut self,
        args: &[Expr],
        piped: Option<VoiceSignalRef>,
    ) -> Result<VoiceSignalRef, EvalError> {
        if piped.is_some() || args.len() != 1 {
            return Err(EvalError::new(
                "`feedback(body)` takes exactly one loop expression; inside it, `fb` is \
                 the loop's previous output (e.g. `feedback(dry + fb |> delay(0.25) |> \
                 gain(0.6))`)",
            ));
        }

        let depth = u32::try_from(self.feedback_scopes.len())
            .map_err(|_| EvalError::new("feedback loops are nested too deeply"))?;
        let placeholder = FEEDBACK_PLACEHOLDER_BASE - depth;
        let loop_start = self.nodes.len();
        self.feedback_scopes.push(placeholder);
        let result = self.compile_expr(&args[0]);
        self.feedback_scopes.pop();

        let VoiceSignalRef::Node(root) = result? else {
            return Err(EvalError::new(
                "a feedback loop body must contain at least one processing stage so its \
                 output can be fed back",
            ));
        };
        for node in &mut self.nodes[loop_start..] {
            node.map_refs(|reference| {
                if *reference == VoiceSignalRef::Feedback(placeholder) {
                    *reference = VoiceSignalRef::Feedback(root);
                }
            });
        }
        Ok(VoiceSignalRef::Node(root))
    }

    /// Compiles `fan(input, branch, branch, ...)` — the input signal is
    /// duplicated into every branch (split) and the branch outputs are summed
    /// back into one signal (merge).
    fn compile_fan(
        &mut self,
        args: &[Expr],
        piped: Option<VoiceSignalRef>,
    ) -> Result<VoiceSignalRef, EvalError> {
        let (input, branches) = match piped {
            Some(input) if args.len() >= 2 => (input, args),
            None if args.len() >= 3 => (self.compile_expr(&args[0])?, &args[1..]),
            _ => {
                return Err(EvalError::new(
                    "`fan` expects an input signal plus at least two parallel branches \
                     (e.g. `fan(x, lowpass(500, 0.2), lowpass(3000, 0.2))`)",
                ));
            }
        };
        let inputs = branches
            .iter()
            .map(|branch| self.compile_branch(input, branch))
            .collect::<Result<Vec<_>, _>>()?;
        self.push(VoiceNodeSpec::Merge { inputs })
    }

    /// Compiles one `fan` branch: a stage call or pipe chain that receives
    /// the fan input as its piped-in first argument.
    fn compile_branch(
        &mut self,
        input: VoiceSignalRef,
        branch: &Expr,
    ) -> Result<VoiceSignalRef, EvalError> {
        match branch {
            Expr::Pipe { lhs, rhs } => {
                let head = self.compile_branch(input, lhs)?;
                self.compile_pipe_target(head, rhs)
            }
            Expr::Call { callee, args } => self.compile_call(callee, args, Some(input)),
            Expr::Ident(name) => self.compile_stage(name, &[], Some(input)),
            _ => Err(EvalError::new(
                "`fan` branches must be stage calls or pipe chains; each branch receives \
                 the fan input as its first argument",
            )),
        }
    }
}

#[derive(Clone, Copy)]
enum GateSource<'a> {
    Reference(VoiceSignalRef),
    Expression(&'a Expr),
}

/// Extracts an envelope segment parameter, which must be a number literal so
/// the segment can be bound as a compile-time constant (ADR 0004's `bind`).
#[allow(clippy::cast_possible_truncation)]
fn envelope_literal(stage: &str, segment: &str, expr: &Expr) -> Result<f32, EvalError> {
    match expr {
        Expr::Number(value) if value.is_finite() && *value >= 0.0 => Ok(*value as f32),
        _ => Err(EvalError::new(format!(
            "`{stage}` requires its {segment} parameter to be a non-negative number literal"
        ))),
    }
}

/// Extracts a `delay` time parameter, which must be a number literal within
/// the DSP layer's cap so the line's capacity is fixed before the audio
/// thread runs.
#[allow(clippy::cast_possible_truncation)]
fn delay_literal(expr: &Expr) -> Result<f32, EvalError> {
    match expr {
        Expr::Number(value) if (0.0..=f64::from(MAX_VOICE_DELAY_SECONDS)).contains(value) => {
            Ok(*value as f32)
        }
        _ => Err(EvalError::new(format!(
            "`delay` requires its seconds parameter to be a number literal between 0 and \
             {MAX_VOICE_DELAY_SECONDS} (the delay buffer is allocated before the audio \
             thread runs)"
        ))),
    }
}

impl Explain for VoiceValue {
    fn explain(&self, binding_name: &str) -> String {
        let polyphony = self.polyphony.map_or_else(
            || format!("{DEFAULT_GRAPH_VOICE_POLYPHONY} (default)"),
            |polyphony| polyphony.to_string(),
        );
        let title = format!(
            "{} {binding_name}\nRelease Tail: {}s\nPolyphony: {}\nSource: {}",
            "Voice Program:".cyan().bold(),
            self.release_seconds.to_string().yellow(),
            polyphony.yellow(),
            self.source.as_str().green()
        );
        let mut table = crate::explain::explain_table(["Node", "Spec"]);
        for (index, node) in self.nodes.iter().enumerate() {
            table.add_row(vec![
                Cell::new(index).fg(comfy_table::Color::Cyan),
                Cell::new(format!("{node:?}")).fg(comfy_table::Color::Green),
            ]);
        }
        table.add_row(vec![
            Cell::new("=> output").fg(comfy_table::Color::Cyan),
            Cell::new(format!("{:?}", self.output)).fg(comfy_table::Color::Yellow),
        ]);
        format!("{title}\n{table}")
    }
}
