use orpheus_pattern::{Event, Rational};

use crate::engine::EngineError;
use crate::voice::VoiceKind;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ScheduledTrigger {
    pub frame: u64,
    pub token: Box<str>,
    pub voice: VoiceKind,
}

/// Sample-clock scheduler that bridges exact pattern time to audio frames.
#[derive(Clone, Debug, Default)]
pub struct Scheduler {
    triggers: Vec<ScheduledTrigger>,
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
    /// Returns an error if the event time is negative, overflows the sample
    /// clock, or names a voice token without a synthesized fallback.
    pub fn schedule_cycle_events<'a, I>(
        &mut self,
        cycle_start_frame: u64,
        frames_per_cycle: u64,
        events: I,
    ) -> Result<(), EngineError>
    where
        I: IntoIterator<Item = Event<&'a str>>,
    {
        for event in events {
            let offset = rational_to_frame_offset(event.part.start(), frames_per_cycle)?;
            let frame = cycle_start_frame
                .checked_add(offset)
                .ok_or(EngineError::FrameOverflow)?;
            self.schedule_trigger(frame, event.value)?;
        }

        Ok(())
    }

    /// Drains all triggers due on or before `frame`, preserving insertion order
    /// for simultaneous events.
    #[must_use]
    pub fn drain_due_events(&mut self, frame: u64) -> Vec<String> {
        self.drain_due(frame)
            .into_iter()
            .map(|trigger| trigger.token.into())
            .collect()
    }

    pub fn drain_due(&mut self, frame: u64) -> Vec<ScheduledTrigger> {
        let count = self
            .triggers
            .partition_point(|trigger| trigger.frame <= frame);
        self.triggers.drain(..count).collect()
    }

    /// Schedules one built-in voice token at an absolute sample frame.
    ///
    /// # Errors
    ///
    /// Returns an error if `token` does not map to a synthesized fallback.
    pub fn schedule_trigger(&mut self, frame: u64, token: &str) -> Result<(), EngineError> {
        let voice =
            VoiceKind::from_token(token).ok_or_else(|| EngineError::UnknownVoice(token.into()))?;
        let trigger = ScheduledTrigger {
            frame,
            token: token.into(),
            voice,
        };
        let index = self
            .triggers
            .partition_point(|existing| existing.frame <= trigger.frame);
        self.triggers.insert(index, trigger);
        Ok(())
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
