//! Language-level compilation for `voice { ... }` instrument definitions.
//!
//! A voice block describes a playable instrument over the graph vocabulary
//! that `orpheus-dsp` exposes (oscillators, gate-driven envelopes, the ladder
//! low-pass filter, the multi-mode SVF and peaking/shelving EQ filters,
//! saturation, and arithmetic mixing). The compiler lowers
//! the block into a flat, declarative [`VoiceNodeSpec`] DAG; the session
//! attaches the binding name as the pattern token and ships the finished
//! [`GraphVoiceSpec`] to the engine (ADR 0009/0010).
//!
//! Six ambient signals are in scope inside a voice body:
//!
//! - `gate` — 1 while the pattern event span holds, then 0
//! - `freq` — the note frequency in Hertz (reference frequency x event rate)
//! - `p1`..`p4` — general-purpose per-note pattern parameters, set from the
//!   pattern side (`melody |> p1(<200 800>)`), sampled at trigger time and
//!   held for the note; 0 when the pattern never sets them (ADR 0010
//!   addendum)
//!
//! The result expression is the mono voice signal; the engine appends the
//! per-trigger gain and equal-power pan stages automatically.

use std::collections::BTreeMap;

use comfy_table::Cell;
use crossterm::style::Stylize;
use orpheus_dsp::{
    DEFAULT_ANALOG_BASE_FREQUENCY_HZ, DEFAULT_GRAPH_VOICE_POLYPHONY, DEFAULT_PARAM_RAMP_SECONDS,
    FILTER_MAX_GAIN_DB, FILTER_MAX_Q, FILTER_MIN_FREQUENCY_HZ, FILTER_MIN_Q, GraphVoiceSpec,
    MAX_GRAPH_VOICE_POLYPHONY, MAX_PARAM_RAMP_SECONDS, MAX_VOICE_DELAY_SECONDS,
    MODULATED_VOICE_DELAY_MAX_SECONDS, SampleBank, ShelfMode, StealPolicy, SvfMode, VoiceNodeSpec,
    VoiceSignalRef,
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

/// The largest `p1`..`p4` steal-ramp window a `param_ramp = ...` pragma may
/// request, in seconds (the DSP layer's [`MAX_PARAM_RAMP_SECONDS`] bound).
const MAX_PARAM_RAMP_PRAGMA_SECONDS: f64 = MAX_PARAM_RAMP_SECONDS as f64;

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
    steal: Option<StealPolicy>,
    param_ramp: Option<f32>,
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

    /// The pool-exhaustion policy requested by a `steal = ...` pragma, when
    /// present.
    ///
    /// `None` means the engine default ([`StealPolicy::Oldest`]: steal the
    /// most-released, else oldest, voice).
    #[must_use]
    pub const fn steal(&self) -> Option<StealPolicy> {
        self.steal
    }

    /// The `p1`..`p4` steal-ramp window requested by a `param_ramp = ...`
    /// pragma, in seconds, when present.
    ///
    /// `None` means the engine default ([`DEFAULT_PARAM_RAMP_SECONDS`], the
    /// 2 ms gain/pan steal-ramp precedent).
    #[must_use]
    pub const fn param_ramp(&self) -> Option<f32> {
        self.param_ramp
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
        let spec = match self.polyphony {
            Some(polyphony) => spec.with_polyphony(polyphony).map_err(|error| {
                EvalError::new(format!("voice `{token}` is not playable: {error}"))
            })?,
            None => spec,
        };
        let spec = match self.steal {
            Some(steal) => spec.with_steal_policy(steal),
            None => spec,
        };
        Ok(match self.param_ramp {
            Some(seconds) => spec.with_param_ramp_seconds(seconds).map_err(|error| {
                EvalError::new(format!("voice `{token}` is not playable: {error}"))
            })?,
            None => spec,
        })
    }
}

/// Compiles a `voice { ... }` block into a [`VoiceValue`].
///
/// `samples` resolves `sample("name")` stages: the named buffer's shared
/// handle is embedded in the compiled node DAG here, at definition time, so
/// the audio thread never touches the bank and an unknown name errors
/// immediately. A voice therefore keeps the buffer it was defined with until
/// it is redefined, even if the bank is reloaded afterwards.
///
/// # Errors
///
/// Returns [`EvalError`] when the body references an unbound name, calls an
/// unknown stage, passes the wrong number of arguments, drives an envelope
/// segment with anything but a number literal, or names a sample that is not
/// loaded in `samples`.
pub fn compile_voice(
    bindings: &[GraphBinding],
    result: &Expr,
    samples: &SampleBank,
) -> Result<VoiceValue, EvalError> {
    let mut compiler = VoiceCompiler::new(samples);

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
            "steal" => {
                compiler.set_steal_policy(&binding.expr)?;
                continue;
            }
            "param_ramp" => {
                compiler.set_param_ramp(&binding.expr)?;
                continue;
            }
            "gate" | "freq" | "p1" | "p2" | "p3" | "p4" => {
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
        steal: compiler.steal,
        param_ramp: compiler.param_ramp,
    })
}

struct VoiceCompiler<'bank> {
    /// The loaded sample bank `sample("name")` stages resolve against.
    samples: &'bank SampleBank,
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
    /// The pool-exhaustion policy requested by a `steal = ...` pragma
    /// binding.
    steal: Option<StealPolicy>,
    /// The `p1`..`p4` steal-ramp window requested by a `param_ramp = ...`
    /// pragma binding, in seconds.
    param_ramp: Option<f32>,
}

impl<'bank> VoiceCompiler<'bank> {
    const fn new(samples: &'bank SampleBank) -> Self {
        Self {
            samples,
            resolved: BTreeMap::new(),
            nodes: Vec::new(),
            max_release: 0.0,
            feedback_scopes: Vec::new(),
            polyphony: None,
            release_floor: None,
            steal: None,
            param_ramp: None,
        }
    }

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

    /// Handles the `steal = oldest|off` pragma binding, which sets what the
    /// program does when a trigger arrives and its voice pool is exhausted.
    fn set_steal_policy(&mut self, expr: &Expr) -> Result<(), EvalError> {
        if self.steal.is_some() {
            return Err(EvalError::new("voice `steal` is set twice"));
        }
        match expr {
            Expr::Ident(name) if name == "oldest" => {
                self.steal = Some(StealPolicy::Oldest);
                Ok(())
            }
            Expr::Ident(name) if name == "off" => {
                self.steal = Some(StealPolicy::Off);
                Ok(())
            }
            _ => Err(EvalError::new(
                "voice `steal` must be `oldest` (steal the most-released, else \
                 oldest, voice when the pool is full — the default) or `off` \
                 (drop extra notes)",
            )),
        }
    }

    /// Handles the `param_ramp = <number literal>` pragma binding, which sets
    /// how long the per-note `p1`..`p4` parameters take to glide from a
    /// stolen note's current values to the new note's (default 2 ms — the
    /// gain/pan steal-ramp precedent). Fresh triggers never ramp.
    #[allow(clippy::cast_possible_truncation)]
    fn set_param_ramp(&mut self, expr: &Expr) -> Result<(), EvalError> {
        if self.param_ramp.is_some() {
            return Err(EvalError::new("voice `param_ramp` is set twice"));
        }
        match expr {
            Expr::Number(value) if (0.0..=MAX_PARAM_RAMP_PRAGMA_SECONDS).contains(value) => {
                self.param_ramp = Some(*value as f32);
                Ok(())
            }
            _ => Err(EvalError::new(format!(
                "voice `param_ramp` must be a number literal between 0 and \
                 {MAX_PARAM_RAMP_PRAGMA_SECONDS} seconds (how long p1..p4 glide to a \
                 stolen note's new values)"
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
                "voice bodies only support the ambient `gate`/`freq`/`p1`..`p4` signals, \
                 local names, number literals, `+`/`*` arithmetic, stage calls, and pipes",
            )),
        }
    }

    fn compile_ident(&self, name: &str) -> Result<VoiceSignalRef, EvalError> {
        match name {
            "gate" => Ok(VoiceSignalRef::Gate),
            "freq" => Ok(VoiceSignalRef::Freq),
            // The per-note pattern parameters (ADR 0010 addendum): set on
            // the triggering event (`melody |> p1(<200 800>)`), sampled at
            // trigger time and held for the note; 0 when never set.
            "p1" => Ok(VoiceSignalRef::Param(0)),
            "p2" => Ok(VoiceSignalRef::Param(1)),
            "p3" => Ok(VoiceSignalRef::Param(2)),
            "p4" => Ok(VoiceSignalRef::Param(3)),
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
                    "unbound voice signal `{name}`; voice bodies see `gate`, `freq`, the \
                     pattern parameters `p1`..`p4`, and names bound earlier in the block"
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
            "svf_lp" | "svf_hp" | "svf_bp" | "svf_notch" => self.compile_svf(name, args, piped),
            "eq_peak" => self.compile_eq_peak(args, piped),
            "eq_low_shelf" | "eq_high_shelf" => self.compile_eq_shelf(name, args, piped),
            "drive" => self.compile_drive(args, piped),
            "gain" => self.compile_gain(args, piped),
            "delay" => self.compile_delay(args, piped),
            "feedback" => self.compile_feedback(args, piped),
            "fan" => self.compile_fan(args, piped),
            "sample" | "sample_loop" | "sample_loop_xf" => self.compile_sample(name, args, piped),
            "sample_pitched" | "sample_loop_pitched" | "sample_loop_pitched_xf" => {
                self.compile_sample_pitched(name, args, piped)
            }
            other => Err(EvalError::new(format!(
                "unknown voice stage `{other}`; available stages are `sine`, `saw`, `tri`, \
                 `pulse`, `noise`, `sample`, `sample_loop`, `sample_loop_xf`, `sample_pitched`, \
                 `sample_loop_pitched`, `sample_loop_pitched_xf`, `adsr`, `ar`, `lowpass`, \
                 `svf_lp`, `svf_hp`, `svf_bp`, `svf_notch`, `eq_peak`, `eq_low_shelf`, \
                 `eq_high_shelf`, `drive`, `gain`, `delay`, `feedback`, and `fan`"
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

    /// Compiles `svf_lp`/`svf_hp`/`svf_bp`/`svf_notch` — one response of the
    /// TPT state-variable filter, `(input, cutoff_hz, q)` like `lowpass`.
    /// The SVF recomputes its coefficients every sample, so cutoff and Q may
    /// be bound signals (`saw(freq) |> svf_lp(lfo, 0.7)` sweeps the cutoff
    /// with an LFO). Literal parameters are range-checked here at definition
    /// time; signal parameters clamp at render time instead.
    fn compile_svf(
        &mut self,
        name: &str,
        args: &[Expr],
        piped: Option<VoiceSignalRef>,
    ) -> Result<VoiceSignalRef, EvalError> {
        let (input, cutoff_expr, q_expr) = match piped {
            Some(input) if args.len() == 2 => (input, &args[0], &args[1]),
            None if args.len() == 3 => (self.compile_expr(&args[0])?, &args[1], &args[2]),
            _ => {
                return Err(EvalError::new(format!(
                    "`{name}` expects an input signal plus cutoff and Q \
                     (e.g. `x |> {name}(1200, 0.7)`)"
                )));
            }
        };
        filter_frequency_literal_in_range(name, "cutoff", cutoff_expr)?;
        filter_q_literal_in_range(name, q_expr)?;
        let cutoff_hz = self.compile_expr(cutoff_expr)?;
        let q = self.compile_expr(q_expr)?;
        let mode = match name {
            "svf_lp" => SvfMode::Lowpass,
            "svf_hp" => SvfMode::Highpass,
            "svf_bp" => SvfMode::Bandpass,
            "svf_notch" => SvfMode::Notch,
            _ => unreachable!("svf dispatch is exhaustive"),
        };
        self.push(VoiceNodeSpec::Svf {
            input,
            cutoff_hz,
            q,
            mode,
        })
    }

    /// Compiles `eq_peak(input, freq_hz, q, gain_db)` — the RBJ peaking
    /// (bell) EQ biquad. Positive gains boost the band around `freq_hz`,
    /// negative gains cut it; all three parameters are signals. Literal
    /// parameters are range-checked here at definition time; signal
    /// parameters clamp at render time instead.
    fn compile_eq_peak(
        &mut self,
        args: &[Expr],
        piped: Option<VoiceSignalRef>,
    ) -> Result<VoiceSignalRef, EvalError> {
        let (input, freq_expr, q_expr, gain_expr) = match piped {
            Some(input) if args.len() == 3 => (input, &args[0], &args[1], &args[2]),
            None if args.len() == 4 => (self.compile_expr(&args[0])?, &args[1], &args[2], &args[3]),
            _ => {
                return Err(EvalError::new(
                    "`eq_peak` expects an input signal plus center frequency, Q, and \
                     gain in dB (e.g. `x |> eq_peak(800, 1.5, 6)`)",
                ));
            }
        };
        filter_frequency_literal_in_range("eq_peak", "center frequency", freq_expr)?;
        filter_q_literal_in_range("eq_peak", q_expr)?;
        filter_gain_literal_in_range("eq_peak", gain_expr)?;
        let freq_hz = self.compile_expr(freq_expr)?;
        let q = self.compile_expr(q_expr)?;
        let gain_db = self.compile_expr(gain_expr)?;
        self.push(VoiceNodeSpec::EqPeak {
            input,
            freq_hz,
            q,
            gain_db,
        })
    }

    /// Compiles `eq_low_shelf(input, freq_hz, q, gain_db)` and
    /// `eq_high_shelf(...)` — the RBJ shelving EQ biquads. A low shelf
    /// applies the gain below the corner frequency and leaves the highs at
    /// unity; the high shelf mirrors it. Positive gains boost, negative
    /// gains cut; all three parameters are signals. Literal parameters are
    /// range-checked here at definition time; signal parameters clamp at
    /// render time instead.
    fn compile_eq_shelf(
        &mut self,
        name: &str,
        args: &[Expr],
        piped: Option<VoiceSignalRef>,
    ) -> Result<VoiceSignalRef, EvalError> {
        let (input, freq_expr, q_expr, gain_expr) = match piped {
            Some(input) if args.len() == 3 => (input, &args[0], &args[1], &args[2]),
            None if args.len() == 4 => (self.compile_expr(&args[0])?, &args[1], &args[2], &args[3]),
            _ => {
                return Err(EvalError::new(format!(
                    "`{name}` expects an input signal plus corner frequency, Q, and \
                     gain in dB (e.g. `x |> {name}(800, 0.7, 6)`)"
                )));
            }
        };
        filter_frequency_literal_in_range(name, "corner frequency", freq_expr)?;
        filter_q_literal_in_range(name, q_expr)?;
        filter_gain_literal_in_range(name, gain_expr)?;
        let freq_hz = self.compile_expr(freq_expr)?;
        let q = self.compile_expr(q_expr)?;
        let gain_db = self.compile_expr(gain_expr)?;
        let mode = match name {
            "eq_low_shelf" => ShelfMode::Low,
            "eq_high_shelf" => ShelfMode::High,
            _ => unreachable!("shelf dispatch is exhaustive"),
        };
        self.push(VoiceNodeSpec::EqShelf {
            input,
            freq_hz,
            q,
            gain_db,
            mode,
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

    /// Compiles `sample("name"[, rate])`, `sample_loop("name"[, rate])`, and
    /// `sample_loop_xf("name"[, rate])` — playback of a preloaded
    /// sample-bank buffer, so hybrid sample+synth instruments compose (e.g.
    /// `voice { s = sample("bd") ; s * ar(gate, 0.001, 0.2) }`). `sample` is
    /// one-shot; `sample_loop` hard-wraps at the buffer end and keeps
    /// sounding until the voice's release tail ends (a falling gate never
    /// cuts any of these stages — the one-shot semantics); `sample_loop_xf`
    /// is `sample_loop` with a short linear crossfade at the loop wrap (5 ms
    /// of source material, capped at 10% of the buffer), removing the wrap
    /// click for loops that do not end on a zero crossing. The crossfade is
    /// a separate stage name for the same reason `sample_loop` is: the
    /// grammar has no keyword arguments and the second positional slot is
    /// already the rate signal.
    ///
    /// The note gate triggers playback implicitly: a rising edge restarts
    /// the buffer from the top and the level is otherwise ignored (one-shot,
    /// like the engine's sample voices). The optional `rate` argument is a
    /// SIGNAL (1.0 = native pitch, the default) — a per-note pattern
    /// parameter binds directly (`sample("bd", p1)` with `hits |> p1(1 2)`).
    /// The name must be a string literal: the buffer's shared handle is
    /// resolved and embedded here, before the audio thread runs, so an
    /// unknown name errors at definition time.
    fn compile_sample(
        &mut self,
        stage: &str,
        args: &[Expr],
        piped: Option<VoiceSignalRef>,
    ) -> Result<VoiceSignalRef, EvalError> {
        if piped.is_some() {
            return Err(EvalError::new(format!(
                "`{stage}` is a source and cannot be a pipe target; call it directly \
                 with a sample name (e.g. `{stage}(\"bd\")` or `{stage}(\"bd\", 2)`)"
            )));
        }
        let (name_expr, rate_expr) = match args {
            [name] => (name, None),
            [name, rate] => (name, Some(rate)),
            _ => {
                return Err(EvalError::new(format!(
                    "`{stage}` expects a sample name plus an optional rate signal \
                     (e.g. `{stage}(\"bd\")` or `{stage}(\"bd\", 2)`)"
                )));
            }
        };
        let sample = self.resolve_sample_literal(stage, name_expr)?;
        let rate = match rate_expr {
            Some(expr) => self.compile_expr(expr)?,
            None => self.push(VoiceNodeSpec::Constant { value: 1.0 })?,
        };
        self.push(VoiceNodeSpec::Sample {
            gate: VoiceSignalRef::Gate,
            rate,
            sample,
            looped: matches!(stage, "sample_loop" | "sample_loop_xf"),
            loop_crossfade: stage == "sample_loop_xf",
            pitch_reference_hz: None,
        })
    }

    /// Compiles `sample_pitched("name"[, reference_hz])`,
    /// `sample_loop_pitched("name"[, reference_hz])`, and
    /// `sample_loop_pitched_xf("name"[, reference_hz])` — playback whose rate
    /// tracks the triggering note: rate = `freq` / reference, so a pattern's
    /// pitches transpose the sample like an oscillator. `sample_pitched` is
    /// one-shot; `sample_loop_pitched` also hard-wraps at the buffer end
    /// like `sample_loop`, and `sample_loop_pitched_xf` further crossfades
    /// the loop wrap like `sample_loop_xf` (the flags compose in the node
    /// spec). The combinations are separate stage names for the same reason
    /// `sample_loop` is: the grammar has no keyword arguments and the second
    /// positional slot is already the reference frequency.
    ///
    /// The reference is the note frequency that plays the buffer at native
    /// rate. It defaults to the engine's rate-1.0 reference frequency
    /// (220 Hz — `freq` is computed as 220 Hz x the event's playback-rate
    /// multiplier, so an unshifted note plays natively and `|> pitch(12)`
    /// doubles the rate). A number-literal argument overrides it, e.g.
    /// `sample_pitched("bd", 7040)` plays natively on c4 (220 x 2^(60/12)).
    fn compile_sample_pitched(
        &mut self,
        stage: &str,
        args: &[Expr],
        piped: Option<VoiceSignalRef>,
    ) -> Result<VoiceSignalRef, EvalError> {
        if piped.is_some() {
            return Err(EvalError::new(format!(
                "`{stage}` is a source and cannot be a pipe target; call it \
                 directly with a sample name (e.g. `{stage}(\"bd\")`)"
            )));
        }
        let (name_expr, reference_expr) = match args {
            [name] => (name, None),
            [name, reference] => (name, Some(reference)),
            _ => {
                return Err(EvalError::new(format!(
                    "`{stage}` expects a sample name plus an optional reference \
                     frequency in Hz (e.g. `{stage}(\"bd\")` or \
                     `{stage}(\"bd\", 440)`)"
                )));
            }
        };
        let sample = self.resolve_sample_literal(stage, name_expr)?;
        #[allow(clippy::cast_possible_truncation)]
        let reference_hz = match reference_expr {
            None => DEFAULT_ANALOG_BASE_FREQUENCY_HZ,
            Some(Expr::Number(value)) if value.is_finite() && *value > 0.0 => *value as f32,
            Some(Expr::Number(_)) => {
                return Err(EvalError::new(format!(
                    "`{stage}` requires a positive finite reference frequency \
                     (the note frequency that plays the sample at native rate)"
                )));
            }
            Some(_) => {
                return Err(EvalError::new(format!(
                    "`{stage}` requires its reference to be a number literal — \
                     it is fixed before the audio thread runs (e.g. \
                     `{stage}(\"bd\", 440)`)"
                )));
            }
        };
        self.push(VoiceNodeSpec::Sample {
            gate: VoiceSignalRef::Gate,
            rate: VoiceSignalRef::Freq,
            sample,
            looped: matches!(stage, "sample_loop_pitched" | "sample_loop_pitched_xf"),
            loop_crossfade: stage == "sample_loop_pitched_xf",
            pitch_reference_hz: Some(reference_hz),
        })
    }

    /// Resolves a sample-stage name argument against the bank and extends
    /// the voice's release tail to cover the buffer at native rate (capped
    /// like the `release` pragma so note lifetimes stay bounded) — playback
    /// survives the gate falling, for one-shots and loops alike.
    fn resolve_sample_literal(
        &mut self,
        stage: &str,
        name_expr: &Expr,
    ) -> Result<orpheus_dsp::PlaybackSample, EvalError> {
        let Expr::String(name) = name_expr else {
            return Err(EvalError::new(format!(
                "`{stage}` requires its name to be a string literal — the buffer is \
                 resolved before the audio thread runs (e.g. `{stage}(\"bd\")`)"
            )));
        };
        let Some(sample) = self.samples.get_by_token(name) else {
            let available = self.samples.available_tokens();
            return Err(EvalError::new(if available.is_empty() {
                format!("unknown sample `{name}`: no samples are loaded")
            } else {
                format!(
                    "unknown sample `{name}`; loaded samples: {}",
                    available.join(", ")
                )
            }));
        };
        let sample = sample.clone();

        #[allow(clippy::cast_possible_truncation)]
        let duration_seconds = sample
            .duration_seconds()
            .min(MAX_VOICE_RELEASE_FLOOR_SECONDS) as f32;
        self.max_release = self.max_release.max(duration_seconds);
        Ok(sample)
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

/// Rejects a number-literal filter frequency below the DSP layer's 1 Hz
/// floor at definition time. Non-literal (signal) parameters pass: they
/// clamp to the same bounds at render time.
fn filter_frequency_literal_in_range(
    stage: &str,
    parameter: &str,
    expr: &Expr,
) -> Result<(), EvalError> {
    match expr {
        Expr::Number(value)
            if !(value.is_finite() && *value >= f64::from(FILTER_MIN_FREQUENCY_HZ)) =>
        {
            Err(EvalError::new(format!(
                "`{stage}` requires a literal {parameter} of at least \
                 {FILTER_MIN_FREQUENCY_HZ} Hz (a bound signal clamps at render time instead)"
            )))
        }
        _ => Ok(()),
    }
}

/// Rejects a number-literal filter Q outside the DSP layer's clamp bounds at
/// definition time. Non-literal (signal) parameters pass: they clamp to the
/// same bounds at render time.
fn filter_q_literal_in_range(stage: &str, expr: &Expr) -> Result<(), EvalError> {
    let bounds = f64::from(FILTER_MIN_Q)..=f64::from(FILTER_MAX_Q);
    match expr {
        Expr::Number(value) if !bounds.contains(value) => Err(EvalError::new(format!(
            "`{stage}` requires a literal Q between {FILTER_MIN_Q} and {FILTER_MAX_Q} \
             (a bound signal clamps at render time instead)"
        ))),
        _ => Ok(()),
    }
}

/// Rejects a number-literal EQ gain outside the DSP layer's +/-40 dB clamp
/// bounds at definition time. Non-literal (signal) parameters pass: they
/// clamp to the same bounds at render time.
fn filter_gain_literal_in_range(stage: &str, expr: &Expr) -> Result<(), EvalError> {
    let max = f64::from(FILTER_MAX_GAIN_DB);
    match expr {
        Expr::Number(value) if !(-max..=max).contains(value) => Err(EvalError::new(format!(
            "`{stage}` requires a literal gain between -{FILTER_MAX_GAIN_DB} and \
             {FILTER_MAX_GAIN_DB} dB (a bound signal clamps at render time instead)"
        ))),
        _ => Ok(()),
    }
}

impl Explain for VoiceValue {
    fn explain(&self, binding_name: &str) -> String {
        let polyphony = self.polyphony.map_or_else(
            || format!("{DEFAULT_GRAPH_VOICE_POLYPHONY} (default)"),
            |polyphony| polyphony.to_string(),
        );
        let steal = match self.steal {
            None => "oldest (default)".to_string(),
            Some(StealPolicy::Oldest) => "oldest".to_string(),
            Some(StealPolicy::Off) => "off".to_string(),
        };
        let param_ramp = self.param_ramp.map_or_else(
            || format!("{DEFAULT_PARAM_RAMP_SECONDS}s (default)"),
            |seconds| format!("{seconds}s"),
        );
        let title = format!(
            "{} {binding_name}\nRelease Tail: {}s\nPolyphony: {}\nSteal: {}\nParam Ramp: {}\nSource: {}",
            "Voice Program:".cyan().bold(),
            self.release_seconds.to_string().yellow(),
            polyphony.yellow(),
            steal.yellow(),
            param_ramp.yellow(),
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
