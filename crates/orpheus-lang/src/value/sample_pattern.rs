
use std::fmt;


use orpheus_pattern::{CyclePattern, Event, EventStream, PatternNode, Rational, TimeSpan};

use crate::eval::EvalError;
use crate::value::{sort_events, roll_event_cluster, strum_event_cluster, arp_event_cluster, invert_event_cluster, drop_event_cluster};
use crate::value::{Value, PitchClassSetValue, ArpDirectionValue, FunctionValue, GatePatternValue, whole_number_from_degree_value, map_degree_to_semitones};
use super::{PatternRuntime, NumberPatternValue};

#[derive(Clone, Debug, PartialEq)]
pub struct SampleEvent {
    /// ⚡ Bolt: `Arc<str>` is used instead of `Box<str>` because pattern transformations
    /// frequently clone `SampleEvent`. Using `Arc` replaces deep string allocations with
    /// a fast, atomic reference count increment while remaining `Send + Sync`.
    sample: std::sync::Arc<str>,
    gain: f64,
    hpf_cutoff_hz: Option<f64>,
    lpf_cutoff_hz: Option<f64>,
    resonance: f64,
    drive: f64,
    pulse_width: f64,
    pan: f64,
    rate: f64,
    delay_mix: f64,
    delay_time: f64,
    delay_feedback: f64,
    reverb_mix: f64,
    reverb_room: f64,
    reverb_damp: f64,
    chorus_mix: f64,
    chorus_depth: f64,
    chorus_rate: f64,
    compressor_mix: f64,
    compressor_threshold: f64,
    compressor_ratio: f64,
    onset_index: Option<u32>,
    slice_start: f64,
    slice_end: f64,
}

impl SampleEvent {
    pub(crate) fn named(sample: &str) -> Self {
        Self {
            sample: sample.into(),
            gain: 1.0,
            hpf_cutoff_hz: None,
            lpf_cutoff_hz: None,
            resonance: 0.2,
            drive: 1.0,
            pulse_width: 0.5,
            pan: 0.0,
            rate: 1.0,
            delay_mix: 0.0,
            delay_time: 0.125,
            delay_feedback: 0.35,
            reverb_mix: 0.0,
            reverb_room: 0.75,
            reverb_damp: 0.35,
            chorus_mix: 0.0,
            chorus_depth: 0.4,
            chorus_rate: 0.5,
            compressor_mix: 0.0,
            compressor_threshold: 0.5,
            compressor_ratio: 4.0,
            onset_index: None,
            slice_start: 0.0,
            slice_end: 1.0,
        }
    }

    /// The string identifier of the raw audio sample.
    #[must_use]
    pub fn sample(&self) -> &str {
        self.sample.as_ref()
    }

    /// The amplitude multiplier applied to this event.
    #[must_use]
    pub const fn gain(&self) -> f64 {
        self.gain
    }

    /// The high-pass filter cutoff frequency in Hertz, if one is active.
    #[must_use]
    pub const fn hpf_cutoff_hz(&self) -> Option<f64> {
        self.hpf_cutoff_hz
    }

    /// The low-pass filter cutoff frequency in Hertz, if one is active.
    #[must_use]
    pub const fn lpf_cutoff_hz(&self) -> Option<f64> {
        self.lpf_cutoff_hz
    }

    /// The ladder filter resonance amount for synth-style events.
    #[must_use]
    pub const fn resonance(&self) -> f64 {
        self.resonance
    }

    /// The synth drive amount.
    #[must_use]
    pub const fn drive(&self) -> f64 {
        self.drive
    }

    /// The pulse oscillator width, in the open interval `(0, 1)`.
    #[must_use]
    pub const fn pulse_width(&self) -> f64 {
        self.pulse_width
    }

    /// The stereo panning position, clamped between -1.0 (Left) and 1.0 (Right).
    #[must_use]
    pub const fn pan(&self) -> f64 {
        self.pan
    }

    /// The playback speed multiplier. Values greater than 1 speed up and pitch up.
    #[must_use]
    pub const fn rate(&self) -> f64 {
        self.rate
    }

    /// The insert delay wet mix.
    #[must_use]
    pub const fn delay_mix(&self) -> f64 {
        self.delay_mix
    }

    /// The insert delay time expressed in cycle units.
    #[must_use]
    pub const fn delay_time(&self) -> f64 {
        self.delay_time
    }

    /// The insert delay feedback coefficient.
    #[must_use]
    pub const fn delay_feedback(&self) -> f64 {
        self.delay_feedback
    }

    /// The insert reverb wet mix.
    #[must_use]
    pub const fn reverb_mix(&self) -> f64 {
        self.reverb_mix
    }

    /// The insert reverb room-size control.
    #[must_use]
    pub const fn reverb_room(&self) -> f64 {
        self.reverb_room
    }

    /// The insert reverb damping control.
    #[must_use]
    pub const fn reverb_damp(&self) -> f64 {
        self.reverb_damp
    }

    /// The insert chorus wet mix.
    #[must_use]
    pub const fn chorus_mix(&self) -> f64 {
        self.chorus_mix
    }

    /// The insert chorus depth control.
    #[must_use]
    pub const fn chorus_depth(&self) -> f64 {
        self.chorus_depth
    }

    /// The insert chorus modulation rate.
    #[must_use]
    pub const fn chorus_rate(&self) -> f64 {
        self.chorus_rate
    }

    /// The insert compressor wet mix.
    #[must_use]
    pub const fn compressor_mix(&self) -> f64 {
        self.compressor_mix
    }

    /// The insert compressor threshold.
    #[must_use]
    pub const fn compressor_threshold(&self) -> f64 {
        self.compressor_threshold
    }

    /// The insert compressor ratio.
    #[must_use]
    pub const fn compressor_ratio(&self) -> f64 {
        self.compressor_ratio
    }

    /// The transient slice index selected for later sample-bank resolution.
    #[must_use]
    pub const fn onset_index(&self) -> Option<u32> {
        self.onset_index
    }

    /// The normalized starting position `[0, 1]` within the raw audio sample.
    #[must_use]
    pub const fn slice_start(&self) -> f64 {
        self.slice_start
    }

    /// The normalized ending position `[0, 1]` within the raw audio sample.
    #[must_use]
    pub const fn slice_end(&self) -> f64 {
        self.slice_end
    }

    fn clone_with(&self, mutate: impl FnOnce(&mut Self)) -> Self {
        let mut cloned = self.clone();
        mutate(&mut cloned);
        cloned
    }
}

pub trait PatternValueTransform: Sized {
    fn adjust_gain(&self, factor: f64) -> Self;
    fn adjust_delay_mix(&self, mix: f64) -> Self;
    fn adjust_delay_time(&self, time: f64) -> Self;
    fn adjust_delay_feedback(&self, feedback: f64) -> Self;
    fn adjust_hpf(&self, cutoff_hz: f64) -> Self;
    fn adjust_lpf(&self, cutoff_hz: f64) -> Self;
    fn adjust_reverb_mix(&self, mix: f64) -> Self;
    fn adjust_reverb_room(&self, room: f64) -> Self;
    fn adjust_reverb_damp(&self, damp: f64) -> Self;
    fn adjust_chorus_mix(&self, mix: f64) -> Self;
    fn adjust_chorus_depth(&self, depth: f64) -> Self;
    fn adjust_chorus_rate(&self, rate: f64) -> Self;
    fn adjust_compressor_mix(&self, mix: f64) -> Self;
    fn adjust_compressor_threshold(&self, threshold: f64) -> Self;
    fn adjust_compressor_ratio(&self, ratio: f64) -> Self;
    fn adjust_resonance(&self, resonance: f64) -> Self;
    fn adjust_drive(&self, drive: f64) -> Self;
    fn adjust_pulse_width(&self, pulse_width: f64) -> Self;
    fn adjust_pan(&self, amount: f64) -> Self;
    fn adjust_rate(&self, factor: f64) -> Self;
    fn adjust_onset(&self, onset_index: u32) -> Self;
    fn adjust_slice(&self, start: f64, end: f64) -> Self;
    fn map_degrees(&self, collection: &PitchClassSetValue) -> Result<Self, EvalError>;
    fn transpose_semitones(&self, semitones: f64) -> Result<Self, EvalError>;
}

pub trait PatternRuntimeValue: Clone + PatternValueTransform + Send + Sync + fmt::Debug + Sized {
    fn into_runtime_value(pattern: PatternRuntime<Self>) -> Value;
    fn try_from_runtime_value(value: Value) -> Result<PatternRuntime<Self>, EvalError>;
    fn try_from_rand(value: f64) -> Result<Self, EvalError>;
    fn roll_events(events: Vec<Event<Self>>, steps: u32) -> Result<Vec<Event<Self>>, EvalError>;
    fn strum_events(events: Vec<Event<Self>>) -> Result<Vec<Event<Self>>, EvalError>;
    fn arp_events(
        events: Vec<Event<Self>>,
        steps: u32,
        direction: ArpDirectionValue,
    ) -> Result<Vec<Event<Self>>, EvalError>;
    fn invert_events(events: Vec<Event<Self>>, count: u32) -> Result<Vec<Event<Self>>, EvalError>;
    fn drop_events(events: Vec<Event<Self>>, count: u32) -> Result<Vec<Event<Self>>, EvalError>;
}

impl PatternValueTransform for SampleEvent {
    fn adjust_gain(&self, factor: f64) -> Self {
        self.clone_with(|event| event.gain *= factor)
    }

    fn adjust_delay_mix(&self, mix: f64) -> Self {
        self.clone_with(|event| event.delay_mix = mix)
    }

    fn adjust_delay_time(&self, time: f64) -> Self {
        self.clone_with(|event| event.delay_time = time)
    }

    fn adjust_delay_feedback(&self, feedback: f64) -> Self {
        self.clone_with(|event| event.delay_feedback = feedback)
    }

    fn adjust_hpf(&self, cutoff_hz: f64) -> Self {
        self.clone_with(|event| event.hpf_cutoff_hz = Some(cutoff_hz))
    }

    fn adjust_lpf(&self, cutoff_hz: f64) -> Self {
        self.clone_with(|event| event.lpf_cutoff_hz = Some(cutoff_hz))
    }

    fn adjust_reverb_mix(&self, mix: f64) -> Self {
        self.clone_with(|event| event.reverb_mix = mix)
    }

    fn adjust_reverb_room(&self, room: f64) -> Self {
        self.clone_with(|event| event.reverb_room = room)
    }

    fn adjust_reverb_damp(&self, damp: f64) -> Self {
        self.clone_with(|event| event.reverb_damp = damp)
    }

    fn adjust_chorus_mix(&self, mix: f64) -> Self {
        self.clone_with(|event| event.chorus_mix = mix)
    }

    fn adjust_chorus_depth(&self, depth: f64) -> Self {
        self.clone_with(|event| event.chorus_depth = depth)
    }

    fn adjust_chorus_rate(&self, rate: f64) -> Self {
        self.clone_with(|event| event.chorus_rate = rate)
    }

    fn adjust_compressor_mix(&self, mix: f64) -> Self {
        self.clone_with(|event| event.compressor_mix = mix)
    }

    fn adjust_compressor_threshold(&self, threshold: f64) -> Self {
        self.clone_with(|event| event.compressor_threshold = threshold)
    }

    fn adjust_compressor_ratio(&self, ratio: f64) -> Self {
        self.clone_with(|event| event.compressor_ratio = ratio)
    }

    fn adjust_resonance(&self, resonance: f64) -> Self {
        self.clone_with(|event| event.resonance = resonance)
    }

    fn adjust_drive(&self, drive: f64) -> Self {
        self.clone_with(|event| event.drive = drive)
    }

    fn adjust_pulse_width(&self, pulse_width: f64) -> Self {
        self.clone_with(|event| event.pulse_width = pulse_width)
    }

    fn adjust_pan(&self, amount: f64) -> Self {
        self.clone_with(|event| event.pan = (event.pan + amount).clamp(-1.0, 1.0))
    }

    fn adjust_rate(&self, factor: f64) -> Self {
        self.clone_with(|event| event.rate *= factor)
    }

    fn adjust_onset(&self, onset_index: u32) -> Self {
        self.clone_with(|event| event.onset_index = Some(onset_index))
    }

    fn adjust_slice(&self, start: f64, end: f64) -> Self {
        self.clone_with(|event| {
            let current_start = event.slice_start;
            let current_range = event.slice_end - current_start;
            event.slice_start = current_range.mul_add(start, current_start);
            event.slice_end = current_range.mul_add(end, current_start);
        })
    }

    fn map_degrees(&self, _collection: &PitchClassSetValue) -> Result<Self, EvalError> {
        Err(EvalError::new(
            "internal evaluator error: degree mapping only applies to number patterns",
        ))
    }

    fn transpose_semitones(&self, _semitones: f64) -> Result<Self, EvalError> {
        Err(EvalError::new(
            "internal evaluator error: transposition only applies to number patterns",
        ))
    }
}

impl PatternValueTransform for f64 {
    fn adjust_gain(&self, _factor: f64) -> Self {
        *self
    }

    fn adjust_delay_mix(&self, _mix: f64) -> Self {
        *self
    }

    fn adjust_delay_time(&self, _time: f64) -> Self {
        *self
    }

    fn adjust_delay_feedback(&self, _feedback: f64) -> Self {
        *self
    }

    fn adjust_hpf(&self, _cutoff_hz: f64) -> Self {
        *self
    }

    fn adjust_lpf(&self, _cutoff_hz: f64) -> Self {
        *self
    }

    fn adjust_reverb_mix(&self, _mix: f64) -> Self {
        *self
    }

    fn adjust_reverb_room(&self, _room: f64) -> Self {
        *self
    }

    fn adjust_reverb_damp(&self, _damp: f64) -> Self {
        *self
    }

    fn adjust_chorus_mix(&self, _mix: f64) -> Self {
        *self
    }

    fn adjust_chorus_depth(&self, _depth: f64) -> Self {
        *self
    }

    fn adjust_chorus_rate(&self, _rate: f64) -> Self {
        *self
    }

    fn adjust_compressor_mix(&self, _mix: f64) -> Self {
        *self
    }

    fn adjust_compressor_threshold(&self, _threshold: f64) -> Self {
        *self
    }

    fn adjust_compressor_ratio(&self, _ratio: f64) -> Self {
        *self
    }

    fn adjust_resonance(&self, _resonance: f64) -> Self {
        *self
    }

    fn adjust_drive(&self, _drive: f64) -> Self {
        *self
    }

    fn adjust_pulse_width(&self, _pulse_width: f64) -> Self {
        *self
    }

    fn adjust_pan(&self, _amount: f64) -> Self {
        *self
    }

    fn adjust_rate(&self, _factor: f64) -> Self {
        *self
    }

    fn adjust_onset(&self, _onset_index: u32) -> Self {
        *self
    }

    fn adjust_slice(&self, _start: f64, _end: f64) -> Self {
        *self
    }

    fn map_degrees(&self, collection: &PitchClassSetValue) -> Result<Self, EvalError> {
        let degree = whole_number_from_degree_value(*self)?;
        map_degree_to_semitones(degree, collection)
    }

    fn transpose_semitones(&self, semitones: f64) -> Result<Self, EvalError> {
        let value = *self + semitones;
        if value.is_finite() {
            Ok(value)
        } else {
            Err(EvalError::new(
                "`transpose` produced a non-finite numeric value",
            ))
        }
    }
}

impl PatternRuntimeValue for SampleEvent {
    fn into_runtime_value(pattern: PatternRuntime<Self>) -> Value {
        Value::SamplePattern(SamplePatternValue { pattern })
    }

    fn try_from_runtime_value(value: Value) -> Result<PatternRuntime<Self>, EvalError> {
        match value {
            Value::SamplePattern(pattern) => Ok(pattern.pattern),
            Value::NumberPattern(_)
            | Value::ArpDirection(_)
            | Value::PitchClassSet(_)
            | Value::Function(_)
            | Value::String(_) => Err(EvalError::new(
                "transform returned an incompatible value; expected Pattern<Sample>",
            )),
        }
    }

    fn try_from_rand(_value: f64) -> Result<Self, EvalError> {
        Err(EvalError::new("rand only produces numbers"))
    }

    /// ⚡ Bolt: Uses slice bounds (`&events[start_index..index]`) instead of allocating a temporary `cluster` Vec for every group of events with the same span, eliminating redundant heap allocations in the hot evaluation loop.
    fn roll_events(
        mut events: Vec<Event<Self>>,
        steps: u32,
    ) -> Result<Vec<Event<Self>>, EvalError> {
        sort_events(&mut events);
        let mut rolled = Vec::with_capacity(events.len());
        let mut index = 0;

        while index < events.len() {
            let span = events[index].part.clone();
            let start_index = index;
            while index < events.len() && events[index].part == span {
                index += 1;
            }

            rolled.extend(roll_event_cluster(&events[start_index..index], steps)?);
        }

        sort_events(&mut rolled);
        Ok(rolled)
    }

    fn strum_events(_events: Vec<Event<Self>>) -> Result<Vec<Event<Self>>, EvalError> {
        Err(EvalError::new(
            "internal evaluator error: strum only applies to number patterns",
        ))
    }

    fn arp_events(
        _events: Vec<Event<Self>>,
        _steps: u32,
        _direction: ArpDirectionValue,
    ) -> Result<Vec<Event<Self>>, EvalError> {
        Err(EvalError::new(
            "internal evaluator error: arp only applies to number patterns",
        ))
    }

    fn invert_events(
        _events: Vec<Event<Self>>,
        _count: u32,
    ) -> Result<Vec<Event<Self>>, EvalError> {
        Err(EvalError::new(
            "internal evaluator error: inversion only applies to number patterns",
        ))
    }

    fn drop_events(_events: Vec<Event<Self>>, _count: u32) -> Result<Vec<Event<Self>>, EvalError> {
        Err(EvalError::new(
            "internal evaluator error: drop voicings only apply to number patterns",
        ))
    }
}

impl PatternRuntimeValue for f64 {
    fn into_runtime_value(pattern: PatternRuntime<Self>) -> Value {
        Value::NumberPattern(NumberPatternValue { pattern })
    }

    fn try_from_runtime_value(value: Value) -> Result<PatternRuntime<Self>, EvalError> {
        match value {
            Value::NumberPattern(pattern) => Ok(pattern.pattern),
            Value::SamplePattern(_)
            | Value::ArpDirection(_)
            | Value::PitchClassSet(_)
            | Value::Function(_)
            | Value::String(_) => Err(EvalError::new(
                "transform returned an incompatible value; expected Pattern<Number>",
            )),
        }
    }

    fn try_from_rand(value: f64) -> Result<Self, EvalError> {
        Ok(value)
    }

    /// ⚡ Bolt: Uses slice bounds (`&events[start_index..index]`) instead of allocating a temporary `cluster` Vec for every group of events with the same span, eliminating redundant heap allocations in the hot evaluation loop.
    fn roll_events(
        mut events: Vec<Event<Self>>,
        steps: u32,
    ) -> Result<Vec<Event<Self>>, EvalError> {
        sort_events(&mut events);
        let mut rolled = Vec::with_capacity(events.len());
        let mut index = 0;

        while index < events.len() {
            let span = events[index].part.clone();
            let start_index = index;
            while index < events.len() && events[index].part == span {
                if !events[index].value.is_finite() {
                    return Err(EvalError::new("`roll` requires finite numeric values"));
                }
                index += 1;
            }

            rolled.extend(roll_event_cluster(&events[start_index..index], steps)?);
        }

        sort_events(&mut rolled);
        Ok(rolled)
    }

    /// Applies a strum effect across overlapping events.
    ///
    /// ⚡ Bolt: Mutates overlapping clusters in-place, eliminating the need to allocate and copy into an intermediate `strummed` vector.
    fn strum_events(mut events: Vec<Event<Self>>) -> Result<Vec<Event<Self>>, EvalError> {
        sort_events(&mut events);
        let mut index = 0;

        while index < events.len() {
            let span = events[index].part.clone();
            let start_index = index;
            while index < events.len() && events[index].part == span {
                if !events[index].value.is_finite() {
                    return Err(EvalError::new("`strum` requires finite numeric values"));
                }
                index += 1;
            }

            let cluster = &mut events[start_index..index];
            strum_event_cluster(cluster)?;
        }

        sort_events(&mut events);
        Ok(events)
    }

    fn arp_events(
        mut events: Vec<Event<Self>>,
        steps: u32,
        direction: ArpDirectionValue,
    ) -> Result<Vec<Event<Self>>, EvalError> {
        sort_events(&mut events);
        let mut arped = Vec::with_capacity(events.len() * steps as usize);
        let mut index = 0;

        while index < events.len() {
            let span = events[index].part.clone();
            let start_index = index;
            while index < events.len() && events[index].part == span {
                if !events[index].value.is_finite() {
                    return Err(EvalError::new("`arp` requires finite numeric values"));
                }
                index += 1;
            }

            let cluster = &mut events[start_index..index];
            arped.extend(arp_event_cluster(cluster, steps, direction)?);
        }

        sort_events(&mut arped);
        Ok(arped)
    }

    /// Applies a chord inversion effect to overlapping events.
    ///
    /// ⚡ Bolt: Modifies clusters in-place and directly returns the original `events` vector, bypassing O(N) allocation overhead for intermediate `inverted` tracking.
    fn invert_events(
        mut events: Vec<Event<Self>>,
        count: u32,
    ) -> Result<Vec<Event<Self>>, EvalError> {
        sort_events(&mut events);
        let mut index = 0;

        while index < events.len() {
            let span = events[index].part.clone();
            let start_index = index;
            while index < events.len() && events[index].part == span {
                if !events[index].value.is_finite() {
                    return Err(EvalError::new("`invert` requires finite numeric values"));
                }
                index += 1;
            }

            let cluster = &mut events[start_index..index];
            invert_event_cluster(cluster, count)?;
        }

        Ok(events)
    }

    /// Drops the lowest `count` voices from overlapping chords down an octave.
    ///
    /// ⚡ Bolt: Applies the pitch drop in-place over mutable subslices of `events`, completely removing the `dropped` vector allocation step from the hot path.
    fn drop_events(
        mut events: Vec<Event<Self>>,
        count: u32,
    ) -> Result<Vec<Event<Self>>, EvalError> {
        sort_events(&mut events);
        let mut index = 0;

        while index < events.len() {
            let span = events[index].part.clone();
            let start_index = index;
            while index < events.len() && events[index].part == span {
                if !events[index].value.is_finite() {
                    return Err(EvalError::new("`drop` requires finite numeric values"));
                }
                index += 1;
            }

            let cluster = &mut events[start_index..index];
            drop_event_cluster(cluster, count)?;
        }

        Ok(events)
    }
}

/// A delayed computation representing a sequence of audio sample events over time.
///
/// This pattern can be queried over specific temporal windows to yield fully-realized
/// [`SampleEvent`]s.
///
/// Examples
///
/// You can query a sample pattern for its events using `query_unit()`.
/// Note that Orpheus operates on exact continuous time, returning `Event` objects
/// containing rational `TimeSpan`s.
///
/// Orpheus' defining characteristic is that patterns are not static arrays of
/// audio data; they are mathematical functions from time to events. This delayed
/// evaluation is what allows infinite nesting of transforms (like `fast` or `every`)
/// without memory overhead. The `SamplePatternValue` holds the actual AST node
/// graph representing these delayed transforms.
///
/// Examples
///
/// ```
/// use orpheus_lang::{eval_module, ReplMode, SamplePatternValue, SampleEvent};
/// use orpheus_pattern::{Event, TimeSpan};
///
/// // Simulate the Orpheus expression `bd sn`
/// let env = eval_module("pattern = bd", ReplMode::Strict).unwrap();
/// let pattern = env.get("pattern").unwrap().as_sample_pattern().unwrap();
/// let events = pattern.query_unit().unwrap();
///
/// assert_eq!(events.len(), 1);
/// assert_eq!(events[0].value.sample(), "bd");
/// ```
///
/// ```
/// use orpheus_lang::{eval_module, ReplMode, SamplePatternValue, SampleEvent};
/// use orpheus_pattern::{Event, TimeSpan};
///
/// // Simulate the Orpheus expression `bd sn`
/// let env = eval_module("pattern = bd sn", ReplMode::Strict).unwrap();
/// let pattern = env.get("pattern").unwrap().as_sample_pattern().unwrap();
///
/// // By querying the unit cycle, we get the two events inside the period [0, 1).
/// let events = pattern.query_unit().unwrap();
///
/// assert_eq!(events.len(), 2);
/// assert_eq!(events[0].value.sample(), "bd");
/// assert_eq!(events[1].value.sample(), "sn");
/// ```
#[derive(Clone, Debug)]
pub struct SamplePatternValue {
    pub(crate) pattern: PatternRuntime<SampleEvent>,
}

impl SamplePatternValue {
    pub(crate) fn atom(sample: &str) -> Self {
        Self::from_nodes(vec![PatternNode::atom(SampleEvent::named(sample))])
    }

    pub(crate) const fn from_nodes(nodes: Vec<PatternNode<SampleEvent>>) -> Self {
        Self {
            pattern: PatternRuntime::Cycle(CyclePattern::from_nodes(nodes)),
        }
    }

    pub(crate) fn from_group(nodes: Vec<PatternNode<SampleEvent>>) -> Self {
        Self {
            pattern: PatternRuntime::Cycle(CyclePattern::from_nodes(vec![PatternNode::group(
                nodes,
            )])),
        }
    }

    pub(crate) fn stack(patterns: Vec<Self>) -> Self {
        Self {
            pattern: PatternRuntime::Stack(
                patterns
                    .into_iter()
                    .map(|pattern| pattern.pattern)
                    .collect(),
            ),
        }
    }

    pub(crate) fn fast(self, factor: i64) -> Self {
        Self {
            pattern: PatternRuntime::Fast {
                factor,
                inner: Box::new(self.pattern),
            },
        }
    }

    pub(crate) fn chaos_with_site_salt(self, site_salt: u64) -> Self {
        Self {
            pattern: PatternRuntime::Chaos {
                site_salt,
                inner: Box::new(self.pattern),
            },
        }
    }

    pub(crate) fn every(self, period: i64, transform: FunctionValue) -> Self {
        Self {
            pattern: PatternRuntime::Every {
                period,
                transform,
                inner: Box::new(self.pattern),
            },
        }
    }

    pub(crate) fn when(self, period: i64, offset: i64, transform: FunctionValue) -> Self {
        Self {
            pattern: PatternRuntime::When {
                period,
                offset,
                transform,
                inner: Box::new(self.pattern),
            },
        }
    }

    pub(crate) fn sometimes_with_site_salt(self, transform: FunctionValue, site_salt: u64) -> Self {
        Self {
            pattern: PatternRuntime::Sometimes {
                site_salt,
                transform,
                inner: Box::new(self.pattern),
            },
        }
    }

    pub(crate) fn within(self, start: Rational, end: Rational, transform: FunctionValue) -> Self {
        Self {
            pattern: PatternRuntime::Within {
                start,
                end,
                transform,
                inner: Box::new(self.pattern),
            },
        }
    }

    pub(crate) fn mask(self, gate: GatePatternValue) -> Self {
        Self {
            pattern: PatternRuntime::Mask {
                gate: Box::new(gate.into_runtime()),
                inner: Box::new(self.pattern),
            },
        }
    }

    pub(crate) fn slow(self, factor: i64) -> Self {
        Self {
            pattern: PatternRuntime::Slow {
                factor,
                inner: Box::new(self.pattern),
            },
        }
    }

    pub(crate) fn shift(self, offset: Rational) -> Self {
        Self {
            pattern: PatternRuntime::Shift {
                offset,
                inner: Box::new(self.pattern),
            },
        }
    }

    pub(crate) fn rev(self) -> Self {
        Self {
            pattern: PatternRuntime::Rev {
                inner: Box::new(self.pattern),
            },
        }
    }

    pub(crate) fn roll(self, steps: u32) -> Self {
        Self {
            pattern: PatternRuntime::Roll {
                steps,
                inner: Box::new(self.pattern),
            },
        }
    }

    pub(crate) fn gain(self, factor: f64) -> Self {
        Self {
            pattern: PatternRuntime::Gain {
                factor,
                inner: Box::new(self.pattern),
            },
        }
    }

    pub(crate) fn gain_pattern(self, control: NumberPatternValue) -> Self {
        Self {
            pattern: PatternRuntime::GainPattern {
                control: Box::new(control.pattern),
                inner: Box::new(self.pattern),
            },
        }
    }

    pub(crate) fn delay(self, mix: f64) -> Self {
        Self {
            pattern: PatternRuntime::Delay {
                mix,
                inner: Box::new(self.pattern),
            },
        }
    }

    pub(crate) fn delay_pattern(self, control: NumberPatternValue) -> Self {
        Self {
            pattern: PatternRuntime::DelayPattern {
                control: Box::new(control.pattern),
                inner: Box::new(self.pattern),
            },
        }
    }

    pub(crate) fn delay_time(self, time: f64) -> Self {
        Self {
            pattern: PatternRuntime::DelayTime {
                time,
                inner: Box::new(self.pattern),
            },
        }
    }

    pub(crate) fn delay_time_pattern(self, control: NumberPatternValue) -> Self {
        Self {
            pattern: PatternRuntime::DelayTimePattern {
                control: Box::new(control.pattern),
                inner: Box::new(self.pattern),
            },
        }
    }

    pub(crate) fn delay_feedback(self, feedback: f64) -> Self {
        Self {
            pattern: PatternRuntime::DelayFeedback {
                feedback,
                inner: Box::new(self.pattern),
            },
        }
    }

    pub(crate) fn delay_feedback_pattern(self, control: NumberPatternValue) -> Self {
        Self {
            pattern: PatternRuntime::DelayFeedbackPattern {
                control: Box::new(control.pattern),
                inner: Box::new(self.pattern),
            },
        }
    }

    pub(crate) fn hpf(self, cutoff_hz: f64) -> Self {
        Self {
            pattern: PatternRuntime::Hpf {
                cutoff_hz,
                inner: Box::new(self.pattern),
            },
        }
    }

    pub(crate) fn hpf_pattern(self, control: NumberPatternValue) -> Self {
        Self {
            pattern: PatternRuntime::HpfPattern {
                control: Box::new(control.pattern),
                inner: Box::new(self.pattern),
            },
        }
    }

    pub(crate) fn lpf(self, cutoff_hz: f64) -> Self {
        Self {
            pattern: PatternRuntime::Lpf {
                cutoff_hz,
                inner: Box::new(self.pattern),
            },
        }
    }

    pub(crate) fn lpf_pattern(self, control: NumberPatternValue) -> Self {
        Self {
            pattern: PatternRuntime::LpfPattern {
                control: Box::new(control.pattern),
                inner: Box::new(self.pattern),
            },
        }
    }

    pub(crate) fn reverb(self, mix: f64) -> Self {
        Self {
            pattern: PatternRuntime::Reverb {
                mix,
                inner: Box::new(self.pattern),
            },
        }
    }

    pub(crate) fn reverb_pattern(self, control: NumberPatternValue) -> Self {
        Self {
            pattern: PatternRuntime::ReverbPattern {
                control: Box::new(control.pattern),
                inner: Box::new(self.pattern),
            },
        }
    }

    pub(crate) fn reverb_room(self, room: f64) -> Self {
        Self {
            pattern: PatternRuntime::ReverbRoom {
                room,
                inner: Box::new(self.pattern),
            },
        }
    }

    pub(crate) fn reverb_room_pattern(self, control: NumberPatternValue) -> Self {
        Self {
            pattern: PatternRuntime::ReverbRoomPattern {
                control: Box::new(control.pattern),
                inner: Box::new(self.pattern),
            },
        }
    }

    pub(crate) fn reverb_damp(self, damp: f64) -> Self {
        Self {
            pattern: PatternRuntime::ReverbDamp {
                damp,
                inner: Box::new(self.pattern),
            },
        }
    }

    pub(crate) fn reverb_damp_pattern(self, control: NumberPatternValue) -> Self {
        Self {
            pattern: PatternRuntime::ReverbDampPattern {
                control: Box::new(control.pattern),
                inner: Box::new(self.pattern),
            },
        }
    }

    pub(crate) fn cutoff(self, cutoff_hz: f64) -> Self {
        self.lpf(cutoff_hz)
    }

    pub(crate) fn cutoff_pattern(self, control: NumberPatternValue) -> Self {
        self.lpf_pattern(control)
    }

    pub(crate) fn res(self, resonance: f64) -> Self {
        Self {
            pattern: PatternRuntime::Res {
                resonance,
                inner: Box::new(self.pattern),
            },
        }
    }

    pub(crate) fn res_pattern(self, control: NumberPatternValue) -> Self {
        Self {
            pattern: PatternRuntime::ResPattern {
                control: Box::new(control.pattern),
                inner: Box::new(self.pattern),
            },
        }
    }

    pub(crate) fn drive(self, drive: f64) -> Self {
        Self {
            pattern: PatternRuntime::Drive {
                drive,
                inner: Box::new(self.pattern),
            },
        }
    }

    pub(crate) fn drive_pattern(self, control: NumberPatternValue) -> Self {
        Self {
            pattern: PatternRuntime::DrivePattern {
                control: Box::new(control.pattern),
                inner: Box::new(self.pattern),
            },
        }
    }

    pub(crate) fn chorus(self, mix: f64) -> Self {
        Self {
            pattern: PatternRuntime::Chorus {
                mix,
                inner: Box::new(self.pattern),
            },
        }
    }

    pub(crate) fn chorus_pattern(self, control: NumberPatternValue) -> Self {
        Self {
            pattern: PatternRuntime::ChorusPattern {
                control: Box::new(control.pattern),
                inner: Box::new(self.pattern),
            },
        }
    }

    pub(crate) fn chorus_depth(self, depth: f64) -> Self {
        Self {
            pattern: PatternRuntime::ChorusDepth {
                depth,
                inner: Box::new(self.pattern),
            },
        }
    }

    pub(crate) fn chorus_depth_pattern(self, control: NumberPatternValue) -> Self {
        Self {
            pattern: PatternRuntime::ChorusDepthPattern {
                control: Box::new(control.pattern),
                inner: Box::new(self.pattern),
            },
        }
    }

    pub(crate) fn chorus_rate(self, rate: f64) -> Self {
        Self {
            pattern: PatternRuntime::ChorusRate {
                rate,
                inner: Box::new(self.pattern),
            },
        }
    }

    pub(crate) fn chorus_rate_pattern(self, control: NumberPatternValue) -> Self {
        Self {
            pattern: PatternRuntime::ChorusRatePattern {
                control: Box::new(control.pattern),
                inner: Box::new(self.pattern),
            },
        }
    }

    pub(crate) fn pulse_width(self, pulse_width: f64) -> Self {
        Self {
            pattern: PatternRuntime::PulseWidth {
                pulse_width,
                inner: Box::new(self.pattern),
            },
        }
    }

    pub(crate) fn pulse_width_pattern(self, control: NumberPatternValue) -> Self {
        Self {
            pattern: PatternRuntime::PulseWidthPattern {
                control: Box::new(control.pattern),
                inner: Box::new(self.pattern),
            },
        }
    }

    pub(crate) fn pan(self, amount: f64) -> Self {
        Self {
            pattern: PatternRuntime::Pan {
                amount,
                inner: Box::new(self.pattern),
            },
        }
    }

    pub(crate) fn pan_pattern(self, control: NumberPatternValue) -> Self {
        Self {
            pattern: PatternRuntime::PanPattern {
                control: Box::new(control.pattern),
                inner: Box::new(self.pattern),
            },
        }
    }

    pub(crate) fn compressor(self, mix: f64) -> Self {
        Self {
            pattern: PatternRuntime::Compressor {
                mix,
                inner: Box::new(self.pattern),
            },
        }
    }

    pub(crate) fn compressor_pattern(self, control: NumberPatternValue) -> Self {
        Self {
            pattern: PatternRuntime::CompressorPattern {
                control: Box::new(control.pattern),
                inner: Box::new(self.pattern),
            },
        }
    }

    pub(crate) fn compressor_threshold(self, threshold: f64) -> Self {
        Self {
            pattern: PatternRuntime::CompressorThreshold {
                threshold,
                inner: Box::new(self.pattern),
            },
        }
    }

    pub(crate) fn compressor_threshold_pattern(self, control: NumberPatternValue) -> Self {
        Self {
            pattern: PatternRuntime::CompressorThresholdPattern {
                control: Box::new(control.pattern),
                inner: Box::new(self.pattern),
            },
        }
    }

    pub(crate) fn compressor_ratio(self, ratio: f64) -> Self {
        Self {
            pattern: PatternRuntime::CompressorRatio {
                ratio,
                inner: Box::new(self.pattern),
            },
        }
    }

    pub(crate) fn compressor_ratio_pattern(self, control: NumberPatternValue) -> Self {
        Self {
            pattern: PatternRuntime::CompressorRatioPattern {
                control: Box::new(control.pattern),
                inner: Box::new(self.pattern),
            },
        }
    }

    pub(crate) fn pitch(self, semitones: f64) -> Self {
        Self {
            pattern: PatternRuntime::Pitch {
                semitones,
                inner: Box::new(self.pattern),
            },
        }
    }

    pub(crate) fn pitch_pattern(self, control: NumberPatternValue) -> Self {
        Self {
            pattern: PatternRuntime::PitchPattern {
                control: Box::new(control.pattern),
                inner: Box::new(self.pattern),
            },
        }
    }

    pub(crate) fn rate(self, factor: f64) -> Self {
        Self {
            pattern: PatternRuntime::Rate {
                factor,
                inner: Box::new(self.pattern),
            },
        }
    }

    pub(crate) fn rate_pattern(self, control: NumberPatternValue) -> Self {
        Self {
            pattern: PatternRuntime::RatePattern {
                control: Box::new(control.pattern),
                inner: Box::new(self.pattern),
            },
        }
    }

    pub(crate) fn onset(self, onset_index: u32) -> Self {
        Self {
            pattern: PatternRuntime::Onset {
                onset_index,
                inner: Box::new(self.pattern),
            },
        }
    }

    pub(crate) fn onset_pattern(self, control: NumberPatternValue) -> Self {
        Self {
            pattern: PatternRuntime::OnsetPattern {
                control: Box::new(control.pattern),
                inner: Box::new(self.pattern),
            },
        }
    }

    pub(crate) fn slice(self, start: f64, end: f64) -> Self {
        Self {
            pattern: PatternRuntime::Slice {
                start,
                end,
                inner: Box::new(self.pattern),
            },
        }
    }

    pub(crate) fn slice_pattern(
        self,
        start_control: NumberPatternValue,
        end_control: NumberPatternValue,
    ) -> Self {
        Self {
            pattern: PatternRuntime::SlicePattern {
                start_control: Box::new(start_control.pattern),
                end_control: Box::new(end_control.pattern),
                inner: Box::new(self.pattern),
            },
        }
    }

    pub(crate) fn slice_idx_pattern(self, control: NumberPatternValue, segments: u32) -> Self {
        Self {
            pattern: PatternRuntime::SliceIdxPattern {
                control: Box::new(control.pattern),
                segments,
                inner: Box::new(self.pattern),
            },
        }
    }

    pub(crate) fn from_events(events: Vec<Event<SampleEvent>>) -> Self {
        Self {
            pattern: PatternRuntime::Stream(EventStream::new(events)),
        }
    }

    /// Queries the pattern over the default unit cycle `[0, 1)`.
    ///
    /// Errors
    ///
    /// Returns an error if an internal runtime transform produces an invalid
    /// span or overflows the evaluator's bounded rational arithmetic.
    #[must_use = "query_unit() returns a Result; ignoring it may drop query errors"]
    #[allow(clippy::missing_errors_doc)]
    pub fn query_unit(&self) -> Result<Vec<Event<SampleEvent>>, EvalError> {
        self.try_query(&TimeSpan::unit())
    }

    pub(crate) fn try_query(&self, span: &TimeSpan) -> Result<Vec<Event<SampleEvent>>, EvalError> {
        self.pattern.try_query(span)
    }
}
