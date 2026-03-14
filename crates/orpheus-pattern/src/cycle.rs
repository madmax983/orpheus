//! Repeating cycles and subdivision patterns.
//!
//! Cycle patterns map structural descriptions (like sequences of notes) onto an
//! infinitely repeating unit of time called a cycle. Groups within the sequence
//! evenly subdivide the duration allocated to them by their parent.

use core::cmp::{max, min};

use crate::{Event, PatternError, Rational, TimeSpan};

/// Queryable temporal pattern.
pub trait Pattern<T>: Send + Sync {
    /// Returns the events whose spans intersect the half-open window `span`.
    fn query(&self, span: TimeSpan) -> Vec<Event<T>>;
}

/// A single node in a cycle pattern tree.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PatternNode<T> {
    /// Emit `T` over this node's allocated subdivision.
    Atom(T),
    /// Occupy the subdivision without producing an event.
    Rest,
    /// Recursively subdivide this node's span among its children.
    Group(Vec<Self>),
}

impl<T> PatternNode<T> {
    /// Creates an atomic pattern node.
    ///
    /// # Examples
    ///
    /// ```
    /// use orpheus_pattern::PatternNode;
    ///
    /// let note = PatternNode::atom("bd");
    /// ```
    #[must_use]
    pub const fn atom(value: T) -> Self {
        Self::Atom(value)
    }

    /// Creates a rest node.
    #[must_use]
    pub const fn rest() -> Self {
        Self::Rest
    }

    /// Creates a grouped subdivision node.
    #[must_use]
    pub const fn group(nodes: Vec<Self>) -> Self {
        Self::Group(nodes)
    }
}

/// A repeating cycle pattern with equal subdivision semantics.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CyclePattern<T> {
    nodes: Vec<PatternNode<T>>,
}

impl<T> CyclePattern<T> {
    /// Creates a cycle pattern from root nodes.
    #[must_use]
    pub const fn from_nodes(nodes: Vec<PatternNode<T>>) -> Self {
        Self { nodes }
    }

    /// Queries the pattern over the half-open window `span`.
    ///
    /// # Errors
    ///
    /// Returns any arithmetic or span-construction error encountered while
    /// subdividing or shifting cycle-local events into the requested window.
    pub fn try_query(&self, span: &TimeSpan) -> Result<Vec<Event<T>>, PatternError>
    where
        T: Clone,
    {
        if span.is_empty() || self.nodes.is_empty() {
            return Ok(Vec::new());
        }

        query_cycle_pattern(&self.nodes, span)
    }
}

impl<T> Pattern<T> for CyclePattern<T>
where
    T: Clone + Send + Sync,
{
    fn query(&self, span: TimeSpan) -> Vec<Event<T>> {
        expect_query_result(&span, self.try_query(&span))
    }
}

fn query_cycle_pattern<T: Clone>(
    nodes: &[PatternNode<T>],
    span: &TimeSpan,
) -> Result<Vec<Event<T>>, PatternError> {
    let unit_events = collect_nodes(nodes, &TimeSpan::unit())?;
    let mut events = Vec::new();
    let start_cycle = floor_rational(span.start());
    let end_cycle = ceil_rational(span.end());

    for cycle in start_cycle..end_cycle {
        let cycle_offset = Rational::checked_from_parts(cycle, 1)?;
        push_cycle_events_at_offset(&mut events, &unit_events, &cycle_offset, span)?;
    }

    Ok(events)
}

fn expect_query_result<T>(
    span: &TimeSpan,
    result: Result<Vec<Event<T>>, PatternError>,
) -> Vec<Event<T>> {
    result.unwrap_or_else(|error| panic!("cycle pattern query failed for span {span:?}: {error}"))
}

fn collect_nodes<T: Clone>(
    nodes: &[PatternNode<T>],
    span: &TimeSpan,
) -> Result<Vec<(TimeSpan, T)>, PatternError> {
    let mut events = Vec::new();
    collect_nodes_into(nodes, span, &mut events)?;
    Ok(events)
}

fn collect_nodes_into<T: Clone>(
    nodes: &[PatternNode<T>],
    span: &TimeSpan,
    events: &mut Vec<(TimeSpan, T)>,
) -> Result<(), PatternError> {
    if nodes.is_empty() {
        return Ok(());
    }

    let count = i128::try_from(nodes.len()).map_err(|_| PatternError::ArithmeticOverflow {
        operation: "cycle subdivision",
    })?;
    let width = span
        .end()
        .checked_sub(span.start())?
        .checked_mul(&Rational::checked_from_parts(1, count)?)?;
    let mut cursor = *span.start();

    for (index, node) in nodes.iter().enumerate() {
        let next = if index + 1 == nodes.len() {
            *span.end()
        } else {
            cursor.checked_add(&width)?
        };
        let child_span = TimeSpan::new(cursor, next)?;

        match node {
            PatternNode::Atom(value) => events.push((child_span, value.clone())),
            PatternNode::Rest => {}
            PatternNode::Group(children) => collect_nodes_into(children, &child_span, events)?,
        }

        cursor = next;
    }

    Ok(())
}

fn push_cycle_events_at_offset<T: Clone>(
    events: &mut Vec<Event<T>>,
    unit_events: &[(TimeSpan, T)],
    cycle_offset: &Rational,
    span: &TimeSpan,
) -> Result<(), PatternError> {
    for (whole, value) in unit_events {
        let shifted_whole = shift_span(whole, cycle_offset)?;
        if let Some(part) = clip_span(&shifted_whole, span)? {
            let whole = if part == shifted_whole {
                None
            } else {
                Some(shifted_whole)
            };

            events.push(Event {
                whole,
                part,
                value: value.clone(),
            });
        }
    }

    Ok(())
}

fn shift_span(span: &TimeSpan, offset: &Rational) -> Result<TimeSpan, PatternError> {
    let start = span.start().checked_add(offset)?;
    let end = span.end().checked_add(offset)?;
    TimeSpan::new(start, end)
}

fn clip_span(span: &TimeSpan, query: &TimeSpan) -> Result<Option<TimeSpan>, PatternError> {
    let start = *max(span.start(), query.start());
    let end = *min(span.end(), query.end());

    if start >= end {
        return Ok(None);
    }

    TimeSpan::new(start, end).map(Some)
}

const fn floor_rational(value: &Rational) -> i128 {
    let quotient = value.numerator() / value.denominator();
    let remainder = value.numerator() % value.denominator();

    if remainder < 0 {
        quotient - 1
    } else {
        quotient
    }
}

const fn ceil_rational(value: &Rational) -> i128 {
    let quotient = value.numerator() / value.denominator();
    let remainder = value.numerator() % value.denominator();

    if remainder > 0 {
        quotient + 1
    } else {
        quotient
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn floor_and_ceil_handle_negative_rationals() {
        let value = Rational::new(-1, 2).unwrap();

        assert_eq!(floor_rational(&value), -1);
        assert_eq!(ceil_rational(&value), 0);
    }

    #[test]
    fn try_query_matches_query_for_regular_windows() {
        let pattern = CyclePattern::from_nodes(vec![PatternNode::atom("bd")]);
        let span = TimeSpan::new(Rational::zero(), Rational::one()).unwrap();

        assert_eq!(
            pattern.try_query(&span),
            Ok(vec![Event {
                whole: None,
                part: TimeSpan::unit(),
                value: "bd",
            }])
        );
    }

    #[test]
    fn push_cycle_events_at_offset_reports_shift_overflow() {
        let pattern = CyclePattern::from_nodes(vec![PatternNode::atom("bd")]);
        let unit_events = collect_nodes(&pattern.nodes, &TimeSpan::unit()).unwrap();
        let offset = Rational::checked_from_parts(i128::MAX, 1).unwrap();
        let span = TimeSpan::unit();

        assert_eq!(
            push_cycle_events_at_offset(&mut Vec::new(), &unit_events, &offset, &span),
            Err(PatternError::ArithmeticOverflow {
                operation: "rational addition",
            })
        );
    }

    #[test]
    #[should_panic(expected = "cycle pattern query failed for span")]
    fn query_wrapper_panics_with_context_when_result_is_err() {
        let span = TimeSpan::unit();
        let result = Err(PatternError::ArithmeticOverflow {
            operation: "rational addition",
        });

        let _ = expect_query_result::<&str>(&span, result);
    }
}
