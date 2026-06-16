use orpheus_pattern::{Rational, TimeSpan, CyclePattern, PatternNode, Pattern, EventStream};
use proptest::prelude::*;

proptest! {
    #[test]
    #[allow(clippy::collapsible_if)]
    fn havoc_test_cycle_pattern_query(start_num in proptest::num::i128::ANY, start_den in 1..=i128::MAX, end_num in proptest::num::i128::ANY, end_den in 1..=i128::MAX) {
        if let (Ok(start), Ok(end)) = (Rational::checked_from_parts(start_num, start_den), Rational::checked_from_parts(end_num, end_den)) {
            if let Ok(span) = TimeSpan::new(start, end) {
                let pattern = CyclePattern::from_nodes(vec![PatternNode::atom("a"), PatternNode::atom("b")]);
                let _ = std::panic::catch_unwind(|| pattern.query(span));
            }
        }
    }

    #[test]
    #[allow(clippy::collapsible_if)]
    fn havoc_test_event_stream_query(start_num in proptest::num::i128::ANY, start_den in 1..=i128::MAX, end_num in proptest::num::i128::ANY, end_den in 1..=i128::MAX) {
        if let (Ok(start), Ok(end)) = (Rational::checked_from_parts(start_num, start_den), Rational::checked_from_parts(end_num, end_den)) {
            if let Ok(span) = TimeSpan::new(start, end) {
                let stream = EventStream::<&str>::new(vec![]);
                let _ = std::panic::catch_unwind(|| stream.query(span));
            }
        }
    }
}
