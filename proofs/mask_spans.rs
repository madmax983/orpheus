use vstd::prelude::*;

verus! {

pub open spec fn valid_span(start: int, end: int) -> bool {
    start <= end
}

pub open spec fn span_covers(start: int, end: int, slot: int) -> bool {
    start <= slot && slot < end
}

pub open spec fn spans_overlap(a_start: int, a_end: int, b_start: int, b_end: int) -> bool {
    if a_start >= b_start {
        a_start < if a_end <= b_end { a_end } else { b_end }
    } else {
        b_start < if a_end <= b_end { a_end } else { b_end }
    }
}

pub open spec fn clipped_start(source_start: int, gate_start: int) -> int {
    if source_start >= gate_start {
        source_start
    } else {
        gate_start
    }
}

pub open spec fn clipped_end(source_end: int, gate_end: int) -> int {
    if source_end <= gate_end {
        source_end
    } else {
        gate_end
    }
}

pub open spec fn merged_end(left_end: int, right_end: int) -> int {
    if left_end >= right_end {
        left_end
    } else {
        right_end
    }
}

pub proof fn sorted_adjacent_merge_preserves_coverage(
    left_start: int,
    left_end: int,
    right_start: int,
    right_end: int,
    slot: int,
)
    requires
        valid_span(left_start, left_end),
        valid_span(right_start, right_end),
        left_start <= right_start,
        right_start <= left_end,
    ensures
        valid_span(left_start, merged_end(left_end, right_end)),
        span_covers(left_start, merged_end(left_end, right_end), slot)
            <==> (span_covers(left_start, left_end, slot)
                || span_covers(right_start, right_end, slot)),
{
    if left_end >= right_end {
        assert(merged_end(left_end, right_end) == left_end);
    } else {
        assert(merged_end(left_end, right_end) == right_end);
    }

    assert(valid_span(left_start, merged_end(left_end, right_end)));

    if span_covers(left_start, merged_end(left_end, right_end), slot) {
        if slot < left_end {
            assert(span_covers(left_start, left_end, slot));
        } else {
            assert(left_end < merged_end(left_end, right_end));
            assert(merged_end(left_end, right_end) == right_end);
            assert(right_start <= left_end);
            assert(right_start <= slot);
            assert(slot < right_end);
            assert(span_covers(right_start, right_end, slot));
        }
    }

    if span_covers(left_start, left_end, slot) || span_covers(right_start, right_end, slot) {
        assert(left_start <= slot);
        if span_covers(left_start, left_end, slot) {
            assert(slot < left_end);
            assert(left_end <= merged_end(left_end, right_end));
        } else {
            assert(span_covers(right_start, right_end, slot));
            assert(slot < right_end);
            assert(right_end <= merged_end(left_end, right_end));
        }
        assert(slot < merged_end(left_end, right_end));
        assert(span_covers(left_start, merged_end(left_end, right_end), slot));
    }
}

pub proof fn no_overlap_produces_no_clipped_fragment(
    source_start: int,
    source_end: int,
    gate_start: int,
    gate_end: int,
)
    requires
        valid_span(source_start, source_end),
        valid_span(gate_start, gate_end),
        !spans_overlap(source_start, source_end, gate_start, gate_end),
    ensures
        clipped_end(source_end, gate_end) <= clipped_start(source_start, gate_start),
{
    if source_start >= gate_start {
        assert(!(source_start < clipped_end(source_end, gate_end)));
        assert(clipped_end(source_end, gate_end) <= source_start);
        assert(clipped_start(source_start, gate_start) == source_start);
    } else {
        assert(!(gate_start < clipped_end(source_end, gate_end)));
        assert(clipped_end(source_end, gate_end) <= gate_start);
        assert(clipped_start(source_start, gate_start) == gate_start);
    }
}

pub proof fn overlap_fragment_is_valid_and_inside_source_and_gate(
    source_start: int,
    source_end: int,
    gate_start: int,
    gate_end: int,
)
    requires
        valid_span(source_start, source_end),
        valid_span(gate_start, gate_end),
        spans_overlap(source_start, source_end, gate_start, gate_end),
    ensures
        source_start <= clipped_start(source_start, gate_start),
        gate_start <= clipped_start(source_start, gate_start),
        clipped_end(source_end, gate_end) <= source_end,
        clipped_end(source_end, gate_end) <= gate_end,
        source_start <= clipped_start(source_start, gate_start),
        clipped_end(source_end, gate_end) <= source_end,
        valid_span(
            clipped_start(source_start, gate_start),
            clipped_end(source_end, gate_end),
        ),
{
    if source_start >= gate_start {
        assert(clipped_start(source_start, gate_start) == source_start);
        assert(source_start < clipped_end(source_end, gate_end));
        assert(gate_start <= clipped_start(source_start, gate_start));
    } else {
        assert(clipped_start(source_start, gate_start) == gate_start);
        assert(gate_start < clipped_end(source_end, gate_end));
        assert(source_start <= clipped_start(source_start, gate_start));
    }

    if source_end <= gate_end {
        assert(clipped_end(source_end, gate_end) == source_end);
    } else {
        assert(clipped_end(source_end, gate_end) == gate_end);
    }

    assert(valid_span(
        clipped_start(source_start, gate_start),
        clipped_end(source_end, gate_end),
    ));
}

pub proof fn covered_slot_in_overlap_fragment_is_covered_by_both(
    source_start: int,
    source_end: int,
    gate_start: int,
    gate_end: int,
    slot: int,
)
    requires
        valid_span(source_start, source_end),
        valid_span(gate_start, gate_end),
        spans_overlap(source_start, source_end, gate_start, gate_end),
        span_covers(clipped_start(source_start, gate_start), clipped_end(source_end, gate_end), slot),
    ensures
        span_covers(source_start, source_end, slot),
        span_covers(gate_start, gate_end, slot),
{
    overlap_fragment_is_valid_and_inside_source_and_gate(
        source_start,
        source_end,
        gate_start,
        gate_end,
    );

    assert(clipped_start(source_start, gate_start) <= slot);
    assert(slot < clipped_end(source_end, gate_end));
    assert(source_start <= clipped_start(source_start, gate_start));
    assert(gate_start <= clipped_start(source_start, gate_start));
    assert(clipped_end(source_end, gate_end) <= source_end);
    assert(clipped_end(source_end, gate_end) <= gate_end);

    assert(source_start <= slot);
    assert(slot < source_end);
    assert(gate_start <= slot);
    assert(slot < gate_end);
}

} // verus!

fn main() {}
