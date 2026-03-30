//! The `cycle` module implements repeating cycles and subdivision patterns.
//!
//! Cycle patterns map structural descriptions (like sequences of notes) onto an
//! infinitely repeating unit of time called a cycle. Groups within the sequence
//! evenly subdivide the duration allocated to them by their parent.

use core::cmp::{max, min};

use crate::{Event, PatternError, Rational, TimeSpan};

/// Queryable temporal pattern.
///
/// This trait is the foundational abstraction in Orpheus for structures that
/// map exact, rational time intervals to events. Patterns are evaluated by
/// querying them over a half-open window of time called a [`TimeSpan`].
pub trait Pattern<T>: Send + Sync {
    /// Returns the events whose spans intersect the half-open window `span`.
    ///
    /// # Examples
    ///
    /// ```
    /// use orpheus_pattern::{CyclePattern, Pattern, PatternNode, TimeSpan};
    ///
    /// let pattern = CyclePattern::from_nodes(vec![PatternNode::atom("bd")]);
    /// let events = pattern.query(TimeSpan::unit());
    ///
    /// assert_eq!(events.len(), 1);
    /// assert_eq!(events[0].value, "bd");
    /// ```
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
    ///
    /// ## Examples
    ///
    /// ```
    /// use orpheus_pattern::{CyclePattern, PatternNode, TimeSpan};
    ///
    /// let pattern = CyclePattern::from_nodes(vec![
    ///     PatternNode::atom("a"),
    ///     PatternNode::atom("b"),
    /// ]);
    /// let events = pattern.try_query(&TimeSpan::unit()).unwrap();
    ///
    /// assert_eq!(events.len(), 2);
    /// assert_eq!(events[0].value, "a");
    /// assert_eq!(events[1].value, "b");
    /// ```
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
    let start_cycle = floor_rational(span.start());
    let end_cycle = ceil_rational(span.end());

    let cycle_count = end_cycle.saturating_sub(start_cycle);
    if cycle_count > 1_000_000 {
        return Err(PatternError::ArithmeticOverflow {
            operation: "cycle range too large",
        });
    }

    let cycle_count = usize::try_from(cycle_count).unwrap_or(0);
    // ⚡ Bolt: Pre-allocate vector using the cycle count and unit event count
    // to reduce heap reallocations during pattern querying.
    let mut events = Vec::with_capacity(cycle_count.saturating_mul(unit_events.len()));

    for cycle in start_cycle..end_cycle {
        let cycle_offset = Rational::checked_from_parts(cycle, 1)?;
        push_cycle_events_at_offset(&mut events, &unit_events, &cycle_offset, span)?;
    }

    Ok(events)
}

fn expect_query_result<T>(
    _span: &TimeSpan,
    result: Result<Vec<Event<T>>, PatternError>,
) -> Vec<Event<T>> {
    result.unwrap_or_else(|_error| Vec::new())
}

fn collect_nodes<T: Clone>(
    nodes: &[PatternNode<T>],
    span: &TimeSpan,
) -> Result<Vec<(TimeSpan, T)>, PatternError> {
    // ⚡ Bolt: Pre-allocate vector using the number of top-level nodes as a lower bound
    // to reduce heap reallocations during pattern collection.
    let mut events = Vec::with_capacity(nodes.len());
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
    let mut cursor = span.start().clone();

    for (index, node) in nodes.iter().enumerate() {
        let next = if index + 1 == nodes.len() {
            span.end().clone()
        } else {
            cursor.checked_add(&width)?
        };
        let child_span = TimeSpan::new(cursor.clone(), next.clone())?;

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
    let start = max(span.start(), query.start()).clone();
    let end = min(span.end(), query.end()).clone();

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
    fn query_wrapper_returns_empty_when_result_is_err() {
        let span = TimeSpan::unit();
        let result = Err(PatternError::ArithmeticOverflow {
            operation: "rational addition",
        });

        let events = expect_query_result::<&str>(&span, result);
        assert!(events.is_empty());
    }

    #[test]
    fn try_query_returns_empty_when_span_is_empty() {
        let pattern = CyclePattern::from_nodes(vec![PatternNode::atom("bd")]);
        let span = TimeSpan::new(Rational::zero(), Rational::zero()).unwrap();
        assert_eq!(pattern.try_query(&span), Ok(vec![]));
    }

    #[test]
    fn try_query_returns_empty_when_nodes_are_empty() {
        let pattern: CyclePattern<&str> = CyclePattern::from_nodes(vec![]);
        assert_eq!(pattern.try_query(&TimeSpan::unit()), Ok(vec![]));
    }

    #[test]
    fn collect_nodes_into_handles_rest_nodes() {
        let pattern = CyclePattern::from_nodes(vec![PatternNode::atom("a"), PatternNode::rest()]);
        let events = pattern.try_query(&TimeSpan::unit()).unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].value, "a");
    }

    #[test]
    fn collect_nodes_into_handles_empty_group_nodes() {
        let pattern: CyclePattern<&str> =
            CyclePattern::from_nodes(vec![PatternNode::group(vec![])]);
        let events = pattern.try_query(&TimeSpan::unit()).unwrap();
        assert_eq!(events.len(), 0);
    }

    #[test]
    fn push_cycle_events_at_offset_clips_partial_events() {
        let pattern = CyclePattern::from_nodes(vec![PatternNode::atom("bd")]);
        let start = Rational::new(1, 2).unwrap();
        let end = Rational::new(3, 2).unwrap();
        let span = TimeSpan::new(start.clone(), end.clone()).unwrap();
        let events = pattern.try_query(&span).unwrap();

        assert_eq!(events.len(), 2);

        let first = &events[0];
        assert_eq!(first.value, "bd");
        assert_eq!(first.part, TimeSpan::new(start, Rational::one()).unwrap());
        assert_eq!(first.whole, Some(TimeSpan::unit()));

        let second = &events[1];
        assert_eq!(second.value, "bd");
        assert_eq!(second.part, TimeSpan::new(Rational::one(), end).unwrap());
        assert_eq!(
            second.whole,
            Some(TimeSpan::new(Rational::one(), Rational::new(2, 1).unwrap()).unwrap())
        );
    }
}
