use orpheus_pattern::{CyclePattern, Event, EventStream, PatternNode, Rational, TimeSpan};

use crate::eval::EvalError;
use crate::value::{GatePatternValue, FunctionValue, ArpDirectionValue, PitchClassSetValue};
use super::PatternRuntime;

#[derive(Clone, Debug)]
pub struct NumberPatternValue {
    pub(crate) pattern: PatternRuntime<f64>,
}

impl NumberPatternValue {
    pub(crate) fn constant(value: f64) -> Self {
        Self::from_nodes(vec![PatternNode::atom(value)])
    }

    pub(crate) const fn from_nodes(nodes: Vec<PatternNode<f64>>) -> Self {
        Self {
            pattern: PatternRuntime::Cycle(CyclePattern::from_nodes(nodes)),
        }
    }

    pub(crate) fn from_group(nodes: Vec<PatternNode<f64>>) -> Self {
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

    pub(crate) fn degrees(self, collection: PitchClassSetValue) -> Self {
        Self {
            pattern: PatternRuntime::Degrees {
                collection,
                inner: Box::new(self.pattern),
            },
        }
    }

    pub(crate) fn chord(self, intervals: &[f64]) -> Self {
        let mut layers = Vec::with_capacity(intervals.len());
        if let Some((last, rest)) = intervals.split_last() {
            for interval in rest {
                layers.push(self.clone().transpose(*interval));
            }
            layers.push(self.transpose(*last));
        }
        Self::stack(layers)
    }

    pub(crate) fn invert(self, count: u32) -> Self {
        Self {
            pattern: PatternRuntime::Invert {
                count,
                inner: Box::new(self.pattern),
            },
        }
    }

    pub(crate) fn strum(self) -> Self {
        Self {
            pattern: PatternRuntime::Strum {
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

    pub(crate) fn arp(self, steps: u32, direction: ArpDirectionValue) -> Self {
        Self {
            pattern: PatternRuntime::Arp {
                steps,
                direction,
                inner: Box::new(self.pattern),
            },
        }
    }

    pub(crate) fn drop_voice(self, count: u32) -> Self {
        Self {
            pattern: PatternRuntime::Drop {
                count,
                inner: Box::new(self.pattern),
            },
        }
    }

    pub(crate) fn transpose(self, semitones: f64) -> Self {
        Self {
            pattern: PatternRuntime::Transpose {
                semitones,
                inner: Box::new(self.pattern),
            },
        }
    }

    pub(crate) fn transpose_pattern(self, control: Self) -> Self {
        Self {
            pattern: PatternRuntime::TransposePattern {
                control: Box::new(control.pattern),
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

    pub(crate) fn chaos_with_site_salt(self, site_salt: u64) -> Self {
        Self {
            pattern: PatternRuntime::Chaos {
                site_salt,
                inner: Box::new(self.pattern),
            },
        }
    }

    /// Queries the pattern over the default unit cycle `[0, 1)`.
    ///
    #[must_use]
    pub fn query_unit(&self) -> Vec<Event<f64>> {
        self.try_query_unit().unwrap_or_else(|_err| {
            // In a live-coding environment, gracefully degrade instead of crashing the UI
            Vec::new()
        })
    }

    /// Fallible variant of [`NumberPatternValue::query_unit`].
    ///
    /// Queries the pattern over the default unit cycle `[0, 1)`.
    ///
    /// # Errors
    ///
    /// Returns an error if an internal runtime transform produces an invalid
    /// span or overflows the evaluator's bounded rational arithmetic.
    pub fn try_query_unit(&self) -> Result<Vec<Event<f64>>, EvalError> {
        self.try_query(&TimeSpan::unit())
    }

    pub(crate) fn constant_value(&self) -> Result<f64, EvalError> {
        let events = self.try_query(&TimeSpan::unit())?;
        let unit = TimeSpan::unit();

        match events.as_slice() {
            [event] if event.whole.is_none() && event.part == unit => Ok(event.value),
            _ => Err(EvalError::new(
                "expected a constant number pattern over the unit cycle",
            )),
        }
    }

    /// # Errors
    /// Returns `EvalError` if querying fails.
    pub fn try_query(&self, span: &TimeSpan) -> Result<Vec<Event<f64>>, EvalError> {
        self.pattern.try_query(span)
    }

    pub(crate) fn from_events(events: Vec<Event<f64>>) -> Self {
        Self {
            pattern: PatternRuntime::Stream(EventStream::new(events)),
        }
    }

    pub(crate) const fn rand(site_salt: u64) -> Self {
        Self {
            pattern: PatternRuntime::Rand { site_salt },
        }
    }
}
