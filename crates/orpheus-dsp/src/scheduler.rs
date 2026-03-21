//! The `scheduler` module implements the precise temporal scheduling of audio events.
//!
//! The scheduler maintains a queue of upcoming `SampleTrigger` events and translates
//! their logical fractional timing into exact frame-accurate offsets within the current
//! audio buffer, ensuring sample-accurate playback without jitter.

use std::collections::VecDeque;

use orpheus_pattern::{Event, Rational};

use crate::SampleTrigger;
use crate::engine::EngineError;
use crate::voice::VoiceKind;

#[derive(Clone, Debug, PartialEq)]
pub struct ScheduledTrigger {
    pub frame: u64,
    pub trigger: SampleTrigger,
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
        self.schedule_trigger(frame, token)
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
        cycle_start_frame: u64,
        frames_per_cycle: u64,
        events: I,
    ) -> Result<(), EngineError>
    where
        I: IntoIterator<Item = Event<&'a SampleTrigger>>,
    {
        let mut pending = Vec::new();
        for event in events {
            let offset = rational_to_frame_offset(event.part.start(), frames_per_cycle)?;
            let frame = cycle_start_frame
                .checked_add(offset)
                .ok_or(EngineError::FrameOverflow)?;
            pending.push(ScheduledTrigger {
                frame,
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
    pub fn schedule_trigger(&mut self, frame: u64, token: &str) -> Result<(), EngineError> {
        let voice =
            VoiceKind::from_token(token).ok_or_else(|| EngineError::UnknownVoice(token.into()))?;
        self.insert_trigger(ScheduledTrigger {
            frame,
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
