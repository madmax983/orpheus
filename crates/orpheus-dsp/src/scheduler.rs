//! The `scheduler` module implements the precise temporal scheduling of audio events.
//!
//! The scheduler maintains a queue of upcoming `SampleTrigger` events and translates
//! their logical fractional timing into exact frame-accurate offsets within the current
//! audio buffer, ensuring sample-accurate playback without jitter.

use std::collections::VecDeque;

use orpheus_pattern::{Event, Rational};

use crate::SampleTrigger;
use crate::engine::EngineError;
use crate::routing::TrackId;
use crate::voice::VoiceKind;

/// An audio event accurately scheduled for playback at a specific sample frame.
///
/// Maps logical pattern-based triggers (e.g., from a sequencer) into concrete,
/// time-stamped instructions that the audio thread will execute when that frame
/// is reached, ensuring sample-accurate timing.
///
/// ## Examples
///
/// ```
/// use orpheus_dsp::{ScheduledTrigger, SampleTrigger, TrackId, VoiceKind};
///
/// let sched = ScheduledTrigger {
///     frame: 44100,
///     duration_frames: 1000,
///     track_id: TrackId::new(1),
///     trigger: SampleTrigger::named("bd"),
///     fallback_voice: Some(VoiceKind::KickLike),
/// };
/// assert_eq!(sched.frame, 44100);
/// ```
#[derive(Clone, Debug, PartialEq)]
pub struct ScheduledTrigger {
    /// The absolute sample frame when this trigger should begin playback.
    pub frame: u64,
    /// The exact duration of the trigger in audio frames.
    pub duration_frames: u32,
    /// The identifier of the audio track this event routes to.
    pub track_id: TrackId,
    /// The core playback parameters (e.g., pitch, gain, sample name).
    pub trigger: SampleTrigger,
    /// A synthesized fallback voice to use if the requested sample isn't loaded.
    pub fallback_voice: Option<VoiceKind>,
}

/// Sample-clock scheduler that bridges exact pattern time to audio frames.
#[derive(Clone, Debug, Default)]
pub struct Scheduler {
    triggers: VecDeque<ScheduledTrigger>,
}

impl Scheduler {
    /// Creates an empty scheduler suitable for deterministic tests.
    #[must_use]
    pub fn new_for_test() -> Self {
        Self::default()
    }

    /// Pushes a named event into the test queue at an absolute sample frame.
    ///
    /// # Panics
    ///
    /// Panics if `token` does not resolve to one of the built-in drum voices.
    pub fn push_test_event(&mut self, frame: u64, token: &str) {
        self.schedule_trigger(frame, TrackId::new(0), token)
            .unwrap_or_else(|error| panic!("invalid test trigger: {error}"));
    }

    /// Converts cycle-relative pattern events into absolute sample triggers.
    ///
    /// The event start uses the clipped `part.start()` boundary because the
    /// current vertical slice only schedules one-shot sample tokens.
    ///
    /// # Errors
    ///
    /// Returns an error if the event time is negative or overflows the sample
    /// clock.
    pub fn schedule_cycle_events<'a, I>(
        &mut self,
        track_id: TrackId,
        cycle_start_frame: u64,
        frames_per_cycle: u64,
        events: I,
    ) -> Result<(), EngineError>
    where
        I: IntoIterator<Item = &'a Event<SampleTrigger>>,
    {
        let iter = events.into_iter();
        let (lower, upper) = iter.size_hint();
        // ⚡ Bolt: Pre-allocate vectors based on iterator size hints to avoid O(N) heap allocations.
        let mut pending = Vec::with_capacity(upper.unwrap_or(lower));
        for event in iter {
            let offset = rational_to_frame_offset(event.part.start(), frames_per_cycle)?;
            let frame = cycle_start_frame
                .checked_add(offset)
                .ok_or(EngineError::FrameOverflow)?;
            pending.push(ScheduledTrigger {
                frame,
                duration_frames: duration_frames_for_event(event, frames_per_cycle)?,
                track_id,
                trigger: event.value.clone(),
                fallback_voice: VoiceKind::from_token(event.value.token()),
            });
        }

        for trigger in pending {
            self.insert_trigger(trigger);
        }

        Ok(())
    }

    /// Drains all triggers due on or before `frame`, preserving insertion order
    /// for simultaneous events.
    #[must_use]
    pub fn drain_due_events(&mut self, frame: u64) -> Vec<String> {
        let mut due = Vec::new();
        while let Some(trigger) = self.pop_due(frame) {
            due.push(trigger.trigger.token().to_owned());
        }
        due
    }

    /// Pops the next trigger due on or before `frame`.
    pub fn pop_due(&mut self, frame: u64) -> Option<ScheduledTrigger> {
        if self
            .triggers
            .front()
            .is_some_and(|trigger| trigger.frame <= frame)
        {
            self.triggers.pop_front()
        } else {
            None
        }
    }

    /// Removes all scheduled triggers.
    pub fn clear(&mut self) {
        self.triggers.clear();
    }

    /// Schedules one built-in voice token at an absolute sample frame.
    ///
    /// # Errors
    ///
    /// Returns an error if `token` does not map to a built-in playback mapping.
    pub fn schedule_trigger(
        &mut self,
        frame: u64,
        track_id: TrackId,
        token: &str,
    ) -> Result<(), EngineError> {
        let voice =
            VoiceKind::from_token(token).ok_or_else(|| EngineError::UnknownVoice(token.into()))?;
        self.insert_trigger(ScheduledTrigger {
            frame,
            duration_frames: 1,
            track_id,
            trigger: SampleTrigger::named(token),
            fallback_voice: Some(voice),
        });
        Ok(())
    }

    fn insert_trigger(&mut self, trigger: ScheduledTrigger) {
        let index = self
            .triggers
            .iter()
            .position(|existing| existing.frame > trigger.frame)
            .unwrap_or(self.triggers.len());
        self.triggers.insert(index, trigger);
    }
}

fn rational_to_frame_offset(start: &Rational, frames_per_cycle: u64) -> Result<u64, EngineError> {
    if start.numerator() < 0 {
        return Err(EngineError::NegativeCycleOffset);
    }

    let scaled = start
        .numerator()
        .checked_mul(i128::from(frames_per_cycle))
        .ok_or(EngineError::FrameOverflow)?;
    let offset = scaled / start.denominator();
    u64::try_from(offset).map_err(|_| EngineError::FrameOverflow)
}

/// Frames from the trigger point (`part.start`) to the end of the event's
/// full extent.
///
/// The end comes from `whole` when present: an event clipped by a cycle
/// window keeps its unclipped extent there, so a note whose span crosses the
/// cycle boundary sustains into the next cycle instead of clamping at the
/// window edge (ADR 0009). An event clipped only at the window *start*
/// (whole begins before `part.start`) is unaffected: the duration always
/// runs forward from the trigger frame. Durations that exceed `u32` frames
/// saturate rather than erroring the audio thread.
fn duration_frames_for_event(
    event: &Event<SampleTrigger>,
    frames_per_cycle: u64,
) -> Result<u32, EngineError> {
    let start = rational_to_frame_offset(event.part.start(), frames_per_cycle)?;
    let extent_end = event.whole.as_ref().map_or_else(
        || event.part.end(),
        |whole| event.part.end().max(whole.end()),
    );
    let end = rational_to_frame_offset(extent_end, frames_per_cycle)?;
    let duration = end.saturating_sub(start).max(1);
    Ok(u32::try_from(duration).unwrap_or(u32::MAX))
}
