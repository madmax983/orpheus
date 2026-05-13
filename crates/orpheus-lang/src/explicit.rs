use orpheus_pattern::{Event, Rational};

use crate::error::EvalError;
use crate::eval::{shift_span, sort_events, shift_events};
use crate::value::{NumberPatternValue, SampleEvent, SamplePatternValue, Value};

#[derive(Clone, Debug)]
pub enum ExplicitValue {
    Sample(Vec<Event<SampleEvent>>),
    Number(Vec<Event<f64>>),
}

impl ExplicitValue {
    pub fn into_value(self) -> Value {
        match self {
            Self::Sample(events) => Value::SamplePattern(SamplePatternValue::from_events(events)),
            Self::Number(events) => Value::NumberPattern(NumberPatternValue::from_events(events)),
        }
    }

    pub fn merge(mut self, other: Self) -> Result<Self, EvalError> {
        self.append_unsorted(other)?;
        self.sort();
        Ok(self)
    }

    pub fn append_unsorted(&mut self, mut other: Self) -> Result<(), EvalError> {
        match (self, &mut other) {
            (Self::Sample(left), Self::Sample(right)) => {
                left.append(right);
                Ok(())
            }
            (Self::Number(left), Self::Number(right)) => {
                left.append(right);
                Ok(())
            }
            (Self::Sample(_), Self::Number(_)) | (Self::Number(_), Self::Sample(_)) => Err(
                EvalError::new("explicit-time items must all resolve to the same pattern kind"),
            ),
        }
    }

    pub fn append_unsorted_shifted(&mut self, base: &Self, offset: &Rational) -> Result<(), EvalError> {
        match (self, base) {
            (Self::Sample(combined), Self::Sample(base_events)) => {
                Self::append_shifted_events(combined, base_events, offset)
            }
            (Self::Number(combined), Self::Number(base_events)) => {
                Self::append_shifted_events(combined, base_events, offset)
            }
            (Self::Sample(_), Self::Number(_)) | (Self::Number(_), Self::Sample(_)) => Err(
                EvalError::new("explicit-time items must all resolve to the same pattern kind"),
            ),
        }
    }

    fn append_shifted_events<T: Clone>(
        combined: &mut Vec<Event<T>>,
        base_events: &[Event<T>],
        offset: &Rational,
    ) -> Result<(), EvalError> {
        // ⚡ Bolt: Pre-allocate capacity to eliminate redundant heap allocations when appending events.
        combined.reserve(base_events.len());
        for event in base_events {
            let shifted_part = shift_span(&event.part, offset)?;
            let shifted_whole = if let Some(whole) = &event.whole {
                Some(shift_span(whole, offset)?)
            } else {
                None
            };

            // ⚡ Bolt: Avoid redundant allocations when shifting events.
            // By instantiating a new Event directly and only cloning `event.value`,
            // we eliminate unnecessary cloning of `TimeSpan` fields (`part` and `whole`)
            // that are immediately overwritten anyway.
            combined.push(Event {
                whole: shifted_whole,
                part: shifted_part,
                value: event.value.clone(),
            });
        }
        Ok(())
    }

    pub fn sort(&mut self) {
        match self {
            Self::Sample(events) => sort_events(events),
            Self::Number(events) => sort_events(events),
        }
    }

    pub fn shift(&mut self, offset: &Rational) -> Result<(), EvalError> {
        match self {
            Self::Sample(events) => shift_events(events, offset),
            Self::Number(events) => shift_events(events, offset),
        }
    }

    pub fn empty_with_capacity_matching(&self, multiplier: usize) -> Result<Self, EvalError> {
        let current_len = match self {
            Self::Sample(events) => events.len(),
            Self::Number(events) => events.len(),
        };

        let capacity = current_len
            .checked_mul(multiplier)
            .ok_or_else(|| EvalError::new("section pattern capacity overflowed"))?;

        if capacity > 100_000 {
            return Err(EvalError::new(
                "evaluation exceeded the maximum allowed event limit",
            ));
        }

        match self {
            Self::Sample(_) => Ok(Self::Sample(Vec::with_capacity(capacity))),
            Self::Number(_) => Ok(Self::Number(Vec::with_capacity(capacity))),
        }
    }
}
