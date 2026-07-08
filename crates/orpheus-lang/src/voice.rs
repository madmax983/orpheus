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
use orpheus_dsp::{GraphVoiceSpec, VoiceNodeSpec, VoiceSignalRef};

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

    /// Builds the engine-side program spec, using `token` (the binding name)
    /// as the pattern token.
    ///
    /// # Errors
    ///
    /// Returns [`EvalError`] when `token` is not a valid single-word pattern
    /// token or the DSP layer rejects the spec.
    pub fn to_spec(&self, token: &str) -> Result<GraphVoiceSpec, EvalError> {
        GraphVoiceSpec::new(
            token,
            self.release_seconds,
            self.nodes.clone(),
            self.output,
        )
        .map_err(|error| EvalError::new(format!("voice `{token}` is not playable: {error}")))
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
        if compiler.resolved.contains_key(&binding.name) {
            return Err(EvalError::new(format!(
                "voice signal `{}` is defined twice",
                binding.name
            )));
        }
        if matches!(binding.name.as_str(), "gate" | "freq") {
            return Err(EvalError::new(format!(
                "`{}` is a built-in voice input and cannot be redefined",
                binding.name
            )));
        }
        let reference = compiler.compile_expr(&binding.expr)?;
        compiler.resolved.insert(binding.name.clone(), reference);
    }

    let output = compiler.compile_expr(result)?;

    let mut source = String::with_capacity(128);
    crate::pedal::format_voice_source_into(bindings, result, &mut source);

    Ok(VoiceValue {
        source,
        nodes: compiler.nodes,
        output,
        release_seconds: compiler.max_release.max(DEFAULT_VOICE_RELEASE_SECONDS),
    })
}

#[derive(Default)]
struct VoiceCompiler {
    resolved: BTreeMap<String, VoiceSignalRef>,
    nodes: Vec<VoiceNodeSpec>,
    max_release: f32,
}

impl VoiceCompiler {
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
            other => Err(EvalError::new(format!(
                "unknown voice stage `{other}`; available stages are `sine`, `saw`, `tri`, \
                 `pulse`, `noise`, `adsr`, `ar`, `lowpass`, and `drive`"
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
            return Err(EvalError::new(
                "`noise` is a source and takes no arguments",
            ));
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
}

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

impl Explain for VoiceValue {
    fn explain(&self, binding_name: &str) -> String {
        let title = format!(
            "{} {binding_name}\nRelease Tail: {}s\nSource: {}",
            "Voice Program:".cyan().bold(),
            self.release_seconds.to_string().yellow(),
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
