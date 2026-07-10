//! Graph-backed engine voices (ADR 0004 follow-up, ADR 0009).
//!
//! This module wires the Faust-style graph combinator system into the render
//! engine's voice lifecycle. A [`GraphVoiceProgram`] describes a playable
//! stereo voice as a compiled graph [`Processor`] with the fixed interface
//!
//! ```text
//! inputs:  [gate, freq_hz, gain, pan, p1..p4]   (params as signals, ADR 0004)
//! outputs: [left, right]
//! ```
//!
//! The trailing `p1..p4` channels are general-purpose per-note parameters set
//! by the pattern side (ADR 0010 addendum): each is sampled at trigger time
//! and, by default, held constant for the note, exactly like the gain and
//! pan fields. A control pattern with sub-note structure additionally ships
//! up to [`MAX_VOICE_PARAM_BREAKPOINTS`] automation breakpoints per
//! parameter with the trigger (ADR 0012); while the note sounds, the
//! parameter follows the breakpoints with linear interpolation between them
//! instead of holding. Notes triggered without explicit values read
//! [`DEFAULT_VOICE_PARAM_VALUE`] (0.0). When a trigger steals a sounding
//! voice the parameters glide from the stolen note's current values over
//! the program's param-ramp window ([`DEFAULT_PARAM_RAMP_SECONDS`] unless
//! overridden): linearly onto the new note's held values, or — when the new
//! note carries breakpoints — converging onto its automation envelope. The
//! glide lands exactly, after which the held values (or the envelope) alone
//! drive; fresh (idle-voice) triggers start exactly at the new values.
//!
//! Allocation discipline: programs are compiled into a fixed pool of
//! [`GraphVoice`]s at engine construction time (off the audio thread) and
//! warmed with [`GraphVoice::prepare`] so lazy-but-once scratch growth also
//! happens off-thread. Triggering and rendering a pooled voice performs no
//! heap allocation; finished voices are reset in place and reused instead of
//! being dropped on the audio thread. When a program's pool is exhausted the
//! trigger steals a sounding voice per its [`StealPolicy`] (default: the
//! most-released, else oldest, note) with a click-free handover.

use thiserror::Error;

use crate::SampleTrigger;
use crate::graph::{
    BiquadMode, Node, Processor, Seq, adsr, ar, bind, biquad, constant, delay_line, fdelay,
    feedback, gain_node, ladder_filter, merge, noise, pan, par, passthrough, pulse,
    sample_player_with_options, saw, seq, sine, soft_sat, sum, svf, tri, wire, wire_with_inputs,
};
use crate::routing::TrackId;
use crate::sample_bank::PlaybackSample;

/// How many general-purpose per-note pattern parameters (`p1`..`p4`) every
/// graph voice program receives (ADR 0010 addendum).
///
/// The parameters ride the fixed voice interface as trailing signal inputs
/// (`[gate, freq, gain, pan, p1..p4]`), stamped as plain `f32` fields at
/// trigger time and held for the note, so the audio-thread path stays
/// allocation-free.
pub const VOICE_PARAM_COUNT: usize = 4;

/// The value a voice body reads from a per-note parameter (`p1`..`p4`) the
/// triggering pattern never set.
pub const DEFAULT_VOICE_PARAM_VALUE: f32 = 0.0;

/// How many automation breakpoints one note may carry per parameter
/// (ADR 0012).
///
/// Breakpoint storage is a fixed-size array inside every sounding note so
/// stamping and playback stay allocation-free on the audio thread. When a
/// control pattern produces more sub-note values than fit, only the first
/// `MAX_VOICE_PARAM_BREAKPOINTS` ship (query side and trigger stamping both
/// truncate); the last kept value holds for the rest of the note.
pub const MAX_VOICE_PARAM_BREAKPOINTS: usize = 32;

/// One per-note parameter automation breakpoint (ADR 0012).
///
/// `position` is normalized musical time within the note's sounding extent
/// (0 = the trigger point, 1 = the end of the event's whole span), so the
/// breakpoint survives tempo changes between query time and trigger time;
/// it is mapped to a frame offset when the note starts. `value` is the
/// control-pattern value that takes effect at that position; playback
/// interpolates linearly from the previous breakpoint's value, reaching
/// `value` exactly at `position` (no steps — ADR 0012).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VoiceParamBreakpoint {
    /// Normalized position within the note's extent, clamped to \[0, 1\] at
    /// trigger time. Non-finite positions are skipped.
    pub position: f64,
    /// The parameter value reached at `position`. Non-finite values are
    /// skipped at trigger time.
    pub value: f64,
}

impl VoiceParamBreakpoint {
    /// Creates a breakpoint at a normalized position within the note.
    #[must_use]
    pub const fn new(position: f64, value: f64) -> Self {
        Self { position, value }
    }
}

/// Fixed-capacity per-note parameter automation, stamped at trigger time
/// (ADR 0012).
///
/// Holds up to [`MAX_VOICE_PARAM_BREAKPOINTS`] `(frame, value)` breakpoints
/// for each of the `p1`..`p4` parameters, with frames strictly increasing
/// and the first breakpoint always at frame 0. Everything is inline arrays:
/// building one from a trigger, copying it into a pooled note, and reading
/// values during rendering never allocate.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VoiceParamRamps {
    frames: [[u32; MAX_VOICE_PARAM_BREAKPOINTS]; VOICE_PARAM_COUNT],
    values: [[f32; MAX_VOICE_PARAM_BREAKPOINTS]; VOICE_PARAM_COUNT],
    lens: [u8; VOICE_PARAM_COUNT],
}

impl Default for VoiceParamRamps {
    fn default() -> Self {
        Self::none()
    }
}

impl VoiceParamRamps {
    /// No automation: every parameter holds its trigger-time value.
    #[must_use]
    pub const fn none() -> Self {
        Self {
            frames: [[0; MAX_VOICE_PARAM_BREAKPOINTS]; VOICE_PARAM_COUNT],
            values: [[0.0; MAX_VOICE_PARAM_BREAKPOINTS]; VOICE_PARAM_COUNT],
            lens: [0; VOICE_PARAM_COUNT],
        }
    }

    /// Whether parameter `index` carries breakpoints (out-of-range indices
    /// read `false`).
    #[must_use]
    pub const fn is_active(&self, index: usize) -> bool {
        index < VOICE_PARAM_COUNT && self.lens[index] > 0
    }

    /// Whether any parameter carries breakpoints.
    #[must_use]
    pub const fn any_active(&self) -> bool {
        let mut index = 0;
        while index < VOICE_PARAM_COUNT {
            if self.lens[index] > 0 {
                return true;
            }
            index += 1;
        }
        false
    }

    /// Replaces parameter `index`'s automation with `(frame, value)`
    /// breakpoints, sanitizing as documented: non-finite values are skipped,
    /// frames are forced strictly increasing (a repeated frame replaces the
    /// previous value — last wins), a ramp that starts after frame 0 gets an
    /// implicit start breakpoint holding `start_value`, and everything past
    /// [`MAX_VOICE_PARAM_BREAKPOINTS`] is dropped. Out-of-range indices are
    /// ignored.
    pub fn set_breakpoints(&mut self, index: usize, start_value: f32, breakpoints: &[(u32, f32)]) {
        if index >= VOICE_PARAM_COUNT {
            return;
        }
        self.lens[index] = 0;
        for &(frame, value) in breakpoints {
            self.push_breakpoint(index, frame, value, start_value);
        }
    }

    /// Appends one sanitized breakpoint for parameter `index` per the
    /// [`Self::set_breakpoints`] rules.
    fn push_breakpoint(&mut self, index: usize, frame: u32, value: f32, start_value: f32) {
        if !value.is_finite() {
            return;
        }
        let len = usize::from(self.lens[index]);
        if len == 0 && frame > 0 {
            // The note must start somewhere: hold the trigger-time value
            // until the first real breakpoint (interpolating toward it).
            self.frames[index][0] = 0;
            self.values[index][0] = start_value;
            self.lens[index] = 1;
            return self.push_breakpoint(index, frame, value, start_value);
        }
        if len > 0 && frame <= self.frames[index][len - 1] {
            // Same (or regressed) frame: the later value wins in place.
            self.values[index][len - 1] = value;
            return;
        }
        if len >= MAX_VOICE_PARAM_BREAKPOINTS {
            return;
        }
        self.frames[index][len] = frame;
        self.values[index][len] = value;
        self.lens[index] = self.lens[index].saturating_add(1);
    }

    /// The parameter's value at `age_frames` frames after the trigger:
    /// linear interpolation between the surrounding breakpoints, the last
    /// value held after the final breakpoint, and `fallback` when the
    /// parameter carries no automation. Scans from the front — use
    /// [`Self::value_at_from`] with a persisted cursor on the per-frame
    /// render path.
    #[must_use]
    pub fn value_at(&self, index: usize, fallback: f32, age_frames: u32) -> f32 {
        let mut cursor = 0;
        self.value_at_from(index, fallback, age_frames, &mut cursor)
    }

    /// [`Self::value_at`] resuming the breakpoint scan from `cursor`, which
    /// is advanced in place; with a per-note cursor the per-frame cost is
    /// amortized O(1). Allocation-free.
    #[allow(clippy::cast_precision_loss)]
    pub fn value_at_from(
        &self,
        index: usize,
        fallback: f32,
        age_frames: u32,
        cursor: &mut u8,
    ) -> f32 {
        if index >= VOICE_PARAM_COUNT {
            return fallback;
        }
        let len = usize::from(self.lens[index]);
        if len == 0 {
            return fallback;
        }
        let frames = &self.frames[index];
        let values = &self.values[index];
        if usize::from(*cursor) >= len {
            *cursor = u8::try_from(len - 1).unwrap_or(u8::MAX);
        }
        while usize::from(*cursor) + 1 < len && frames[usize::from(*cursor) + 1] <= age_frames {
            *cursor += 1;
        }
        let at = usize::from(*cursor);
        if at + 1 >= len || age_frames <= frames[at] {
            return values[at];
        }
        let span = frames[at + 1] - frames[at];
        let progress = (age_frames - frames[at]) as f32 / span as f32;
        (values[at + 1] - values[at]).mul_add(progress, values[at])
    }
}

/// Default pooled voices per program (ADR 0009).
///
/// How many simultaneous notes one graph program can sound before further
/// triggers are dropped. User specs may override it per program via
/// [`GraphVoiceSpec::with_polyphony`].
pub const DEFAULT_GRAPH_VOICE_POLYPHONY: usize = 8;

/// The largest per-program polyphony a user spec may request. Pools are
/// pre-built and pre-warmed per program, so the ceiling keeps bank
/// construction (and the audio thread's per-frame slot scan) bounded.
pub const MAX_GRAPH_VOICE_POLYPHONY: usize = 64;

/// The longest fixed delay a [`VoiceNodeSpec::Delay`] node may request, in
/// seconds. Delay capacity is allocated per pooled voice at build time, so
/// the cap keeps bank construction memory bounded.
pub const MAX_VOICE_DELAY_SECONDS: f32 = 10.0;

/// The capacity given to a signal-driven [`VoiceNodeSpec::FractionalDelay`]
/// when the language surface does not name one, in seconds.
///
/// Modulated delays exist for chorus/flanger/doppler work in the millisecond
/// range; one second of headroom covers echo-style modulation while keeping
/// the per-voice buffer (allocated for every pooled voice at build time)
/// modest.
pub const MODULATED_VOICE_DELAY_MAX_SECONDS: f32 = 1.0;

/// The longest release tail a note may stretch to, in seconds.
///
/// Pitched sample stages ([`VoiceNodeSpec::Sample`] with a reference
/// frequency) extend a note's release at trigger time so slowed-down
/// playback sounds to its true end; the extension clamps here — the same
/// 30 s bound the language surface applies to the `release` pragma and the
/// static sample-stage tail — keeping note lifetimes (and pool residency)
/// bounded.
pub const MAX_VOICE_RELEASE_TAIL_SECONDS: f32 = 30.0;

/// Output trim applied to graph voices, matching the analog-voice headroom
/// convention in `voice.rs`.
const GRAPH_OUTPUT_TRIM: f32 = 0.35;

/// Length of the linear gain/pan ramp applied when a sounding voice is
/// stolen, in seconds.
///
/// A steal keeps the voice's graph state (so the envelope restarts click-free
/// from its current level) but swaps the note's control values; the ramp
/// smooths the gain/pan jump between the old and new note over a few
/// milliseconds so the handover never steps the output discontinuously.
pub const VOICE_STEAL_RAMP_SECONDS: f32 = 0.002;

/// Default length of the linear `p1`..`p4` ramp applied when a sounding
/// voice is retriggered or stolen, in seconds (ADR 0010 addendum follow-up).
///
/// Matches the gain/pan steal-ramp precedent ([`VOICE_STEAL_RAMP_SECONDS`]):
/// instead of jumping to the new note's parameter values — an audible zipper
/// when a parameter drives e.g. a filter cutoff — the params glide linearly
/// from the stolen note's current values and land exactly on the new note's,
/// after which the per-note sample-and-hold semantics resume. Fresh
/// (idle-voice) triggers never ramp: they start exactly at the new values.
/// Per-program override via [`GraphVoiceSpec::with_param_ramp_seconds`] (the
/// `param_ramp = seconds` pragma on the language surface).
pub const DEFAULT_PARAM_RAMP_SECONDS: f32 = VOICE_STEAL_RAMP_SECONDS;

/// The longest `p1`..`p4` steal-ramp window a program may request, in
/// seconds.
///
/// One second is already glissando territory for a per-note control; the
/// bound keeps a mistyped pragma from smearing parameters across many notes.
pub const MAX_PARAM_RAMP_SECONDS: f32 = 1.0;

/// What a program does when a trigger arrives and every pooled voice is
/// already sounding (ADR 0009 addendum).
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum StealPolicy {
    /// Steal a sounding voice, per standard synth practice: prefer the voice
    /// furthest into its release tail, else the oldest by trigger time. The
    /// default.
    #[default]
    Oldest,
    /// Never steal: triggers beyond the pool size are dropped (the original
    /// ADR 0009 behavior).
    Off,
}

/// A named, buildable graph voice program.
///
/// The builder function compiles a graph with the fixed voice interface:
/// 8 inputs (gate, freq\_hz, gain, pan, p1..p4) and 2 outputs (left, right).
#[derive(Clone, Copy, Debug)]
pub struct GraphVoiceProgram {
    token: &'static str,
    release_seconds: f32,
    build: fn(f32) -> Processor,
}

impl GraphVoiceProgram {
    /// The pattern-token this program is selected by (e.g. `"gsine"`).
    #[must_use]
    pub const fn token(&self) -> &'static str {
        self.token
    }

    /// How long the voice keeps sounding after its gate falls, in seconds.
    #[must_use]
    pub const fn release_seconds(&self) -> f32 {
        self.release_seconds
    }

    /// Compiles the program into an unprepared [`GraphVoice`].
    ///
    /// Construction may allocate; call it off the audio thread and follow up
    /// with [`GraphVoice::prepare`] before real-time use.
    #[must_use]
    pub fn build_voice(&self, sample_rate_hz: f32) -> GraphVoice {
        GraphVoice {
            processor: (self.build)(sample_rate_hz),
        }
    }

    /// The release tail length in frames at `sample_rate_hz`.
    fn release_frames(&self, sample_rate_hz: f32) -> u32 {
        release_seconds_to_frames(self.release_seconds, sample_rate_hz)
    }
}

/// The built-in graph voice programs available to the engine.
///
/// Currently one program:
///
/// - `gsine` — the gated sine voice from the graph integration suite:
///   sine carrier multiplied by a gate-driven ADSR envelope, scaled by the
///   trigger gain, and placed in the stereo field by an equal-power panner.
#[must_use]
pub const fn builtin_graph_voice_programs() -> &'static [GraphVoiceProgram] {
    const PROGRAMS: [GraphVoiceProgram; 1] = [GraphVoiceProgram {
        token: "gsine",
        release_seconds: GSINE_RELEASE_SECONDS,
        build: build_gsine,
    }];
    &PROGRAMS
}

const GSINE_ATTACK_SECONDS: f32 = 0.001;
const GSINE_DECAY_SECONDS: f32 = 0.01;
const GSINE_SUSTAIN_LEVEL: f32 = 0.7;
const GSINE_RELEASE_SECONDS: f32 = 0.02;

/// Builds the `gsine` voice graph:
/// `[gate, freq, gain, pan, p1..p4] -> [L, R]`. The built-in ignores the
/// per-note pattern parameters; a leading selector drops them.
fn build_gsine(sample_rate_hz: f32) -> Processor {
    let envelope = bind(
        adsr(sample_rate_hz),
        &[
            (1, GSINE_ATTACK_SECONDS),
            (2, GSINE_DECAY_SECONDS),
            (3, GSINE_SUSTAIN_LEVEL),
            (4, GSINE_RELEASE_SECONDS),
        ],
    )
    .unwrap_or_else(|error| panic!("gsine envelope bindings must be valid: {error}"));

    // [gate, freq, gain, pan, p1..p4] -> [freq, gate, gain, pan]
    let reorder = wire_with_inputs(&[1, 0, 2, 3], channel_index(4 + VOICE_PARAM_COUNT));
    // -> [osc, level, gain, pan]
    let sources = par(sine(sample_rate_hz), par(envelope, passthrough(2)));
    // -> [osc * level, gain, pan]
    let enveloped = par(gain_node(), passthrough(2));
    // -> [audio * gain, pan]
    let levelled = par(gain_node(), passthrough(1));
    // -> [left, right]
    let stereo = pan();

    let graph = seq(
        reorder,
        seq(
            sources,
            seq(
                enveloped,
                seq(levelled, stereo)
                    .unwrap_or_else(|error| panic!("gsine pan stage must compose: {error}")),
            )
            .unwrap_or_else(|error| panic!("gsine gain stage must compose: {error}")),
        )
        .unwrap_or_else(|error| panic!("gsine envelope stage must compose: {error}")),
    )
    .unwrap_or_else(|error| panic!("gsine input reorder must compose: {error}"));

    debug_assert_eq!(graph.inputs(), channel_index(4 + VOICE_PARAM_COUNT));
    debug_assert_eq!(graph.outputs(), 2);
    Processor::new(graph)
}

/// A reference to a signal available to a [`VoiceNodeSpec`].
///
/// Specs form a DAG over a flat node list: a node may read the per-note gate,
/// the per-note frequency, or the output of any node defined before it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VoiceSignalRef {
    /// The per-note gate (1 while the pattern event span holds, then 0).
    Gate,
    /// The per-note frequency in Hertz.
    Freq,
    /// A general-purpose per-note pattern parameter (`p1`..`p4`): index
    /// `i` is the language surface's `p{i + 1}`, sampled at trigger time and
    /// held for the note. Must be below [`VOICE_PARAM_COUNT`]. Unset
    /// parameters read [`DEFAULT_VOICE_PARAM_VALUE`].
    Param(u32),
    /// The output of the node at this index in the spec's node list.
    Node(u32),
    /// The output of the node at this index, delayed by one sample.
    ///
    /// Unlike [`Self::Node`], a feedback reference may point at the current
    /// node or a later one: it closes a feedback loop, lowered onto the
    /// recursive (`Rec`) combinator's one-sample delay.
    Feedback(u32),
}

/// The response selected from the four simultaneous outputs of the TPT
/// state-variable filter ([`crate::graph::SvfNode`]).
///
/// The node computes all four responses every sample; a
/// [`VoiceNodeSpec::Svf`] keeps exactly one of them, so the spec DAG's
/// one-output-per-node bus accounting holds.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SvfMode {
    /// 12 dB/octave low-pass.
    Lowpass,
    /// 12 dB/octave high-pass.
    Highpass,
    /// Band-pass, normalized to unity gain at the center frequency.
    Bandpass,
    /// Band-reject with a null at the center frequency.
    Notch,
}

impl SvfMode {
    /// The SVF output channel carrying this response
    /// (`[lowpass, highpass, bandpass, notch]`).
    const fn output_channel(self) -> u32 {
        match self {
            Self::Lowpass => 0,
            Self::Highpass => 1,
            Self::Bandpass => 2,
            Self::Notch => 3,
        }
    }
}

/// Which side of the corner frequency a [`VoiceNodeSpec::EqShelf`] boosts
/// (or cuts, for negative gains).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ShelfMode {
    /// Low shelf: gain applies below the corner; highs stay at unity.
    Low,
    /// High shelf: gain applies above the corner; lows stay at unity.
    High,
}

impl ShelfMode {
    /// The RBJ biquad mode implementing this shelf.
    const fn biquad_mode(self) -> BiquadMode {
        match self {
            Self::Low => BiquadMode::LowShelf,
            Self::High => BiquadMode::HighShelf,
        }
    }
}

/// One node in a declarative [`GraphVoiceSpec`].
///
/// Each variant maps onto one existing graph-module primitive; the spec is a
/// data-only wiring of that vocabulary, so it stays `Clone`/`PartialEq` and
/// can travel through [`crate::EngineCommand`]s.
#[derive(Clone, Debug, PartialEq)]
pub enum VoiceNodeSpec {
    /// A constant control value.
    Constant {
        /// The constant value produced every frame.
        value: f32,
    },
    /// A sine oscillator driven by `freq`.
    Sine {
        /// The frequency signal in Hertz.
        freq: VoiceSignalRef,
    },
    /// A band-limited saw oscillator driven by `freq`.
    Saw {
        /// The frequency signal in Hertz.
        freq: VoiceSignalRef,
    },
    /// A triangle oscillator driven by `freq`.
    Tri {
        /// The frequency signal in Hertz.
        freq: VoiceSignalRef,
    },
    /// A band-limited pulse oscillator driven by `freq` and `width`.
    Pulse {
        /// The frequency signal in Hertz.
        freq: VoiceSignalRef,
        /// The pulse width (duty cycle) signal in \[0, 1\].
        width: VoiceSignalRef,
    },
    /// Deterministic white noise.
    Noise {
        /// The PRNG seed; equal seeds produce identical noise.
        seed: u32,
    },
    /// A gate-driven ADSR envelope with fixed segment parameters.
    Adsr {
        /// The gate signal opening and closing the envelope.
        gate: VoiceSignalRef,
        /// Attack time in seconds.
        attack_s: f32,
        /// Decay time in seconds.
        decay_s: f32,
        /// Sustain level in \[0, 1\].
        sustain: f32,
        /// Release time in seconds.
        release_s: f32,
    },
    /// A gate-driven attack/release envelope with fixed segment parameters.
    Ar {
        /// The gate signal opening and closing the envelope.
        gate: VoiceSignalRef,
        /// Attack time in seconds.
        attack_s: f32,
        /// Release time in seconds.
        release_s: f32,
    },
    /// A 4-stage ladder low-pass filter.
    Lowpass {
        /// The audio signal to filter.
        input: VoiceSignalRef,
        /// The cutoff frequency signal in Hertz.
        cutoff_hz: VoiceSignalRef,
        /// The resonance signal.
        resonance: VoiceSignalRef,
    },
    /// One response of the TPT state-variable filter ([`SvfMode`] picks
    /// which), with coefficients recomputed every sample — cutoff and Q are
    /// signals and may sweep at audio rate (e.g. an LFO on the cutoff).
    Svf {
        /// The audio signal to filter.
        input: VoiceSignalRef,
        /// The cutoff/center frequency signal in Hertz (clamped to
        /// \[1, 0.49 x sample rate\] at render time).
        cutoff_hz: VoiceSignalRef,
        /// The Q signal (clamped to \[0.05, 100\] at render time).
        q: VoiceSignalRef,
        /// Which of the filter's four simultaneous responses this node
        /// outputs.
        mode: SvfMode,
    },
    /// An RBJ-cookbook peaking (bell) EQ biquad, with coefficients
    /// recomputed once per processed block from the block-start parameter
    /// values.
    EqPeak {
        /// The audio signal to filter.
        input: VoiceSignalRef,
        /// The center frequency signal in Hertz (clamped to
        /// \[1, 0.49 x sample rate\] at render time).
        freq_hz: VoiceSignalRef,
        /// The Q (bandwidth) signal (clamped to \[0.05, 100\] at render
        /// time).
        q: VoiceSignalRef,
        /// The bell gain signal in decibels — positive boosts, negative cuts
        /// (clamped to \[-40, 40\] at render time).
        gain_db: VoiceSignalRef,
    },
    /// An RBJ-cookbook shelving EQ biquad ([`ShelfMode`] picks the side),
    /// with coefficients recomputed once per processed block from the
    /// block-start parameter values.
    EqShelf {
        /// The audio signal to filter.
        input: VoiceSignalRef,
        /// The corner frequency signal in Hertz (clamped to
        /// \[1, 0.49 x sample rate\] at render time).
        freq_hz: VoiceSignalRef,
        /// The shelf slope/Q signal (clamped to \[0.05, 100\] at render
        /// time).
        q: VoiceSignalRef,
        /// The shelf gain signal in decibels — positive boosts, negative
        /// cuts (clamped to \[-40, 40\] at render time).
        gain_db: VoiceSignalRef,
        /// Which side of the corner the gain applies to.
        mode: ShelfMode,
    },
    /// Soft saturation.
    Drive {
        /// The audio signal to saturate.
        input: VoiceSignalRef,
        /// The drive amount signal.
        amount: VoiceSignalRef,
    },
    /// Multiplies two signals (ring mod, envelope application, gain).
    Mul {
        /// The left operand.
        left: VoiceSignalRef,
        /// The right operand.
        right: VoiceSignalRef,
    },
    /// Sums two signals.
    Add {
        /// The left operand.
        left: VoiceSignalRef,
        /// The right operand.
        right: VoiceSignalRef,
    },
    /// A fixed delay line.
    Delay {
        /// The audio signal to delay.
        input: VoiceSignalRef,
        /// The delay length in seconds, fixed at build time (capacity is
        /// allocated off-thread), clamped to at least one sample and capped
        /// at [`MAX_VOICE_DELAY_SECONDS`].
        seconds: f32,
    },
    /// A fractional delay line whose delay time is a signal (modulatable at
    /// audio rate) — the chorus/flanger building block.
    FractionalDelay {
        /// The audio signal to delay.
        input: VoiceSignalRef,
        /// The delay time in seconds, read every sample and clamped to
        /// \[0, `max_seconds`\] at render time.
        seconds: VoiceSignalRef,
        /// The line's capacity in seconds, fixed at build time (the buffer is
        /// allocated off-thread) and capped at [`MAX_VOICE_DELAY_SECONDS`].
        max_seconds: f32,
    },
    /// Sums any number of signals (fan-in), lowered onto the merge
    /// combinator.
    Merge {
        /// The signals to sum; must be non-empty.
        inputs: Vec<VoiceSignalRef>,
    },
    /// Playback of a preloaded sample-bank buffer.
    ///
    /// The buffer is resolved to its shared `Arc` handle when the spec is
    /// built (off-thread, so an unknown sample name errors at definition
    /// time and rendering never touches the bank). A rising `gate` edge
    /// restarts playback from the top; the level is otherwise ignored
    /// (one-shot trigger semantics, like the engine's sample voices).
    /// Playback ends at the buffer end unless `looped` is set, in which case
    /// the playhead wraps to the buffer head — hard by default, or blended
    /// over a short crossfade when `loop_crossfade` is set — and the loop
    /// sounds until the voice ends.
    Sample {
        /// The trigger signal (typically [`VoiceSignalRef::Gate`]).
        gate: VoiceSignalRef,
        /// The playback-rate signal (1.0 = native pitch), read every frame.
        /// When `pitch_reference_hz` is set the signal is a frequency in
        /// Hertz instead (typically [`VoiceSignalRef::Freq`]).
        rate: VoiceSignalRef,
        /// The preloaded mono buffer, shared with the sample bank.
        sample: PlaybackSample,
        /// Hard-wrap at the buffer end instead of stopping.
        looped: bool,
        /// Crossfade the loop wrap instead of hard-wrapping: a short linear
        /// (constant-gain) fade — 5 ms of source material, capped at 10% of
        /// the buffer — blends the loop tail into the head so non-zero-
        /// crossing loops stop clicking
        /// ([`crate::graph::sample_player_looped_crossfaded`]). Only
        /// meaningful when `looped` is set.
        loop_crossfade: bool,
        /// When set, the `rate` signal carries a frequency in Hertz and the
        /// playback rate is `rate / reference` — native at the reference
        /// frequency, an octave above it at exactly 2.0. Must be finite and
        /// positive. Notes below the reference play slower than native rate,
        /// so the bank floors their release tail at trigger time with the
        /// actual playback length (`duration x reference / freq`, capped at
        /// [`MAX_VOICE_RELEASE_TAIL_SECONDS`]) — the slowed one-shot sounds
        /// to its true end instead of truncating at the native-rate end.
        pitch_reference_hz: Option<f32>,
    },
}

impl VoiceNodeSpec {
    /// The signal references this node reads, in primitive input order.
    fn input_refs(&self) -> Vec<VoiceSignalRef> {
        match self {
            Self::Constant { .. } | Self::Noise { .. } => Vec::new(),
            Self::Sine { freq } | Self::Saw { freq } | Self::Tri { freq } => vec![*freq],
            Self::Pulse { freq, width } => vec![*freq, *width],
            Self::Adsr { gate, .. } | Self::Ar { gate, .. } => vec![*gate],
            Self::Lowpass {
                input,
                cutoff_hz,
                resonance,
            } => vec![*input, *cutoff_hz, *resonance],
            Self::Svf {
                input,
                cutoff_hz,
                q,
                ..
            } => vec![*input, *cutoff_hz, *q],
            Self::EqPeak {
                input,
                freq_hz,
                q,
                gain_db,
            }
            | Self::EqShelf {
                input,
                freq_hz,
                q,
                gain_db,
                ..
            } => vec![*input, *freq_hz, *q, *gain_db],
            Self::Drive { input, amount } => vec![*input, *amount],
            Self::Mul { left, right } | Self::Add { left, right } => vec![*left, *right],
            Self::Delay { input, .. } => vec![*input],
            Self::FractionalDelay { input, seconds, .. } => vec![*input, *seconds],
            Self::Merge { inputs } => inputs.clone(),
            Self::Sample { gate, rate, .. } => vec![*gate, *rate],
        }
    }

    /// Applies `f` to every signal reference this node reads, in place.
    ///
    /// Compilers use this to patch placeholder references (e.g. rewriting a
    /// feedback loop's back-edge once the loop's root node index is known).
    pub fn map_refs(&mut self, mut f: impl FnMut(&mut VoiceSignalRef)) {
        match self {
            Self::Constant { .. } | Self::Noise { .. } => {}
            Self::Sine { freq } | Self::Saw { freq } | Self::Tri { freq } => f(freq),
            Self::Pulse { freq, width } => {
                f(freq);
                f(width);
            }
            Self::Adsr { gate, .. } | Self::Ar { gate, .. } => f(gate),
            Self::Lowpass {
                input,
                cutoff_hz,
                resonance,
            } => {
                f(input);
                f(cutoff_hz);
                f(resonance);
            }
            Self::Svf {
                input,
                cutoff_hz,
                q,
                ..
            } => {
                f(input);
                f(cutoff_hz);
                f(q);
            }
            Self::EqPeak {
                input,
                freq_hz,
                q,
                gain_db,
            }
            | Self::EqShelf {
                input,
                freq_hz,
                q,
                gain_db,
                ..
            } => {
                f(input);
                f(freq_hz);
                f(q);
                f(gain_db);
            }
            Self::Drive { input, amount } => {
                f(input);
                f(amount);
            }
            Self::Mul { left, right } | Self::Add { left, right } => {
                f(left);
                f(right);
            }
            Self::Delay { input, .. } => f(input),
            Self::FractionalDelay { input, seconds, .. } => {
                f(input);
                f(seconds);
            }
            Self::Merge { inputs } => {
                for input in inputs {
                    f(input);
                }
            }
            Self::Sample { gate, rate, .. } => {
                f(gate);
                f(rate);
            }
        }
    }

    /// Whether every fixed parameter is finite (and non-negative where the
    /// primitive expects a duration or level).
    fn parameters_are_valid(&self) -> bool {
        match self {
            Self::Constant { value } => value.is_finite(),
            Self::Adsr {
                attack_s,
                decay_s,
                sustain,
                release_s,
                ..
            } => [*attack_s, *decay_s, *sustain, *release_s]
                .iter()
                .all(|value| value.is_finite() && *value >= 0.0),
            Self::Ar {
                attack_s,
                release_s,
                ..
            } => [*attack_s, *release_s]
                .iter()
                .all(|value| value.is_finite() && *value >= 0.0),
            Self::Delay { seconds, .. } => seconds.is_finite() && *seconds >= 0.0,
            Self::FractionalDelay { max_seconds, .. } => {
                max_seconds.is_finite() && *max_seconds > 0.0
            }
            Self::Sample {
                pitch_reference_hz, ..
            } => {
                pitch_reference_hz.is_none_or(|reference| reference.is_finite() && reference > 0.0)
            }
            Self::Sine { .. }
            | Self::Saw { .. }
            | Self::Tri { .. }
            | Self::Pulse { .. }
            | Self::Noise { .. }
            | Self::Lowpass { .. }
            | Self::Svf { .. }
            | Self::EqPeak { .. }
            | Self::EqShelf { .. }
            | Self::Drive { .. }
            | Self::Mul { .. }
            | Self::Add { .. }
            | Self::Merge { .. } => true,
        }
    }
}

/// Validation errors for [`GraphVoiceSpec::new`].
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum GraphVoiceSpecError {
    /// The pattern token was empty or contained whitespace.
    #[error("voice program token must be a non-empty single word")]
    InvalidToken,
    /// The spec contained no nodes.
    #[error("voice program body must contain at least one node")]
    EmptyBody,
    /// A node referenced itself or a later node.
    #[error("voice node {node} references node {reference}, which is not defined before it")]
    ForwardReference {
        /// The index of the offending node.
        node: usize,
        /// The out-of-range reference.
        reference: usize,
    },
    /// The output referenced a node index outside the node list.
    #[error("voice program output references node {reference}, which does not exist")]
    OutputOutOfRange {
        /// The out-of-range reference.
        reference: usize,
    },
    /// A node carried a non-finite (or negative duration/level) parameter.
    #[error("voice node {node} has a non-finite or negative parameter")]
    InvalidParameter {
        /// The index of the offending node.
        node: usize,
    },
    /// The release tail was not finite and non-negative.
    #[error("voice program release must be finite and non-negative")]
    InvalidRelease,
    /// A feedback reference pointed outside the node list.
    #[error("voice node {node} takes feedback from node {reference}, which does not exist")]
    FeedbackOutOfRange {
        /// The index of the offending node.
        node: usize,
        /// The out-of-range feedback reference.
        reference: usize,
    },
    /// A merge node listed no input signals.
    #[error("voice node {node} merges zero signals")]
    EmptyMerge {
        /// The index of the offending node.
        node: usize,
    },
    /// A delay node exceeded [`MAX_VOICE_DELAY_SECONDS`].
    #[error("voice node {node} delays by more than {MAX_VOICE_DELAY_SECONDS} seconds")]
    DelayTooLong {
        /// The index of the offending node.
        node: usize,
    },
    /// The requested polyphony was outside `1..=MAX_GRAPH_VOICE_POLYPHONY`.
    #[error("voice polyphony must be between 1 and {MAX_GRAPH_VOICE_POLYPHONY}, got {requested}")]
    InvalidPolyphony {
        /// The rejected pool size.
        requested: usize,
    },
    /// The requested `p1`..`p4` steal-ramp window was non-finite or outside
    /// `0..=MAX_PARAM_RAMP_SECONDS`.
    #[error(
        "voice param_ramp must be a finite number of seconds between 0 and \
         {MAX_PARAM_RAMP_SECONDS}"
    )]
    InvalidParamRamp,
    /// A reference named a per-note pattern parameter index at or above
    /// [`VOICE_PARAM_COUNT`].
    #[error(
        "voice node {node} reads pattern parameter {reference}, but only \
         {VOICE_PARAM_COUNT} parameters exist"
    )]
    ParamOutOfRange {
        /// The index of the offending node (the node count when the spec's
        /// output reference is at fault).
        node: usize,
        /// The out-of-range parameter index.
        reference: usize,
    },
}

/// A declarative, user-definable graph voice program.
///
/// Where [`GraphVoiceProgram`] describes a built-in voice through a static
/// builder function, a `GraphVoiceSpec` is pure data: a flat DAG of
/// [`VoiceNodeSpec`]s over the existing graph vocabulary, validated at
/// construction so compilation cannot fail. Compilation lowers each node onto
/// its graph-module primitive and wires the DAG with `seq`/`par`/`wire`
/// combinators into the fixed voice interface
/// `[gate, freq_hz, gain, pan, p1..p4] -> [left, right]` (the per-trigger
/// gain and equal-power pan stages are appended automatically; `p1..p4` are
/// the per-note pattern parameters of the ADR 0010 addendum).
///
/// Construction and compilation may allocate and must happen off the audio
/// thread; the compiled [`GraphVoice`] follows the pooled discipline of
/// ADR 0009.
#[derive(Clone, Debug, PartialEq)]
pub struct GraphVoiceSpec {
    token: Box<str>,
    release_seconds: f32,
    nodes: Vec<VoiceNodeSpec>,
    output: VoiceSignalRef,
    polyphony: usize,
    steal: StealPolicy,
    param_ramp_seconds: f32,
}

impl GraphVoiceSpec {
    /// Validates and builds a voice spec.
    ///
    /// # Errors
    ///
    /// Returns a [`GraphVoiceSpecError`] when the token is not a single word,
    /// the body is empty, a node references itself or a later node, the
    /// output reference is out of range, or a parameter is invalid.
    pub fn new(
        token: impl Into<Box<str>>,
        release_seconds: f32,
        nodes: Vec<VoiceNodeSpec>,
        output: VoiceSignalRef,
    ) -> Result<Self, GraphVoiceSpecError> {
        let token = token.into();
        if token.is_empty() || token.chars().any(char::is_whitespace) {
            return Err(GraphVoiceSpecError::InvalidToken);
        }
        if nodes.is_empty() {
            return Err(GraphVoiceSpecError::EmptyBody);
        }
        if !(release_seconds.is_finite() && release_seconds >= 0.0) {
            return Err(GraphVoiceSpecError::InvalidRelease);
        }

        for (index, node) in nodes.iter().enumerate() {
            if !node.parameters_are_valid() {
                return Err(GraphVoiceSpecError::InvalidParameter { node: index });
            }
            if let VoiceNodeSpec::Delay { seconds, .. } = node
                && *seconds > MAX_VOICE_DELAY_SECONDS
            {
                return Err(GraphVoiceSpecError::DelayTooLong { node: index });
            }
            if let VoiceNodeSpec::FractionalDelay { max_seconds, .. } = node
                && *max_seconds > MAX_VOICE_DELAY_SECONDS
            {
                return Err(GraphVoiceSpecError::DelayTooLong { node: index });
            }
            if let VoiceNodeSpec::Merge { inputs } = node
                && inputs.is_empty()
            {
                return Err(GraphVoiceSpecError::EmptyMerge { node: index });
            }
            for reference in node.input_refs() {
                match reference {
                    VoiceSignalRef::Node(target) if target as usize >= index => {
                        return Err(GraphVoiceSpecError::ForwardReference {
                            node: index,
                            reference: target as usize,
                        });
                    }
                    VoiceSignalRef::Feedback(target) if target as usize >= nodes.len() => {
                        return Err(GraphVoiceSpecError::FeedbackOutOfRange {
                            node: index,
                            reference: target as usize,
                        });
                    }
                    VoiceSignalRef::Param(target) if target as usize >= VOICE_PARAM_COUNT => {
                        return Err(GraphVoiceSpecError::ParamOutOfRange {
                            node: index,
                            reference: target as usize,
                        });
                    }
                    _ => {}
                }
            }
        }

        if let VoiceSignalRef::Node(target) | VoiceSignalRef::Feedback(target) = output
            && target as usize >= nodes.len()
        {
            return Err(GraphVoiceSpecError::OutputOutOfRange {
                reference: target as usize,
            });
        }
        if let VoiceSignalRef::Param(target) = output
            && target as usize >= VOICE_PARAM_COUNT
        {
            return Err(GraphVoiceSpecError::ParamOutOfRange {
                node: nodes.len(),
                reference: target as usize,
            });
        }

        Ok(Self {
            token,
            release_seconds,
            nodes,
            output,
            polyphony: DEFAULT_GRAPH_VOICE_POLYPHONY,
            steal: StealPolicy::default(),
            param_ramp_seconds: DEFAULT_PARAM_RAMP_SECONDS,
        })
    }

    /// Overrides the program's pooled polyphony (how many simultaneous notes
    /// it can sound; further triggers are dropped).
    ///
    /// # Errors
    ///
    /// Returns [`GraphVoiceSpecError::InvalidPolyphony`] when `polyphony` is
    /// zero or exceeds [`MAX_GRAPH_VOICE_POLYPHONY`].
    pub fn with_polyphony(mut self, polyphony: usize) -> Result<Self, GraphVoiceSpecError> {
        if !(1..=MAX_GRAPH_VOICE_POLYPHONY).contains(&polyphony) {
            return Err(GraphVoiceSpecError::InvalidPolyphony {
                requested: polyphony,
            });
        }
        self.polyphony = polyphony;
        Ok(self)
    }

    /// How many pooled voices this program gets when a bank is built.
    #[must_use]
    pub const fn polyphony(&self) -> usize {
        self.polyphony
    }

    /// Overrides what the program does when a trigger arrives and its pool is
    /// exhausted (default: [`StealPolicy::Oldest`]).
    #[must_use]
    pub const fn with_steal_policy(mut self, steal: StealPolicy) -> Self {
        self.steal = steal;
        self
    }

    /// The program's pool-exhaustion policy.
    #[must_use]
    pub const fn steal_policy(&self) -> StealPolicy {
        self.steal
    }

    /// Overrides how long the per-note pattern parameters (`p1`..`p4`) take
    /// to ramp from a stolen note's current values to the new note's, in
    /// seconds (default: [`DEFAULT_PARAM_RAMP_SECONDS`], the 2 ms gain/pan
    /// steal-ramp precedent).
    ///
    /// The window only applies to the stolen/retriggered path; fresh
    /// (idle-voice) triggers always start exactly at the new values. A zero
    /// window lands on the very next rendered frame (the ramp is floored at
    /// one frame).
    ///
    /// # Errors
    ///
    /// Returns [`GraphVoiceSpecError::InvalidParamRamp`] when `seconds` is
    /// non-finite, negative, or exceeds [`MAX_PARAM_RAMP_SECONDS`].
    pub fn with_param_ramp_seconds(mut self, seconds: f32) -> Result<Self, GraphVoiceSpecError> {
        if !(seconds.is_finite() && (0.0..=MAX_PARAM_RAMP_SECONDS).contains(&seconds)) {
            return Err(GraphVoiceSpecError::InvalidParamRamp);
        }
        self.param_ramp_seconds = seconds;
        Ok(self)
    }

    /// The program's `p1`..`p4` steal-ramp window, in seconds.
    #[must_use]
    pub const fn param_ramp_seconds(&self) -> f32 {
        self.param_ramp_seconds
    }

    /// The pattern-token this program is selected by.
    #[must_use]
    pub fn token(&self) -> &str {
        &self.token
    }

    /// How long the voice keeps sounding after its gate falls, in seconds.
    #[must_use]
    pub const fn release_seconds(&self) -> f32 {
        self.release_seconds
    }

    /// Compiles the spec into an unprepared [`GraphVoice`].
    ///
    /// Construction may allocate; call it off the audio thread and follow up
    /// with [`GraphVoice::prepare`] before real-time use.
    #[must_use]
    pub fn build_voice(&self, sample_rate_hz: f32) -> GraphVoice {
        GraphVoice {
            processor: self.build_processor(sample_rate_hz),
        }
    }

    /// The release tail length in frames at `sample_rate_hz`.
    fn release_frames(&self, sample_rate_hz: f32) -> u32 {
        release_seconds_to_frames(self.release_seconds, sample_rate_hz)
    }

    /// The worst-case playback-length product over the spec's pitched sample
    /// nodes — the largest `duration_seconds x pitch_reference_hz`, in
    /// Hertz-seconds — or 0 when the spec has none.
    ///
    /// Dividing the product by a note's frequency gives the longest pitched
    /// playback that note starts (`duration x reference / freq` is node
    /// playback time at rate `freq / reference`). The rate depends on the
    /// triggering note and is unknown here at build time, so the bank keeps
    /// this per-program constant and floors each note's release tail with it
    /// at trigger time.
    #[allow(clippy::cast_possible_truncation)]
    fn pitched_sample_tail_hz_seconds(&self) -> f32 {
        self.nodes
            .iter()
            .filter_map(|node| match node {
                VoiceNodeSpec::Sample {
                    sample,
                    pitch_reference_hz: Some(reference),
                    ..
                } => Some((sample.duration_seconds() * f64::from(*reference)) as f32),
                _ => None,
            })
            .fold(0.0, f32::max)
    }

    /// The distinct node indices read through [`VoiceSignalRef::Feedback`]
    /// references, sorted. Each becomes one channel of the recursive
    /// combinator's one-sample feedback path.
    fn feedback_taps(&self) -> Vec<u32> {
        let mut taps: Vec<u32> = Vec::new();
        let mut visit = |reference: VoiceSignalRef| {
            if let VoiceSignalRef::Feedback(index) = reference
                && !taps.contains(&index)
            {
                taps.push(index);
            }
        };
        for node in &self.nodes {
            for reference in node.input_refs() {
                visit(reference);
            }
        }
        visit(self.output);
        taps.sort_unstable();
        taps
    }

    /// Lowers the spec DAG onto graph combinators.
    ///
    /// The graph threads a growing signal bus through one stage per node.
    /// Before stage `k` the bus is `[out_{k-1}, .., out_0, tap_0, ..,
    /// tap_{K-1}, gate, freq, gain, pan, p1..p4]`, where the `tap` channels
    /// carry the one-sample-delayed outputs of the nodes read through
    /// feedback references and `p1..p4` are the per-note pattern parameters;
    /// the stage wires the node's inputs to the front (a `wire` node may
    /// duplicate bus channels), runs the node in parallel with a passthrough
    /// of the whole bus, and thereby prepends its output.
    ///
    /// When the spec contains feedback references the whole DAG becomes the
    /// body of a recursive (`Rec`) composition: the body re-exposes each
    /// tapped node's live output first, and an identity feedback path delays
    /// those channels by one sample into the tap inputs. A final selector
    /// feeds `[audio, gain, pan]` through the shared gain and equal-power pan
    /// stages.
    fn build_processor(&self, sample_rate_hz: f32) -> Processor {
        let taps = self.feedback_taps();
        let node_count = self.nodes.len();
        let tap_count = taps.len();

        let mut stages = self
            .nodes
            .iter()
            .enumerate()
            .map(|(index, node)| Self::build_stage(index, node, &taps, sample_rate_hz));
        let first = stages
            .next()
            .unwrap_or_else(|| unreachable!("validated specs have at least one node"));
        let dag = stages.fold(first, |acc, stage| {
            seq(acc, stage)
                .unwrap_or_else(|error| panic!("voice spec stages must compose: {error}"))
        });

        let output_channel = bus_channel(node_count, &taps, self.output);
        let gain_channel = channel_index(node_count + tap_count + 2);
        let pan_channel = channel_index(node_count + tap_count + 3);
        // The full bus width after every stage: node outputs, feedback taps,
        // and the fixed [gate, freq, gain, pan, p1..p4] tail. The output
        // selectors must name it explicitly, since the pattern-parameter
        // channels they drop sit above every selected channel.
        let bus_width = channel_index(node_count + tap_count + 4 + VOICE_PARAM_COUNT);

        let selected = if taps.is_empty() {
            let select = wire_with_inputs(&[output_channel, gain_channel, pan_channel], bus_width);
            seq(dag, select)
                .unwrap_or_else(|error| panic!("voice spec output selector must compose: {error}"))
        } else {
            // Body outputs: every tapped node's live value first (the
            // feedback path re-reads them, delayed one sample), then the
            // audio output and the gain/pan controls.
            let mut selection: Vec<u32> = taps
                .iter()
                .map(|&tap| bus_channel(node_count, &taps, VoiceSignalRef::Node(tap)))
                .collect();
            selection.extend([output_channel, gain_channel, pan_channel]);
            let body = seq(dag, wire_with_inputs(&selection, bus_width)).unwrap_or_else(|error| {
                panic!("voice spec loop body selector must compose: {error}")
            });
            let looped = feedback(body, passthrough(channel_index(tap_count)))
                .unwrap_or_else(|error| panic!("voice spec feedback loop must compose: {error}"));
            // Drop the tap channels: [taps.., audio, gain, pan] -> [audio, gain, pan].
            let drop_taps = wire(&[
                channel_index(tap_count),
                channel_index(tap_count + 1),
                channel_index(tap_count + 2),
            ]);
            seq(looped, drop_taps)
                .unwrap_or_else(|error| panic!("voice spec tap dropper must compose: {error}"))
        };

        // [audio, gain, pan] -> [audio * gain, pan] -> [left, right]
        let levelled = par(gain_node(), passthrough(1));
        let tail = seq(levelled, pan())
            .unwrap_or_else(|error| panic!("voice spec pan stage must compose: {error}"));
        let graph = seq(selected, tail)
            .unwrap_or_else(|error| panic!("voice spec output stage must compose: {error}"));
        debug_assert_eq!(graph.inputs(), channel_index(4 + VOICE_PARAM_COUNT));
        debug_assert_eq!(graph.outputs(), 2);
        Processor::new(graph)
    }

    /// Builds the stage for node `index`: input wiring followed by the node
    /// running in parallel with a passthrough of the whole bus.
    fn build_stage(index: usize, node: &VoiceNodeSpec, taps: &[u32], sample_rate_hz: f32) -> Seq {
        let bus_width = index + taps.len() + 4 + VOICE_PARAM_COUNT;
        let mut mapping: Vec<u32> = node
            .input_refs()
            .iter()
            .map(|reference| bus_channel(index, taps, *reference))
            .collect();
        mapping.extend((0..bus_width).map(channel_index));
        let inputs = wire(&mapping);

        let bus = channel_index(bus_width);
        let staged = match node {
            VoiceNodeSpec::Constant { value } => par(constant(*value), passthrough(bus)),
            VoiceNodeSpec::Sine { .. } => par(sine(sample_rate_hz), passthrough(bus)),
            VoiceNodeSpec::Saw { .. } => par(saw(sample_rate_hz), passthrough(bus)),
            VoiceNodeSpec::Tri { .. } => par(tri(sample_rate_hz), passthrough(bus)),
            VoiceNodeSpec::Pulse { .. } => par(pulse(sample_rate_hz), passthrough(bus)),
            VoiceNodeSpec::Noise { seed } => par(noise(*seed), passthrough(bus)),
            VoiceNodeSpec::Adsr {
                attack_s,
                decay_s,
                sustain,
                release_s,
                ..
            } => {
                let envelope = bind(
                    adsr(sample_rate_hz),
                    &[
                        (1, *attack_s),
                        (2, *decay_s),
                        (3, *sustain),
                        (4, *release_s),
                    ],
                )
                .unwrap_or_else(|error| panic!("adsr bindings are valid by construction: {error}"));
                par(envelope, passthrough(bus))
            }
            VoiceNodeSpec::Ar {
                attack_s,
                release_s,
                ..
            } => {
                let envelope = bind(ar(sample_rate_hz), &[(1, *attack_s), (2, *release_s)])
                    .unwrap_or_else(|error| {
                        panic!("ar bindings are valid by construction: {error}")
                    });
                par(envelope, passthrough(bus))
            }
            VoiceNodeSpec::Lowpass { .. } => par(ladder_filter(sample_rate_hz), passthrough(bus)),
            VoiceNodeSpec::Svf { mode, .. } => {
                // The SVF computes [lowpass, highpass, bandpass, notch]
                // simultaneously; a fixed-width wire keeps the selected
                // response and drops the rest, preserving the bus's
                // one-output-per-node shape.
                let select = wire_with_inputs(&[mode.output_channel()], 4);
                let filter = seq(svf(sample_rate_hz), select)
                    .unwrap_or_else(|error| panic!("svf response selector must compose: {error}"));
                par(filter, passthrough(bus))
            }
            VoiceNodeSpec::EqPeak { .. } => par(
                biquad(sample_rate_hz, BiquadMode::Peaking),
                passthrough(bus),
            ),
            VoiceNodeSpec::EqShelf { mode, .. } => {
                par(biquad(sample_rate_hz, mode.biquad_mode()), passthrough(bus))
            }
            VoiceNodeSpec::Drive { .. } => par(soft_sat(), passthrough(bus)),
            VoiceNodeSpec::Mul { .. } => par(gain_node(), passthrough(bus)),
            VoiceNodeSpec::Add { .. } => par(sum(2), passthrough(bus)),
            VoiceNodeSpec::Delay { seconds, .. } => par(
                delay_line(delay_seconds_to_samples(*seconds, sample_rate_hz)),
                passthrough(bus),
            ),
            VoiceNodeSpec::FractionalDelay { max_seconds, .. } => {
                par(fdelay(sample_rate_hz, *max_seconds), passthrough(bus))
            }
            VoiceNodeSpec::Merge { inputs } => {
                let width = channel_index(inputs.len());
                let fan_in = merge(passthrough(width), passthrough(1))
                    .unwrap_or_else(|error| panic!("voice merge fan-in must compose: {error}"));
                par(fan_in, passthrough(bus))
            }
            VoiceNodeSpec::Sample {
                sample,
                looped,
                loop_crossfade,
                pitch_reference_hz,
                ..
            } => par(
                sample_player_with_options(
                    sample,
                    sample_rate_hz,
                    *looped,
                    *loop_crossfade,
                    *pitch_reference_hz,
                ),
                passthrough(bus),
            ),
        };

        seq(inputs, staged)
            .unwrap_or_else(|error| panic!("voice spec stage wiring must compose: {error}"))
    }
}

/// The bus channel carrying `reference` when `prepended` node outputs sit in
/// front of the feedback tap channels and the fixed
/// `[gate, freq, gain, pan, p1..p4]` tail.
fn bus_channel(prepended: usize, taps: &[u32], reference: VoiceSignalRef) -> u32 {
    match reference {
        VoiceSignalRef::Gate => channel_index(prepended + taps.len()),
        VoiceSignalRef::Freq => channel_index(prepended + taps.len() + 1),
        // Gain and pan occupy the two channels after freq; the per-note
        // pattern parameters follow them.
        VoiceSignalRef::Param(index) => channel_index(prepended + taps.len() + 4 + index as usize),
        VoiceSignalRef::Node(index) => {
            // Outputs are prepended, so node j sits at prepended - 1 - j.
            channel_index(prepended - 1 - index as usize)
        }
        VoiceSignalRef::Feedback(index) => {
            let position = taps
                .iter()
                .position(|&tap| tap == index)
                .unwrap_or_else(|| unreachable!("feedback references are collected as taps"));
            channel_index(prepended + position)
        }
    }
}

/// Converts a fixed delay in seconds to whole samples, clamped to at least
/// one (the delay-line primitive's minimum).
#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss
)]
fn delay_seconds_to_samples(seconds: f32, sample_rate_hz: f32) -> usize {
    let samples = (seconds * sample_rate_hz).round();
    if samples.is_finite() && samples > 1.0 {
        samples as usize
    } else {
        1
    }
}

fn channel_index(index: usize) -> u32 {
    u32::try_from(index).unwrap_or_else(|_| panic!("voice spec bus width does not fit in u32"))
}

/// Converts a release tail in seconds to frames, clamped to at least one.
#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss
)]
fn release_seconds_to_frames(release_seconds: f32, sample_rate_hz: f32) -> u32 {
    let frames = (release_seconds * sample_rate_hz).ceil();
    if frames.is_finite() && frames > 0.0 {
        frames.min(u32::MAX as f32) as u32
    } else {
        1
    }
}

/// A compiled, per-note instance of a graph voice program.
///
/// The engine owns these in a fixed pool. All allocation happens in
/// [`GraphVoiceProgram::build_voice`] and the first block of
/// [`Self::prepare`]; after that, [`Self::process_frame`] and [`Self::reset`]
/// are allocation-free and lock-free.
#[derive(Debug)]
pub struct GraphVoice {
    processor: Processor,
}

impl GraphVoice {
    /// Warms the processor so lazy-but-once scratch buffers are grown off the
    /// audio thread, then resets all state.
    ///
    /// The engine renders one frame at a time, so a single one-frame block is
    /// sufficient to reach every scratch buffer in the graph.
    pub fn prepare(&mut self) {
        let _ = self.process_frame(0.0, 0.0, 0.0, 0.0);
        self.reset();
    }

    /// Renders one stereo frame with the given control values and the
    /// default (all-zero) per-note pattern parameters.
    ///
    /// Parameters flow as signals (ADR 0004): the gate, frequency, gain, and
    /// pan are one-frame input channels. This path must stay allocation-free.
    pub fn process_frame(&mut self, gate: f32, freq_hz: f32, gain: f32, pan: f32) -> (f32, f32) {
        self.process_frame_with_params(
            gate,
            freq_hz,
            gain,
            pan,
            &[DEFAULT_VOICE_PARAM_VALUE; VOICE_PARAM_COUNT],
        )
    }

    /// Renders one stereo frame with the given control values and per-note
    /// pattern parameters (`p1`..`p4`, ADR 0010 addendum).
    ///
    /// Parameters flow as signals (ADR 0004): every control is a one-frame
    /// input channel. This path must stay allocation-free.
    pub fn process_frame_with_params(
        &mut self,
        gate: f32,
        freq_hz: f32,
        gain: f32,
        pan: f32,
        params: &[f32; VOICE_PARAM_COUNT],
    ) -> (f32, f32) {
        let gate_buf = [gate];
        let freq_buf = [freq_hz];
        let gain_buf = [gain];
        let pan_buf = [pan];
        let param_bufs = params.map(|value| [value]);
        let inputs: [&[f32]; 4 + VOICE_PARAM_COUNT] = [
            &gate_buf,
            &freq_buf,
            &gain_buf,
            &pan_buf,
            &param_bufs[0],
            &param_bufs[1],
            &param_bufs[2],
            &param_bufs[3],
        ];
        let mut left = [0.0_f32];
        let mut right = [0.0_f32];
        {
            let mut outputs: [&mut [f32]; 2] = [&mut left, &mut right];
            self.processor.process(&inputs, &mut outputs, 1);
        }
        (left[0], right[0])
    }

    /// Resets all internal DSP state without releasing buffers.
    pub fn reset(&mut self) {
        self.processor.reset();
    }
}

/// One currently sounding note on a pooled graph voice.
#[derive(Clone, Copy, Debug)]
struct GraphVoiceNote {
    track_id: TrackId,
    gate_frames_remaining: u32,
    release_frames_remaining: u32,
    freq_hz: f32,
    gain: f32,
    pan: f32,
    /// The per-note pattern parameters (`p1`..`p4`), sampled at trigger time
    /// (ADR 0010 addendum). Without automation they hold for the note, and —
    /// like gain/pan, unlike the frequency, which switches immediately — a
    /// steal ramps them in place linearly from the stolen note's current
    /// values over the program's param-ramp window, landing exactly on the
    /// new note's values. When `ramps` carries breakpoints, these instead
    /// stay fixed at the note-start values (the automation's
    /// pre-first-breakpoint base) and the envelope governs the note
    /// (ADR 0012); a stolen voice then glides `current_params` onto the
    /// envelope rather than mutating these.
    params: [f32; VOICE_PARAM_COUNT],
    /// Per-note parameter breakpoint automation, stamped at trigger time
    /// (ADR 0012). Fixed-size storage; playback interpolates linearly.
    ramps: VoiceParamRamps,
    /// Amortized-O(1) breakpoint scan positions for `ramps`, one per
    /// parameter.
    ramp_cursors: [u8; VOICE_PARAM_COUNT],
    /// Frames rendered since the trigger; drives the automation playback.
    age_frames: u32,
    /// Monotonic per-bank trigger counter, set when the note starts; the
    /// steal policy uses it as the note's age (smaller = older).
    trigger_seq: u64,
    /// Frames the gate is forced low after a steal (one frame), so the
    /// graph's gate-driven envelopes see a falling then rising edge and
    /// restart their attack click-free from the current level.
    retrigger_gap_frames: u32,
    /// Remaining frames of the post-steal linear gain/pan ramp from the
    /// stolen note's control values to this note's.
    ramp_frames_remaining: u32,
    gain_step: f32,
    pan_step: f32,
    target_gain: f32,
    target_pan: f32,
    /// Remaining frames of the post-steal linear `p1`..`p4` ramp from the
    /// stolen note's current parameter values to this note's. Kept separate
    /// from the gain/pan counter so the `param_ramp` pragma can widen this
    /// window without touching the fixed 2 ms gain/pan handover.
    param_ramp_frames_remaining: u32,
    param_steps: [f32; VOICE_PARAM_COUNT],
    target_params: [f32; VOICE_PARAM_COUNT],
    /// The `p1`..`p4` values actually shipped to the graph on the most
    /// recent rendered frame — mid-steal-ramp and mid-automation included.
    /// The steal handover reads the victim's field so back-to-back steals
    /// chain smoothly from whatever is audible, and a stolen voice with
    /// automation carries its glide state here (steal x automation
    /// composition, ADR 0012).
    current_params: [f32; VOICE_PARAM_COUNT],
}

impl GraphVoiceNote {
    /// A note starting on an idle pooled voice: controls apply immediately.
    #[allow(clippy::too_many_arguments)]
    fn fresh(
        track_id: TrackId,
        gate_frames: u32,
        release_frames: u32,
        freq_hz: f32,
        gain: f32,
        pan: f32,
        params: [f32; VOICE_PARAM_COUNT],
        ramps: &VoiceParamRamps,
        trigger_seq: u64,
    ) -> Self {
        Self {
            track_id,
            gate_frames_remaining: gate_frames.max(1),
            release_frames_remaining: release_frames,
            freq_hz,
            gain,
            pan,
            params,
            ramps: *ramps,
            ramp_cursors: [0; VOICE_PARAM_COUNT],
            age_frames: 0,
            trigger_seq,
            retrigger_gap_frames: 0,
            ramp_frames_remaining: 0,
            gain_step: 0.0,
            pan_step: 0.0,
            target_gain: gain,
            target_pan: pan,
            param_ramp_frames_remaining: 0,
            param_steps: [0.0; VOICE_PARAM_COUNT],
            target_params: params,
            current_params: params,
        }
    }

    /// Converts a fresh note into one stealing a sounding voice: the gate
    /// drops for one frame so the envelope retriggers from its current level,
    /// gain/pan ramp linearly from the stolen note's current values over
    /// `ramp_frames`, and the per-note pattern parameters (`p1`..`p4`) glide
    /// the same way over `param_ramp_frames` — linearly onto the new note's
    /// held values, or onto its automation envelope when it carries
    /// breakpoints (ADR 0012).
    #[allow(clippy::cast_precision_loss)]
    fn begin_steal_handover(
        &mut self,
        stolen_from: &Self,
        ramp_frames: u32,
        param_ramp_frames: u32,
    ) {
        let ramp_frames = ramp_frames.max(1);
        self.retrigger_gap_frames = 1;
        self.ramp_frames_remaining = ramp_frames;
        self.gain_step = (self.target_gain - stolen_from.gain) / ramp_frames as f32;
        self.pan_step = (self.target_pan - stolen_from.pan) / ramp_frames as f32;
        self.gain = stolen_from.gain;
        self.pan = stolen_from.pan;

        // Ramp state is plain f32 field arithmetic (current, target, step),
        // stamped here at trigger time — nothing on this path allocates. The
        // stolen note's `current_params` field is its audible value even
        // mid-ramp or mid-automation, so back-to-back steals chain smoothly.
        let param_ramp_frames = param_ramp_frames.max(1);
        self.param_ramp_frames_remaining = param_ramp_frames;
        if self.ramps.any_active() {
            // Steal x automation composition (ADR 0012): when the new note
            // carries breakpoints, the handover glides `current_params` from
            // the stolen note's current values ONTO the new note's automation
            // envelope (each rendered frame closes 1/remaining of the gap to
            // the envelope's current value — see `render_frame`). `params`
            // keeps the new note's trigger-time values untouched: they are
            // the envelope's pre-first-breakpoint hold/base. Once the glide
            // lands — exactly, on the window's last frame — the envelope
            // alone drives.
            self.current_params = stolen_from.current_params;
        } else {
            // No automation on the new note: the in-place linear ramp,
            // landing exactly on `target_params`.
            for (step, (target, stolen)) in self
                .param_steps
                .iter_mut()
                .zip(self.target_params.iter().zip(&stolen_from.current_params))
            {
                *step = (target - stolen) / param_ramp_frames as f32;
            }
            self.params = stolen_from.current_params;
            self.current_params = self.params;
        }
    }

    /// The steal ranking key: minimising it lexicographically prefers voices
    /// already releasing (gate expired), then — among releasing voices — the
    /// one furthest into its release tail, then the oldest by trigger time.
    const fn steal_preference(&self) -> (bool, u32, u64) {
        let gated = self.gate_frames_remaining > 0;
        let release_progress = if gated {
            0
        } else {
            self.release_frames_remaining
        };
        (gated, release_progress, self.trigger_seq)
    }
}

#[derive(Debug)]
struct GraphVoiceSlot {
    token: Box<str>,
    release_frames: u32,
    /// [`GraphVoiceSpec::pitched_sample_tail_hz_seconds`] for the slot's
    /// program: the worst-case pitched-sample playback product, in
    /// Hertz-seconds, or 0 when the program has no pitched sample stages.
    pitched_tail_hz_seconds: f32,
    steal: StealPolicy,
    /// The program's `p1`..`p4` steal-ramp window
    /// ([`GraphVoiceSpec::param_ramp_seconds`]) in frames at the bank's
    /// sample rate, floored at one frame.
    param_ramp_frames: u32,
    voice: GraphVoice,
    note: Option<GraphVoiceNote>,
}

impl GraphVoiceSlot {
    /// The release tail for a note at `freq_hz`, in frames.
    ///
    /// A pitched sample plays at `freq / reference`, so notes below the
    /// reference outlast the program's static release (which covers the
    /// buffer at native rate). The tail is floored per note with the actual
    /// playback length, capped at [`MAX_VOICE_RELEASE_TAIL_SECONDS`]; notes
    /// at or above the reference (and programs without pitched samples)
    /// keep the static release unchanged. Pure arithmetic — the trigger
    /// path stays allocation-free.
    fn release_frames_for_note(&self, freq_hz: f32, sample_rate_hz: f32) -> u32 {
        if self.pitched_tail_hz_seconds <= 0.0 || !freq_hz.is_finite() || freq_hz <= 0.0 {
            return self.release_frames;
        }
        let tail_seconds =
            (self.pitched_tail_hz_seconds / freq_hz).min(MAX_VOICE_RELEASE_TAIL_SECONDS);
        self.release_frames
            .max(release_seconds_to_frames(tail_seconds, sample_rate_hz))
    }

    /// Renders one frame of the sounding voice into the per-track mix.
    ///
    /// Finished voices are reset in place and returned to the pool.
    pub fn render_frame(&mut self, track_mix: &mut [(f32, f32)]) {
        let Some(note) = self.note.as_mut() else {
            return;
        };

        // A freshly stolen voice holds its gate low for one frame so the
        // graph's envelopes see a rising edge on the next frame and
        // restart click-free from their current level.
        let in_retrigger_gap = note.retrigger_gap_frames > 0;
        let gate = if !in_retrigger_gap && note.gate_frames_remaining > 0 {
            1.0
        } else {
            0.0
        };

        // Post-steal handover: gain/pan ramp linearly from the stolen
        // note's control values to this note's, landing exactly on the
        // targets at the ramp's end.
        if note.ramp_frames_remaining > 0 {
            note.ramp_frames_remaining -= 1;
            if note.ramp_frames_remaining == 0 {
                note.gain = note.target_gain;
                note.pan = note.target_pan;
            } else {
                note.gain += note.gain_step;
                note.pan += note.pan_step;
            }
        }

        // Per-note parameter automation (ADR 0012): automated parameters
        // follow their breakpoints (linear interpolation, last value
        // held); parameters without breakpoints keep the trigger-time
        // value, bit-identically to the pre-automation path.
        let mut params = note.params;
        if note.ramps.any_active() {
            for (index, value) in params.iter_mut().enumerate() {
                *value = note.ramps.value_at_from(
                    index,
                    *value,
                    note.age_frames,
                    &mut note.ramp_cursors[index],
                );
            }
        }
        note.age_frames = note.age_frames.saturating_add(1);

        // The per-note pattern parameters ramp the same way over their
        // own (pragma-configurable) window, landing exactly; the ramp
        // keeps running through the release tail, so even a one-frame
        // gate reaches the new note's values.
        if note.param_ramp_frames_remaining > 0 {
            if note.ramps.any_active() {
                // Steal x automation composition (ADR 0012): glide from
                // the stolen note's values ONTO the (possibly moving)
                // envelope. Each frame closes 1/remaining of the gap to
                // the envelope's current value: against a flat envelope
                // this traces the exact linear path of the plain steal
                // ramp, against a moving one it converges smoothly, and
                // on the window's last frame (remaining == 1) it lands
                // exactly on the envelope — which alone drives from then
                // on.
                #[allow(clippy::cast_precision_loss)]
                let remaining = note.param_ramp_frames_remaining as f32;
                for (value, current) in params.iter_mut().zip(&note.current_params) {
                    *value = current + (*value - current) / remaining;
                }
                note.param_ramp_frames_remaining -= 1;
            } else {
                note.param_ramp_frames_remaining -= 1;
                if note.param_ramp_frames_remaining == 0 {
                    note.params = note.target_params;
                } else {
                    for (param, step) in note.params.iter_mut().zip(&note.param_steps) {
                        *param += step;
                    }
                }
                params = note.params;
            }
        }
        note.current_params = params;

        let (left, right) =
            self.voice
                .process_frame_with_params(gate, note.freq_hz, note.gain, note.pan, &params);

        if let Ok(track_index) = usize::try_from(note.track_id.get())
            && let Some((mix_left, mix_right)) = track_mix.get_mut(track_index)
        {
            *mix_left += left;
            *mix_right += right;
        }

        if in_retrigger_gap {
            note.retrigger_gap_frames -= 1;
        } else if note.gate_frames_remaining > 0 {
            note.gate_frames_remaining -= 1;
        } else if note.release_frames_remaining > 0 {
            note.release_frames_remaining -= 1;
        } else {
            self.voice.reset();
            self.note = None;
        }
    }
}

/// A fixed pool of prepared graph voices owned by the engine core.
///
/// Built off the audio thread — at engine construction or on the language
/// thread before an [`crate::EngineCommand::ReplaceGraphVoicePrograms`] swap;
/// triggering and rendering never allocate.
#[derive(Debug)]
pub struct GraphVoiceBank {
    sample_rate_hz: f32,
    user_specs: Vec<GraphVoiceSpec>,
    slots: Vec<GraphVoiceSlot>,
    /// Monotonic trigger counter stamping each note's age for the steal
    /// policy. A plain field write per trigger; never wraps in practice.
    next_trigger_seq: u64,
    /// [`VOICE_STEAL_RAMP_SECONDS`] in frames at the bank's sample rate.
    steal_ramp_frames: u32,
}

/// Two banks are interchangeable when they were built for the same sample
/// rate from the same user programs (built-ins are constant).
impl PartialEq for GraphVoiceBank {
    fn eq(&self, other: &Self) -> bool {
        self.sample_rate_hz.to_bits() == other.sample_rate_hz.to_bits()
            && self.user_specs == other.user_specs
    }
}

/// Cloning rebuilds and re-prepares the pool from the retained specs. It
/// allocates: never clone a bank on the audio thread.
impl Clone for GraphVoiceBank {
    fn clone(&self) -> Self {
        Self::with_user_programs(self.sample_rate_hz, self.user_specs.clone())
    }
}

impl GraphVoiceBank {
    /// Builds and prepares the pool for every built-in program.
    #[must_use]
    pub fn with_builtin_programs(sample_rate_hz: f32) -> Self {
        Self::with_user_programs(sample_rate_hz, Vec::new())
    }

    /// Builds and prepares pools for the built-in programs plus every user
    /// spec. A user spec whose token collides with a built-in shadows it.
    ///
    /// Construction compiles and warms every pooled voice, so it allocates;
    /// call it off the audio thread and hand the finished bank to the engine
    /// (via [`crate::EngineCommand::ReplaceGraphVoicePrograms`]).
    #[must_use]
    pub fn with_user_programs(sample_rate_hz: f32, user_specs: Vec<GraphVoiceSpec>) -> Self {
        let mut slots = Vec::new();
        for program in builtin_graph_voice_programs() {
            if user_specs
                .iter()
                .any(|spec| spec.token() == program.token())
            {
                continue;
            }
            let release_frames = program.release_frames(sample_rate_hz);
            for _ in 0..DEFAULT_GRAPH_VOICE_POLYPHONY {
                let mut voice = program.build_voice(sample_rate_hz);
                voice.prepare();
                slots.push(GraphVoiceSlot {
                    token: program.token().into(),
                    release_frames,
                    pitched_tail_hz_seconds: 0.0,
                    steal: StealPolicy::default(),
                    param_ramp_frames: release_seconds_to_frames(
                        DEFAULT_PARAM_RAMP_SECONDS,
                        sample_rate_hz,
                    ),
                    voice,
                    note: None,
                });
            }
        }
        for spec in &user_specs {
            let release_frames = spec.release_frames(sample_rate_hz);
            let pitched_tail_hz_seconds = spec.pitched_sample_tail_hz_seconds();
            let param_ramp_frames =
                release_seconds_to_frames(spec.param_ramp_seconds(), sample_rate_hz);
            for _ in 0..spec.polyphony() {
                let mut voice = spec.build_voice(sample_rate_hz);
                voice.prepare();
                slots.push(GraphVoiceSlot {
                    token: spec.token().into(),
                    release_frames,
                    pitched_tail_hz_seconds,
                    steal: spec.steal_policy(),
                    param_ramp_frames,
                    voice,
                    note: None,
                });
            }
        }
        Self {
            sample_rate_hz,
            user_specs,
            slots,
            next_trigger_seq: 0,
            steal_ramp_frames: release_seconds_to_frames(VOICE_STEAL_RAMP_SECONDS, sample_rate_hz),
        }
    }

    /// The user-defined program specs this bank was built from.
    #[must_use]
    pub fn user_specs(&self) -> &[GraphVoiceSpec] {
        &self.user_specs
    }

    /// Whether `token` names a pooled graph voice program.
    #[must_use]
    pub fn has_program(&self, token: &str) -> bool {
        self.slots.iter().any(|slot| &*slot.token == token)
    }

    /// Starts a note on a pooled voice for `token`.
    ///
    /// An idle voice is claimed when one exists. When the program's pool is
    /// exhausted, the note steals a sounding voice per the program's
    /// [`StealPolicy`] — preferring the voice furthest into its release tail,
    /// else the oldest by trigger time. The steal is click-free: the voice's
    /// graph state is kept (its envelopes retrigger from the current level
    /// after a one-frame gate gap), the note's gain/pan ramp linearly from
    /// the stolen note's values over [`VOICE_STEAL_RAMP_SECONDS`], and its
    /// `p1`..`p4` parameters ramp from the stolen note's values over the
    /// program's param-ramp window.
    ///
    /// Returns `false` (dropping the trigger) when the token names no
    /// program, or the pool is exhausted and stealing is
    /// [`StealPolicy::Off`]. Never allocates.
    ///
    /// The note's per-note pattern parameters (`p1`..`p4`) read their
    /// default; use [`Self::trigger_with_params`] to set them.
    pub fn trigger(
        &mut self,
        token: &str,
        track_id: TrackId,
        gate_frames: u32,
        freq_hz: f32,
        gain: f32,
        pan: f32,
    ) -> bool {
        self.trigger_with_params(
            token,
            track_id,
            gate_frames,
            freq_hz,
            gain,
            pan,
            [DEFAULT_VOICE_PARAM_VALUE; VOICE_PARAM_COUNT],
        )
    }

    /// [`Self::trigger`] with explicit per-note pattern parameters
    /// (`p1`..`p4`, ADR 0010 addendum).
    ///
    /// The values are stamped on the note as plain fields and held for its
    /// whole lifetime — constant signal inputs per note. A steal ramps them
    /// linearly from the stolen note's current values over the program's
    /// param-ramp window ([`GraphVoiceSpec::param_ramp_seconds`], default
    /// [`DEFAULT_PARAM_RAMP_SECONDS`]) — the gain/pan handover treatment —
    /// landing exactly on the new note's values; frequency still switches
    /// immediately. Fresh (idle-voice) triggers never ramp. Never allocates.
    #[allow(clippy::too_many_arguments)]
    pub fn trigger_with_params(
        &mut self,
        token: &str,
        track_id: TrackId,
        gate_frames: u32,
        freq_hz: f32,
        gain: f32,
        pan: f32,
        params: [f32; VOICE_PARAM_COUNT],
    ) -> bool {
        self.trigger_with_automation(
            token,
            track_id,
            gate_frames,
            freq_hz,
            gain,
            pan,
            params,
            &VoiceParamRamps::none(),
        )
    }

    /// [`Self::trigger_with_params`] with per-note parameter breakpoint
    /// automation (ADR 0012).
    ///
    /// `ramps` is copied into the note's fixed-size storage; while the note
    /// sounds, each automated parameter follows its breakpoints (linear
    /// interpolation between them, the last value held through the release
    /// tail) instead of holding the trigger-time value. A steal stamps the
    /// NEW note's automation and restarts it from the note's beginning; over
    /// the program's param-ramp window the parameters glide from the stolen
    /// note's current values onto that envelope, landing exactly, after
    /// which the envelope alone drives (steal x automation composition).
    /// Never allocates.
    #[allow(clippy::too_many_arguments)]
    pub fn trigger_with_automation(
        &mut self,
        token: &str,
        track_id: TrackId,
        gate_frames: u32,
        freq_hz: f32,
        gain: f32,
        pan: f32,
        params: [f32; VOICE_PARAM_COUNT],
        ramps: &VoiceParamRamps,
    ) -> bool {
        let mut idle: Option<usize> = None;
        let mut victim: Option<(usize, (bool, u32, u64))> = None;
        for (index, slot) in self.slots.iter().enumerate() {
            if &*slot.token != token {
                continue;
            }
            match &slot.note {
                None => {
                    idle = Some(index);
                    break;
                }
                Some(note) => {
                    if slot.steal == StealPolicy::Off {
                        continue;
                    }
                    let preference = note.steal_preference();
                    if victim.is_none_or(|(_, best)| preference < best) {
                        victim = Some((index, preference));
                    }
                }
            }
        }

        let sample_rate_hz = self.sample_rate_hz;

        if let Some(index) = idle {
            let trigger_seq = self.next_trigger_seq;
            self.next_trigger_seq += 1;
            let slot = &mut self.slots[index];
            slot.note = Some(GraphVoiceNote::fresh(
                track_id,
                gate_frames,
                slot.release_frames_for_note(freq_hz, sample_rate_hz),
                freq_hz,
                gain,
                pan,
                params,
                ramps,
                trigger_seq,
            ));
            return true;
        }

        if let Some((index, _)) = victim {
            let trigger_seq = self.next_trigger_seq;
            self.next_trigger_seq += 1;
            let slot = &mut self.slots[index];
            let stolen_from = slot
                .note
                .unwrap_or_else(|| unreachable!("steal victims are sounding notes"));
            // Deliberately no `slot.voice.reset()`: keeping the graph state
            // is what makes the handover click-free (ADR 0009 addendum).
            let mut note = GraphVoiceNote::fresh(
                track_id,
                gate_frames,
                slot.release_frames_for_note(freq_hz, sample_rate_hz),
                freq_hz,
                gain,
                pan,
                params,
                ramps,
                trigger_seq,
            );
            note.begin_steal_handover(&stolen_from, self.steal_ramp_frames, slot.param_ramp_frames);
            slot.note = Some(note);
            return true;
        }

        false
    }

    /// Renders one frame of every sounding voice into the per-track mix.
    ///
    /// Finished voices are reset in place and returned to the pool; nothing
    /// is allocated or dropped on this path.
    pub fn render_frame(&mut self, track_mix: &mut [(f32, f32)]) {
        for slot in &mut self.slots {
            slot.render_frame(track_mix);
        }
    }

    /// Silences every sounding voice and returns it to the pool.
    pub fn stop_all(&mut self) {
        for slot in &mut self.slots {
            if slot.note.take().is_some() {
                slot.voice.reset();
            }
        }
    }
}

/// Derives the graph voice control values from a scheduled trigger.
///
/// Frequency follows the analog-voice convention: the engine reference
/// frequency scaled by the trigger's playback rate. Gain applies the shared
/// output trim; pan is clamped to the stereo field.
#[allow(clippy::cast_possible_truncation)]
pub fn graph_note_params(trigger: &SampleTrigger, base_hz: f32) -> (f32, f32, f32) {
    let rate = if trigger.rate().is_finite() {
        trigger.rate().abs() as f32
    } else {
        1.0
    };
    let freq_hz = (base_hz * rate).max(0.0);

    let gain = if trigger.gain().is_finite() {
        (trigger.gain() as f32).max(0.0)
    } else {
        1.0
    } * GRAPH_OUTPUT_TRIM;

    let pan = if trigger.pan().is_finite() {
        (trigger.pan() as f32).clamp(-1.0, 1.0)
    } else {
        0.0
    };

    (freq_hz, gain, pan)
}

/// Derives the per-note pattern parameters (`p1`..`p4`) from a scheduled
/// trigger (ADR 0010 addendum).
///
/// Values are narrowed to `f32` for the audio thread; non-finite values fall
/// back to [`DEFAULT_VOICE_PARAM_VALUE`].
#[must_use]
#[allow(clippy::cast_possible_truncation)]
pub fn graph_note_voice_params(trigger: &SampleTrigger) -> [f32; VOICE_PARAM_COUNT] {
    trigger.voice_params().map(|value| {
        if value.is_finite() {
            value as f32
        } else {
            DEFAULT_VOICE_PARAM_VALUE
        }
    })
}

/// Derives the per-note parameter breakpoint automation from a scheduled
/// trigger (ADR 0012).
///
/// Each shipped [`VoiceParamBreakpoint`]'s normalized position is mapped
/// onto a frame offset within the note's gate (`gate_frames`, the event's
/// full extent in frames). Sanitizing is allocation-free and documented on
/// [`VoiceParamRamps::set_breakpoints`]: non-finite positions or values are
/// skipped, positions clamp into \[0, 1\], repeated frames keep the last
/// value, a late-starting ramp holds the trigger-time value (`params`) until
/// its first breakpoint, and everything past
/// [`MAX_VOICE_PARAM_BREAKPOINTS`] is dropped.
#[must_use]
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
pub fn graph_note_voice_ramps(
    trigger: &SampleTrigger,
    gate_frames: u32,
    params: &[f32; VOICE_PARAM_COUNT],
) -> VoiceParamRamps {
    let mut ramps = VoiceParamRamps::none();
    for (index, &start_value) in params.iter().enumerate() {
        let Some(breakpoints) = trigger.voice_param_ramp(index) else {
            continue;
        };
        for breakpoint in breakpoints {
            if !breakpoint.position.is_finite() {
                continue;
            }
            let position = breakpoint.position.clamp(0.0, 1.0);
            let frame = (position * f64::from(gate_frames)).round() as u32;
            let value = if breakpoint.value.is_finite() {
                breakpoint.value as f32
            } else {
                // Mirror `graph_note_voice_params`: reject the value, not
                // the note — push_breakpoint drops non-finite values.
                f32::NAN
            };
            ramps.push_breakpoint(index, frame, value, start_value);
        }
    }
    ramps
}

#[cfg(test)]
mod tests {
    use super::*;

    const SR: f32 = 48_000.0;

    fn track(id: u32) -> TrackId {
        TrackId::new(id)
    }

    #[test]
    fn builtin_programs_expose_gsine() {
        let programs = builtin_graph_voice_programs();
        assert!(programs.iter().any(|program| program.token() == "gsine"));
    }

    #[test]
    fn gsine_voice_produces_audio_while_gated_and_decays_after_release() {
        let program = &builtin_graph_voice_programs()[0];
        let mut voice = program.build_voice(SR);
        voice.prepare();

        let mut gated_energy = 0.0_f32;
        for _ in 0..2_400 {
            let (left, right) = voice.process_frame(1.0, 220.0, 0.5, 0.0);
            gated_energy += left.abs() + right.abs();
        }
        assert!(gated_energy > 1.0, "gated voice should be audible");

        // Run the release out (0.02 s = 960 frames), then check silence.
        for _ in 0..2_400 {
            let _ = voice.process_frame(0.0, 220.0, 0.5, 0.0);
        }
        let (left, right) = voice.process_frame(0.0, 220.0, 0.5, 0.0);
        assert!(left.abs() < 1e-4 && right.abs() < 1e-4);
    }

    #[test]
    fn bank_trigger_respects_polyphony_and_unknown_tokens() {
        let mut bank = GraphVoiceBank::with_builtin_programs(SR);
        assert!(bank.has_program("gsine"));
        assert!(!bank.has_program("bd"));
        assert!(!bank.trigger("bd", track(0), 10, 220.0, 0.5, 0.0));

        for _ in 0..DEFAULT_GRAPH_VOICE_POLYPHONY {
            assert!(bank.trigger("gsine", track(0), 10, 220.0, 0.5, 0.0));
        }
        assert!(
            bank.trigger("gsine", track(0), 10, 220.0, 0.5, 0.0),
            "the pool is exhausted, the trigger must steal a sounding voice"
        );
        let sounding = bank.slots.iter().filter(|slot| slot.note.is_some()).count();
        assert_eq!(
            sounding, DEFAULT_GRAPH_VOICE_POLYPHONY,
            "stealing must reuse a pooled voice, not grow the pool"
        );
    }

    /// A minimal sustained voice (sine x ADSR) for steal-policy tests.
    fn steal_test_spec(polyphony: usize, steal: StealPolicy) -> GraphVoiceSpec {
        GraphVoiceSpec::new(
            "lead",
            0.02,
            vec![
                VoiceNodeSpec::Sine {
                    freq: VoiceSignalRef::Freq,
                },
                VoiceNodeSpec::Adsr {
                    gate: VoiceSignalRef::Gate,
                    attack_s: 0.001,
                    decay_s: 0.005,
                    sustain: 0.8,
                    release_s: 0.02,
                },
                VoiceNodeSpec::Mul {
                    left: VoiceSignalRef::Node(0),
                    right: VoiceSignalRef::Node(1),
                },
            ],
            VoiceSignalRef::Node(2),
        )
        .expect("steal test spec should validate")
        .with_polyphony(polyphony)
        .expect("test polyphony is within bounds")
        .with_steal_policy(steal)
    }

    fn sounding_freqs(bank: &GraphVoiceBank) -> Vec<f32> {
        bank.slots
            .iter()
            .filter_map(|slot| slot.note.as_ref().map(|note| note.freq_hz))
            .collect()
    }

    #[test]
    fn exhausted_pool_steals_the_oldest_gated_voice() {
        let mut bank =
            GraphVoiceBank::with_user_programs(SR, vec![steal_test_spec(2, StealPolicy::Oldest)]);
        assert!(bank.trigger("lead", track(0), 10_000, 220.0, 0.5, 0.0));
        assert!(bank.trigger("lead", track(0), 10_000, 330.0, 0.5, 0.0));
        assert!(bank.trigger("lead", track(0), 10_000, 440.0, 0.5, 0.0));

        let freqs = sounding_freqs(&bank);
        assert!(
            !freqs.contains(&220.0) && freqs.contains(&330.0) && freqs.contains(&440.0),
            "the oldest note (220 Hz) must be stolen, got {freqs:?}"
        );
    }

    #[test]
    fn steal_prefers_the_voice_furthest_into_release() {
        let mut bank =
            GraphVoiceBank::with_user_programs(SR, vec![steal_test_spec(3, StealPolicy::Oldest)]);
        // 220 Hz releases first (gate 2), 330 Hz later (gate 5), 550 Hz stays
        // gated; after 8 frames the 220 Hz note is furthest into release.
        assert!(bank.trigger("lead", track(0), 2, 220.0, 0.5, 0.0));
        assert!(bank.trigger("lead", track(0), 5, 330.0, 0.5, 0.0));
        assert!(bank.trigger("lead", track(0), 10_000, 550.0, 0.5, 0.0));
        let mut mix = vec![(0.0_f32, 0.0_f32); 1];
        for _ in 0..8 {
            bank.render_frame(&mut mix);
        }

        assert!(bank.trigger("lead", track(0), 10_000, 440.0, 0.5, 0.0));
        let freqs = sounding_freqs(&bank);
        assert!(
            !freqs.contains(&220.0) && freqs.contains(&330.0) && freqs.contains(&550.0),
            "the most-released note (220 Hz) must be stolen, got {freqs:?}"
        );
    }

    #[test]
    fn steal_prefers_releasing_voices_over_older_gated_ones() {
        let mut bank =
            GraphVoiceBank::with_user_programs(SR, vec![steal_test_spec(2, StealPolicy::Oldest)]);
        // 220 Hz is oldest but still gated; 330 Hz is newer but releasing.
        assert!(bank.trigger("lead", track(0), 10_000, 220.0, 0.5, 0.0));
        assert!(bank.trigger("lead", track(0), 2, 330.0, 0.5, 0.0));
        let mut mix = vec![(0.0_f32, 0.0_f32); 1];
        for _ in 0..5 {
            bank.render_frame(&mut mix);
        }

        assert!(bank.trigger("lead", track(0), 10_000, 440.0, 0.5, 0.0));
        let freqs = sounding_freqs(&bank);
        assert!(
            freqs.contains(&220.0) && !freqs.contains(&330.0),
            "the releasing note (330 Hz) must be stolen before the older gated one, got {freqs:?}"
        );
    }

    #[test]
    fn no_steal_occurs_while_the_pool_has_idle_voices() {
        let mut bank =
            GraphVoiceBank::with_user_programs(SR, vec![steal_test_spec(3, StealPolicy::Oldest)]);
        assert!(bank.trigger("lead", track(0), 10_000, 220.0, 0.5, 0.0));
        assert!(bank.trigger("lead", track(0), 10_000, 330.0, 0.5, 0.0));
        assert!(bank.trigger("lead", track(0), 10_000, 440.0, 0.5, 0.0));

        let freqs = sounding_freqs(&bank);
        assert!(
            freqs.contains(&220.0) && freqs.contains(&330.0) && freqs.contains(&440.0),
            "a poly-3 pool must fit three notes without stealing, got {freqs:?}"
        );
    }

    #[test]
    fn steal_policy_off_drops_triggers_when_the_pool_is_exhausted() {
        let mut bank =
            GraphVoiceBank::with_user_programs(SR, vec![steal_test_spec(2, StealPolicy::Off)]);
        assert!(bank.trigger("lead", track(0), 10_000, 220.0, 0.5, 0.0));
        assert!(bank.trigger("lead", track(0), 10_000, 330.0, 0.5, 0.0));
        assert!(
            !bank.trigger("lead", track(0), 10_000, 440.0, 0.5, 0.0),
            "steal = off must keep the original drop behavior"
        );
        let freqs = sounding_freqs(&bank);
        assert!(freqs.contains(&220.0) && freqs.contains(&330.0));
    }

    #[test]
    fn stolen_notes_ramp_gain_and_pan_from_the_stolen_values() {
        let mut bank =
            GraphVoiceBank::with_user_programs(SR, vec![steal_test_spec(1, StealPolicy::Oldest)]);
        assert!(bank.trigger("lead", track(0), 10_000, 220.0, 0.2, -1.0));
        assert!(bank.trigger("lead", track(0), 10_000, 440.0, 0.8, 1.0));

        let lead_slot = |bank: &GraphVoiceBank| {
            bank.slots
                .iter()
                .position(|slot| &*slot.token == "lead")
                .expect("the lead program has a pooled slot")
        };
        let slot_index = lead_slot(&bank);
        let note = bank.slots[slot_index]
            .note
            .expect("the stolen note is sounding");
        assert_eq!(note.retrigger_gap_frames, 1, "one-frame gate gap");
        assert_eq!(note.ramp_frames_remaining, bank.steal_ramp_frames);
        assert!((note.gain - 0.2).abs() < 1e-6, "gain starts at old value");
        assert!((note.pan - -1.0).abs() < 1e-6, "pan starts at old value");

        let mut mix = vec![(0.0_f32, 0.0_f32); 1];
        for _ in 0..bank.steal_ramp_frames {
            bank.render_frame(&mut mix);
        }
        let note = bank.slots[slot_index]
            .note
            .expect("the note is still sounding");
        assert_eq!(note.ramp_frames_remaining, 0);
        assert!((note.gain - 0.8).abs() < 1e-6, "gain lands on the target");
        assert!((note.pan - 1.0).abs() < 1e-6, "pan lands on the target");
    }

    #[test]
    fn bank_returns_finished_voices_to_the_pool() {
        let mut bank = GraphVoiceBank::with_builtin_programs(SR);
        assert!(bank.trigger("gsine", track(0), 2, 220.0, 0.5, 0.0));

        let mut mix = vec![(0.0_f32, 0.0_f32); 1];
        // 2 gate frames + release frames + the final reclaim frame.
        let program = &builtin_graph_voice_programs()[0];
        let lifetime = 2 + program.release_frames(SR) + 1;
        for _ in 0..lifetime {
            bank.render_frame(&mut mix);
        }

        for _ in 0..DEFAULT_GRAPH_VOICE_POLYPHONY {
            assert!(bank.trigger("gsine", track(0), 2, 220.0, 0.5, 0.0));
        }
    }

    #[test]
    fn render_frame_ignores_out_of_range_tracks() {
        let mut bank = GraphVoiceBank::with_builtin_programs(SR);
        assert!(bank.trigger("gsine", track(7), 4, 220.0, 0.5, 0.0));
        let mut mix = vec![(0.0_f32, 0.0_f32); 1];
        bank.render_frame(&mut mix);
        assert_eq!(mix[0], (0.0, 0.0));
    }

    /// A voice whose audio output IS the gated `p1` parameter, so the
    /// per-note pattern-parameter plumbing is directly observable.
    fn param_meter_spec(polyphony: usize) -> GraphVoiceSpec {
        GraphVoiceSpec::new(
            "meter",
            0.001,
            vec![VoiceNodeSpec::Mul {
                left: VoiceSignalRef::Param(0),
                right: VoiceSignalRef::Gate,
            }],
            VoiceSignalRef::Node(0),
        )
        .expect("param meter spec should validate")
        .with_polyphony(polyphony)
        .expect("test polyphony is within bounds")
    }

    /// The pooled slot index of the (poly-1) `meter` program.
    fn meter_slot(bank: &GraphVoiceBank) -> usize {
        bank.slots
            .iter()
            .position(|slot| &*slot.token == "meter")
            .expect("the meter program has a pooled slot")
    }

    #[test]
    #[allow(clippy::float_cmp, clippy::suboptimal_flops)] // exact test constants
    fn voice_params_reach_the_voice_as_ambient_signals() {
        let spec = param_meter_spec(1);
        let mut voice = spec.build_voice(SR);
        voice.prepare();

        let center = std::f32::consts::FRAC_1_SQRT_2;
        let (left, right) =
            voice.process_frame_with_params(1.0, 220.0, 1.0, 0.0, &[0.25, 0.0, 0.0, 0.0]);
        assert!(
            (left - 0.25 * center).abs() < 1e-6 && (right - 0.25 * center).abs() < 1e-6,
            "p1 must reach the voice body as a signal: ({left}, {right})"
        );

        // Each of the four parameters occupies its own input channel.
        let spec_p4 = GraphVoiceSpec::new(
            "meter4",
            0.001,
            vec![VoiceNodeSpec::Mul {
                left: VoiceSignalRef::Param(3),
                right: VoiceSignalRef::Gate,
            }],
            VoiceSignalRef::Node(0),
        )
        .expect("p4 meter spec should validate");
        let mut voice = spec_p4.build_voice(SR);
        voice.prepare();
        let (left, _) =
            voice.process_frame_with_params(1.0, 220.0, 1.0, 0.0, &[0.9, 0.8, 0.7, 0.6]);
        assert!(
            (left - 0.6 * center).abs() < 1e-6,
            "p4 must map onto the fourth parameter channel: {left}"
        );
    }

    #[test]
    #[allow(clippy::float_cmp)] // exact test constants
    fn unset_voice_params_read_the_documented_zero_default() {
        let spec = param_meter_spec(1);
        let mut voice = spec.build_voice(SR);
        voice.prepare();
        let (left, right) = voice.process_frame(1.0, 220.0, 1.0, 0.0);
        assert_eq!(
            (left, right),
            (0.0, 0.0),
            "a param the pattern never sets must read {DEFAULT_VOICE_PARAM_VALUE}"
        );

        let mut bank = GraphVoiceBank::with_user_programs(SR, vec![param_meter_spec(1)]);
        assert!(bank.trigger("meter", track(0), 10, 220.0, 1.0, 0.0));
        let slot = meter_slot(&bank);
        let note = bank.slots[slot].note.expect("the note is sounding");
        assert_eq!(note.params, [DEFAULT_VOICE_PARAM_VALUE; VOICE_PARAM_COUNT]);
    }

    #[test]
    #[allow(clippy::float_cmp, clippy::suboptimal_flops)] // exact test constants
    fn bank_trigger_with_params_stamps_the_note() {
        let mut bank = GraphVoiceBank::with_user_programs(SR, vec![param_meter_spec(1)]);
        assert!(bank.trigger_with_params(
            "meter",
            track(0),
            10,
            220.0,
            1.0,
            0.0,
            [0.25, 0.5, 0.75, 1.0]
        ));
        let slot = meter_slot(&bank);
        let note = bank.slots[slot].note.expect("the note is sounding");
        assert_eq!(note.params, [0.25, 0.5, 0.75, 1.0]);

        let mut mix = vec![(0.0_f32, 0.0_f32); 1];
        bank.render_frame(&mut mix);
        let center = std::f32::consts::FRAC_1_SQRT_2;
        assert!(
            (mix[0].0 - 0.25 * center).abs() < 1e-6,
            "the rendered frame must carry the note's p1: {}",
            mix[0].0
        );
    }

    #[test]
    #[allow(clippy::float_cmp)] // exact test constants
    fn steal_ramps_params_from_the_stolen_values_and_lands_exactly() {
        let mut bank = GraphVoiceBank::with_user_programs(SR, vec![param_meter_spec(1)]);
        assert!(bank.trigger_with_params(
            "meter",
            track(0),
            10_000,
            220.0,
            0.5,
            0.0,
            [0.3, 0.0, 0.0, 0.0]
        ));
        assert!(
            bank.trigger_with_params(
                "meter",
                track(0),
                10_000,
                440.0,
                0.5,
                0.0,
                [0.9, 0.0, 0.0, 0.0]
            ),
            "the poly-1 pool must steal for the second note"
        );
        let slot = meter_slot(&bank);
        let note = bank.slots[slot].note.expect("the stolen note is sounding");
        assert!(
            (note.params[0] - 0.3).abs() < 1e-6,
            "a steal must start `p1` at the stolen note's value, got {}",
            note.params[0]
        );
        // Frequency keeps its immediate-switch semantics: only the p1..p4
        // parameters (like gain/pan) ramp through the handover.
        assert!((note.freq_hz - 440.0).abs() < 1e-6);

        // The ramp moves linearly and lands exactly on the new note's value.
        let ramp_frames = bank.steal_ramp_frames;
        let mut mix = vec![(0.0_f32, 0.0_f32); 1];
        for _ in 0..ramp_frames / 2 {
            bank.render_frame(&mut mix);
        }
        let note = bank.slots[slot].note.expect("the note is still sounding");
        #[allow(clippy::cast_precision_loss)]
        let expected_midpoint = 0.3 + (0.9 - 0.3) * (ramp_frames / 2) as f32 / ramp_frames as f32;
        assert!(
            (note.params[0] - expected_midpoint).abs() < 1e-3,
            "halfway through the window `p1` must sit at the linear midpoint: \
             got {}, expected {expected_midpoint}",
            note.params[0]
        );
        for _ in 0..ramp_frames.div_ceil(2) {
            bank.render_frame(&mut mix);
        }
        let note = bank.slots[slot].note.expect("the note is still sounding");
        assert_eq!(
            note.params,
            [0.9, 0.0, 0.0, 0.0],
            "after the ramp window the params must land exactly on the new \
             note's values (the #1418 sample-and-hold semantics resume)"
        );
    }

    #[test]
    #[allow(clippy::float_cmp, clippy::suboptimal_flops)] // exact test constants
    fn fresh_triggers_start_params_exactly_at_the_new_value() {
        // A reused pooled voice must NOT ramp from the previous note's stale
        // params: only the stolen/retriggered path ramps (like gain/pan).
        let mut bank = GraphVoiceBank::with_user_programs(SR, vec![param_meter_spec(1)]);
        assert!(bank.trigger_with_params(
            "meter",
            track(0),
            2,
            220.0,
            1.0,
            0.0,
            [0.7, 0.0, 0.0, 0.0]
        ));
        // Run the first note to completion so its slot returns to the pool
        // (2 gate frames + the 0.001 s release + the reclaim frame).
        let mut mix = vec![(0.0_f32, 0.0_f32); 1];
        let lifetime = 2 + release_seconds_to_frames(0.001, SR) + 1;
        for _ in 0..lifetime {
            mix[0] = (0.0, 0.0);
            bank.render_frame(&mut mix);
        }
        let slot = meter_slot(&bank);
        assert!(bank.slots[slot].note.is_none(), "the pool must be idle");

        assert!(bank.trigger_with_params(
            "meter",
            track(0),
            10,
            220.0,
            1.0,
            0.0,
            [0.2, 0.0, 0.0, 0.0]
        ));
        let note = bank.slots[slot].note.expect("the fresh note is sounding");
        assert_eq!(
            note.params,
            [0.2, 0.0, 0.0, 0.0],
            "a fresh (idle-voice) trigger must start exactly at the new value"
        );
        // And the very first rendered frame already carries the new value.
        mix[0] = (0.0, 0.0);
        bank.render_frame(&mut mix);
        let center = std::f32::consts::FRAC_1_SQRT_2;
        assert!(
            (mix[0].0 - 0.2 * center).abs() < 1e-6,
            "the first frame must read the fresh note's exact p1: {}",
            mix[0].0
        );
    }

    /// A `meter`-shaped voice with a release tail long enough to outlive the
    /// default 2 ms param ramp, for short-note ramp-completion tests.
    fn long_release_meter_spec() -> GraphVoiceSpec {
        GraphVoiceSpec::new(
            "meter",
            0.01,
            vec![VoiceNodeSpec::Mul {
                left: VoiceSignalRef::Param(0),
                right: VoiceSignalRef::Gate,
            }],
            VoiceSignalRef::Node(0),
        )
        .expect("long-release meter spec should validate")
        .with_polyphony(1)
        .expect("test polyphony is within bounds")
    }

    #[test]
    #[allow(clippy::float_cmp)] // exact test constants
    fn param_ramp_completes_through_the_release_tail_of_a_short_note() {
        let mut bank = GraphVoiceBank::with_user_programs(SR, vec![long_release_meter_spec()]);
        assert!(bank.trigger_with_params(
            "meter",
            track(0),
            10_000,
            220.0,
            0.5,
            0.0,
            [0.1, 0.0, 0.0, 0.0]
        ));
        // The stealing note's gate is a single frame: the ramp must keep
        // running through its release tail and still land exactly.
        assert!(bank.trigger_with_params(
            "meter",
            track(0),
            1,
            440.0,
            0.5,
            0.0,
            [0.8, 0.0, 0.0, 0.0]
        ));
        let slot = meter_slot(&bank);
        let mut mix = vec![(0.0_f32, 0.0_f32); 1];
        for _ in 0..=bank.steal_ramp_frames {
            bank.render_frame(&mut mix);
        }
        let note = bank.slots[slot]
            .note
            .expect("the short note is still in its release tail");
        assert_eq!(
            note.params,
            [0.8, 0.0, 0.0, 0.0],
            "the ramp must complete (and land exactly) during the release tail"
        );
    }

    #[test]
    fn spec_param_ramp_window_is_configurable_and_bounded() {
        // The default window matches the gain/pan steal-ramp precedent.
        #[allow(clippy::float_cmp)]
        {
            assert_eq!(DEFAULT_PARAM_RAMP_SECONDS, VOICE_STEAL_RAMP_SECONDS);
            assert_eq!(
                param_meter_spec(1).param_ramp_seconds(),
                DEFAULT_PARAM_RAMP_SECONDS
            );
        }

        // A custom window reaches the pooled slots and the steal handover.
        let spec = param_meter_spec(1)
            .with_param_ramp_seconds(0.01)
            .expect("10 ms is a valid param ramp window");
        let mut bank = GraphVoiceBank::with_user_programs(SR, vec![spec]);
        assert!(bank.trigger_with_params(
            "meter",
            track(0),
            10_000,
            220.0,
            0.5,
            0.0,
            [0.1, 0.0, 0.0, 0.0]
        ));
        assert!(bank.trigger_with_params(
            "meter",
            track(0),
            10_000,
            440.0,
            0.5,
            0.0,
            [0.9, 0.0, 0.0, 0.0]
        ));
        let slot = meter_slot(&bank);
        let note = bank.slots[slot].note.expect("the stolen note is sounding");
        assert_eq!(
            note.param_ramp_frames_remaining,
            release_seconds_to_frames(0.01, SR),
            "the pragma window must set the steal's param ramp length"
        );
        assert_ne!(
            note.param_ramp_frames_remaining, bank.steal_ramp_frames,
            "a 10 ms param window must differ from the fixed 2 ms gain/pan ramp"
        );

        // Out-of-range and non-finite windows are rejected at build time.
        for invalid in [-0.1, MAX_PARAM_RAMP_SECONDS + 0.5, f32::NAN, f32::INFINITY] {
            let error = param_meter_spec(1)
                .with_param_ramp_seconds(invalid)
                .expect_err("invalid param ramp windows must be rejected");
            assert_eq!(error, GraphVoiceSpecError::InvalidParamRamp);
        }
    }

    #[test]
    fn spec_rejects_out_of_range_param_references() {
        let out_of_range = channel_index(VOICE_PARAM_COUNT);
        let error = GraphVoiceSpec::new(
            "bad",
            0.01,
            vec![VoiceNodeSpec::Mul {
                left: VoiceSignalRef::Param(out_of_range),
                right: VoiceSignalRef::Gate,
            }],
            VoiceSignalRef::Node(0),
        )
        .expect_err("param indices at or above VOICE_PARAM_COUNT must be rejected");
        assert_eq!(
            error,
            GraphVoiceSpecError::ParamOutOfRange {
                node: 0,
                reference: VOICE_PARAM_COUNT,
            }
        );
    }

    #[test]
    #[allow(clippy::float_cmp)] // exact test constants
    fn graph_note_voice_params_follow_trigger_fields() {
        let trigger = SampleTrigger::named("meter")
            .with_voice_param(0, 880.0)
            .with_voice_param(3, -0.5)
            .with_voice_param(VOICE_PARAM_COUNT, 42.0);
        assert_eq!(graph_note_voice_params(&trigger), [880.0, 0.0, 0.0, -0.5]);

        let non_finite = SampleTrigger::named("meter").with_voice_param(1, f64::NAN);
        assert_eq!(
            graph_note_voice_params(&non_finite),
            [0.0; VOICE_PARAM_COUNT],
            "non-finite parameter values must fall back to the default"
        );
    }

    /// A pitched-sample voice shaped like the language compiler emits: a
    /// constant-amplitude buffer whose static release covers the buffer at
    /// native rate (`frames` source samples at `source_rate_hz`).
    fn pitched_sample_spec(
        frames: usize,
        source_rate_hz: u32,
        reference_hz: f32,
    ) -> GraphVoiceSpec {
        let sample = PlaybackSample::from_mono_frames(vec![0.5_f32; frames], source_rate_hz);
        #[allow(clippy::cast_possible_truncation)]
        let native_seconds = sample.duration_seconds() as f32;
        GraphVoiceSpec::new(
            "keys",
            native_seconds,
            vec![VoiceNodeSpec::Sample {
                gate: VoiceSignalRef::Gate,
                rate: VoiceSignalRef::Freq,
                sample,
                looped: false,
                loop_crossfade: false,
                pitch_reference_hz: Some(reference_hz),
            }],
            VoiceSignalRef::Node(0),
        )
        .expect("pitched sample spec should validate")
    }

    /// The sounding note on the single-program test bank.
    fn keys_note(bank: &GraphVoiceBank) -> GraphVoiceNote {
        bank.slots
            .iter()
            .find_map(|slot| slot.note)
            .expect("a keys note is sounding")
    }

    #[test]
    fn pitched_note_below_the_reference_sounds_to_its_true_end() {
        // An octave below the 220 Hz reference plays at rate 0.5, so an
        // 800-frame buffer takes 1600 output frames — twice the native-rate
        // release. The trigger must stretch this note's release tail to the
        // playback's actual end instead of truncating at the native end.
        const LOW_SR: f32 = 8_000.0;
        let mut bank = GraphVoiceBank::with_user_programs(
            LOW_SR,
            vec![pitched_sample_spec(800, 8_000, 220.0)],
        );
        assert!(bank.trigger("keys", track(0), 4, 110.0, 1.0, 0.0));
        assert_eq!(
            keys_note(&bank).release_frames_remaining,
            release_seconds_to_frames(0.2, LOW_SR),
            "the stamped release must cover the buffer at the note's actual rate"
        );

        let mut mix = vec![(0.0_f32, 0.0_f32); 1];
        let mut rendered = Vec::with_capacity(2_400);
        for _ in 0..2_400 {
            mix[0] = (0.0, 0.0);
            bank.render_frame(&mut mix);
            rendered.push(mix[0].0);
        }
        let energy =
            |range: std::ops::Range<usize>| rendered[range].iter().map(|s| s.abs()).sum::<f32>();
        assert!(
            energy(1_000..1_550) > 1.0,
            "the note must keep sounding past the native-rate end (frame 800)"
        );
        assert!(
            energy(1_700..2_400) == 0.0,
            "the note must be silent once the slowed playback truly ends"
        );
    }

    #[test]
    fn pitched_release_extension_caps_at_the_thirty_second_tail_bound() {
        // 0.1 s of buffer referenced at 220 Hz, triggered at 0.5 Hz, would
        // play for 44 s; the per-note extension must clamp to the same 30 s
        // bound as the `release` pragma so note lifetimes stay bounded.
        const LOW_SR: f32 = 8_000.0;
        let mut bank = GraphVoiceBank::with_user_programs(
            LOW_SR,
            vec![pitched_sample_spec(800, 8_000, 220.0)],
        );
        assert!(bank.trigger("keys", track(0), 4, 0.5, 1.0, 0.0));
        assert_eq!(
            keys_note(&bank).release_frames_remaining,
            release_seconds_to_frames(30.0, LOW_SR),
            "far-below-reference notes must cap at the 30 s release bound"
        );
    }

    #[test]
    fn pitched_notes_at_or_above_the_reference_keep_the_static_release() {
        // Regression: the static release already covers native-rate playback
        // (the language extends it to the buffer's duration), so notes at or
        // above the reference — and unpitched sample voices at any note —
        // must stamp exactly the program release, bit for bit.
        const LOW_SR: f32 = 8_000.0;
        let static_frames = release_seconds_to_frames(0.1, LOW_SR);
        for freq_hz in [220.0, 440.0, 880.0] {
            let mut bank = GraphVoiceBank::with_user_programs(
                LOW_SR,
                vec![pitched_sample_spec(800, 8_000, 220.0)],
            );
            assert!(bank.trigger("keys", track(0), 4, freq_hz, 1.0, 0.0));
            assert_eq!(
                keys_note(&bank).release_frames_remaining,
                static_frames,
                "a note at {freq_hz} Hz must keep the static release"
            );
        }

        // An unpitched sample voice (plain rate semantics) never extends,
        // whatever the note frequency says.
        let sample = PlaybackSample::from_mono_frames(vec![0.5_f32; 800], 8_000);
        let unpitched = GraphVoiceSpec::new(
            "keys",
            0.1,
            vec![
                VoiceNodeSpec::Constant { value: 1.0 },
                VoiceNodeSpec::Sample {
                    gate: VoiceSignalRef::Gate,
                    rate: VoiceSignalRef::Node(0),
                    sample,
                    looped: false,
                    loop_crossfade: false,
                    pitch_reference_hz: None,
                },
            ],
            VoiceSignalRef::Node(1),
        )
        .expect("unpitched sample spec should validate");
        let mut bank = GraphVoiceBank::with_user_programs(LOW_SR, vec![unpitched]);
        assert!(bank.trigger("keys", track(0), 4, 1.0, 1.0, 0.0));
        assert_eq!(keys_note(&bank).release_frames_remaining, static_frames);
    }

    #[test]
    fn graph_note_params_follow_trigger_fields() {
        let trigger = SampleTrigger::named("gsine")
            .with_rate(2.0)
            .with_gain(1.0)
            .with_pan(-0.5);
        let (freq_hz, gain, pan) = graph_note_params(&trigger, 220.0);
        assert!((freq_hz - 440.0).abs() < 1e-3);
        assert!((gain - GRAPH_OUTPUT_TRIM).abs() < 1e-6);
        assert!((pan - -0.5).abs() < 1e-6);
    }

    // -----------------------------------------------------------------------
    // Per-note parameter breakpoint automation (ADR 0012): a control pattern
    // with sub-note structure ships (position, value) breakpoints with the
    // trigger; playback interpolates linearly between them at every frame.
    // -----------------------------------------------------------------------

    /// The center-channel equal-power pan factor applied by the meter voice.
    const CENTER: f32 = std::f32::consts::FRAC_1_SQRT_2;

    /// Renders `frames` frames of the poly-1 `meter` program (whose output is
    /// the gated `p1` signal) and returns the left-channel values.
    fn render_meter_frames(bank: &mut GraphVoiceBank, frames: usize) -> Vec<f32> {
        let mut mix = vec![(0.0_f32, 0.0_f32); 1];
        (0..frames)
            .map(|_| {
                mix[0] = (0.0, 0.0);
                bank.render_frame(&mut mix);
                mix[0].0
            })
            .collect()
    }

    #[test]
    fn ramped_param_interpolates_linearly_between_breakpoints() {
        let mut bank = GraphVoiceBank::with_user_programs(SR, vec![param_meter_spec(1)]);
        let mut ramps = VoiceParamRamps::none();
        // p1: 0.2 at the note start, reaching 1.0 at frame 100, holding after.
        ramps.set_breakpoints(0, 0.2, &[(0, 0.2), (100, 1.0)]);
        assert!(bank.trigger_with_automation(
            "meter",
            track(0),
            1_000,
            220.0,
            1.0,
            0.0,
            [0.2, 0.0, 0.0, 0.0],
            &ramps,
        ));

        let rendered = render_meter_frames(&mut bank, 200);
        #[allow(clippy::cast_precision_loss, clippy::suboptimal_flops)]
        let expected = |frame: usize| {
            let value = if frame >= 100 {
                1.0
            } else {
                0.2 + 0.8 * (frame as f32 / 100.0)
            };
            value * CENTER
        };
        for (frame, &sample) in rendered.iter().enumerate() {
            assert!(
                (sample - expected(frame)).abs() < 1e-5,
                "frame {frame}: got {sample}, expected {}",
                expected(frame)
            );
        }
    }

    #[test]
    fn ramp_playback_has_no_step_discontinuities() {
        let mut bank = GraphVoiceBank::with_user_programs(SR, vec![param_meter_spec(1)]);
        let mut ramps = VoiceParamRamps::none();
        // Up then down: the reversal at frame 480 is the step-risk point.
        ramps.set_breakpoints(0, 0.0, &[(0, 0.0), (480, 1.0), (960, 0.0)]);
        assert!(bank.trigger_with_automation(
            "meter",
            track(0),
            2_000,
            220.0,
            1.0,
            0.0,
            [0.0; VOICE_PARAM_COUNT],
            &ramps,
        ));

        let rendered = render_meter_frames(&mut bank, 1_200);
        let max_slope = CENTER / 480.0;
        for (frame, pair) in rendered.windows(2).enumerate() {
            let step = (pair[1] - pair[0]).abs();
            assert!(
                step <= max_slope + 1e-6,
                "frame {frame}: step {step} exceeds the interpolation slope {max_slope}"
            );
        }
    }

    #[test]
    #[allow(clippy::float_cmp)] // bit-identical regression contract
    fn constant_param_triggers_render_bit_identical_without_automation() {
        let render = |use_automation: bool| {
            let mut bank = GraphVoiceBank::with_user_programs(SR, vec![param_meter_spec(1)]);
            let params = [0.4, 0.0, 0.0, 0.0];
            if use_automation {
                assert!(bank.trigger_with_automation(
                    "meter",
                    track(0),
                    500,
                    220.0,
                    1.0,
                    0.0,
                    params,
                    &VoiceParamRamps::none(),
                ));
            } else {
                assert!(bank.trigger_with_params("meter", track(0), 500, 220.0, 1.0, 0.0, params));
            }
            render_meter_frames(&mut bank, 600)
        };
        assert_eq!(
            render(false),
            render(true),
            "a trigger without breakpoints must render bit-identically \
             through the automation-aware path"
        );
    }

    #[test]
    fn graph_note_voice_ramps_stamp_frames_from_normalized_positions() {
        let breakpoints: std::sync::Arc<[VoiceParamBreakpoint]> = vec![
            VoiceParamBreakpoint::new(0.0, 100.0),
            VoiceParamBreakpoint::new(0.25, 200.0),
            VoiceParamBreakpoint::new(0.5, 400.0),
        ]
        .into();
        let trigger = SampleTrigger::named("meter")
            .with_voice_param(0, 100.0)
            .with_voice_param_ramp(0, breakpoints);
        let params = graph_note_voice_params(&trigger);
        let ramps = graph_note_voice_ramps(&trigger, 1_000, &params);

        assert!(ramps.is_active(0));
        assert!(!ramps.is_active(1));
        assert!((ramps.value_at(0, 0.0, 0) - 100.0).abs() < 1e-4);
        assert!((ramps.value_at(0, 0.0, 125) - 150.0).abs() < 1e-4);
        assert!((ramps.value_at(0, 0.0, 250) - 200.0).abs() < 1e-4);
        assert!((ramps.value_at(0, 0.0, 375) - 300.0).abs() < 1e-4);
        assert!(
            (ramps.value_at(0, 0.0, 900) - 400.0).abs() < 1e-4,
            "the last breakpoint's value must hold to the note's end"
        );

        // A trigger without ramps stamps no automation at all.
        let plain = SampleTrigger::named("meter").with_voice_param(0, 7.0);
        let plain_params = graph_note_voice_params(&plain);
        let plain_ramps = graph_note_voice_ramps(&plain, 1_000, &plain_params);
        assert!((0..VOICE_PARAM_COUNT).all(|index| !plain_ramps.is_active(index)));
    }

    #[test]
    fn graph_note_voice_ramps_skip_degenerate_breakpoints() {
        // Non-finite positions or values are skipped; positions clamp into
        // [0, 1]; a ramp that starts after the note's beginning gets an
        // implicit start breakpoint holding the trigger-time value.
        let breakpoints: std::sync::Arc<[VoiceParamBreakpoint]> = vec![
            VoiceParamBreakpoint::new(f64::NAN, 999.0),
            VoiceParamBreakpoint::new(0.5, f64::INFINITY),
            VoiceParamBreakpoint::new(0.5, 300.0),
            VoiceParamBreakpoint::new(7.0, 500.0),
        ]
        .into();
        let trigger = SampleTrigger::named("meter")
            .with_voice_param(0, 100.0)
            .with_voice_param_ramp(0, breakpoints);
        let params = graph_note_voice_params(&trigger);
        let ramps = graph_note_voice_ramps(&trigger, 1_000, &params);

        assert!(ramps.is_active(0));
        assert!(
            (ramps.value_at(0, 0.0, 0) - 100.0).abs() < 1e-4,
            "the implicit start breakpoint must hold the trigger-time value"
        );
        assert!((ramps.value_at(0, 0.0, 250) - 200.0).abs() < 1e-4);
        assert!((ramps.value_at(0, 0.0, 500) - 300.0).abs() < 1e-4);
        assert!(
            (ramps.value_at(0, 0.0, 1_000) - 500.0).abs() < 1e-4,
            "positions above 1 must clamp to the note's end"
        );
    }

    #[test]
    fn breakpoints_beyond_the_cap_are_dropped() {
        #[allow(clippy::cast_precision_loss)]
        let breakpoints: std::sync::Arc<[VoiceParamBreakpoint]> = (0..64)
            .map(|i| VoiceParamBreakpoint::new(f64::from(i) / 64.0, f64::from(i)))
            .collect::<Vec<_>>()
            .into();
        let trigger = SampleTrigger::named("meter")
            .with_voice_param(0, 0.0)
            .with_voice_param_ramp(0, breakpoints);
        let params = graph_note_voice_params(&trigger);
        let ramps = graph_note_voice_ramps(&trigger, 6_400, &params);

        assert!(ramps.is_active(0));
        #[allow(clippy::cast_precision_loss)]
        let capped = MAX_VOICE_PARAM_BREAKPOINTS as f32 - 1.0;
        assert!(
            (ramps.value_at(0, 0.0, 6_399) - capped).abs() < 1e-4,
            "only the first {MAX_VOICE_PARAM_BREAKPOINTS} breakpoints ship; \
             the last kept value holds to the note's end: {}",
            ramps.value_at(0, 0.0, 6_399)
        );
    }

    #[test]
    fn steal_stamps_the_new_notes_ramps() {
        let mut bank = GraphVoiceBank::with_user_programs(SR, vec![param_meter_spec(1)]);
        let mut first = VoiceParamRamps::none();
        first.set_breakpoints(0, 0.1, &[(0, 0.1), (100, 0.2)]);
        assert!(bank.trigger_with_automation(
            "meter",
            track(0),
            10_000,
            220.0,
            1.0,
            0.0,
            [0.1, 0.0, 0.0, 0.0],
            &first,
        ));
        let mut second = VoiceParamRamps::none();
        second.set_breakpoints(0, 0.5, &[(0, 0.5), (100, 1.0)]);
        assert!(
            bank.trigger_with_automation(
                "meter",
                track(0),
                10_000,
                440.0,
                1.0,
                0.0,
                [0.5, 0.0, 0.0, 0.0],
                &second,
            ),
            "the poly-1 pool must steal for the second note"
        );

        // Skip the one-frame retrigger gap, then the ramp restarts from the
        // NEW note's automation (age resets with the steal).
        let rendered = render_meter_frames(&mut bank, 102);
        assert!(
            (rendered[101] - CENTER).abs() < 1e-4,
            "a steal must adopt the new note's breakpoints: {}",
            rendered[101]
        );
    }

    #[test]
    #[allow(clippy::cast_precision_loss, clippy::suboptimal_flops)] // exact test constants
    fn steal_ramp_glides_onto_the_new_notes_automation_envelope() {
        // Steal x automation composition (#1424 param ramps x ADR 0012): a
        // stolen voice whose new note carries breakpoints must glide from
        // the stolen note's current value ONTO the new note's envelope —
        // no jump beyond the handover slope at the steal, an exact landing
        // on the envelope at the window's end, and the envelope alone
        // driving afterwards.
        let mut bank = GraphVoiceBank::with_user_programs(SR, vec![param_meter_spec(1)]);
        assert!(bank.trigger_with_params(
            "meter",
            track(0),
            10_000,
            220.0,
            1.0,
            0.0,
            [0.9, 0.0, 0.0, 0.0],
        ));
        // Let the first note render a while so its held value is what a
        // listener hears at the steal.
        let _ = render_meter_frames(&mut bank, 10);

        // The new note ramps p1 from 0.1 at its start to 0.5 at frame 960.
        let mut ramps = VoiceParamRamps::none();
        ramps.set_breakpoints(0, 0.1, &[(0, 0.1), (960, 0.5)]);
        assert!(
            bank.trigger_with_automation(
                "meter",
                track(0),
                10_000,
                440.0,
                1.0,
                0.0,
                [0.1, 0.0, 0.0, 0.0],
                &ramps,
            ),
            "the poly-1 pool must steal for the second note"
        );

        let slot = meter_slot(&bank);
        let window = bank.slots[slot].param_ramp_frames as usize;
        assert!(window > 4, "the default handover window spans many frames");
        let rendered = render_meter_frames(&mut bank, 1_000);
        let value = |frame: usize| rendered[frame] / CENTER;
        let envelope = |frame: usize| 0.1 + (0.5 - 0.1) * (frame.min(960) as f32 / 960.0);

        // The handover starts from the stolen note's value: the first
        // audible frame (index 1 — index 0 is the one-frame retrigger gap
        // with the gate low) sits within two glide steps of 0.9, nowhere
        // near the envelope's 0.1 start.
        let glide_step = (0.9 - 0.1) / window as f32;
        assert!(
            (value(1) - 0.9).abs() <= 2.0 * glide_step + 1e-4,
            "the glide must start from the stolen note's current value: \
             got {}, expected about 0.9",
            value(1)
        );

        // No discontinuity beyond the handover slope after the steal: every
        // per-frame step is bounded by the glide step plus the envelope's
        // own slope.
        let envelope_step = (0.5 - 0.1) / 960.0;
        for (frame, pair) in rendered.windows(2).enumerate().skip(1) {
            let step = (pair[1] - pair[0]).abs() / CENTER;
            assert!(
                step <= glide_step + envelope_step + 1e-4,
                "frame {frame}: step {step} exceeds the handover slope"
            );
        }

        // The glide lands exactly ON the envelope at the window's end...
        assert!(
            (value(window - 1) - envelope(window - 1)).abs() < 1e-5,
            "the glide must land exactly on the envelope: got {}, expected {}",
            value(window - 1),
            envelope(window - 1)
        );

        // ...and from then on the envelope alone drives the parameter.
        for frame in window..rendered.len() {
            assert!(
                (value(frame) - envelope(frame)).abs() < 1e-5,
                "frame {frame}: after the glide lands the envelope alone \
                 drives: got {}, expected {}",
                value(frame),
                envelope(frame)
            );
        }
    }
}
