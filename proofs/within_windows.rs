use vstd::prelude::*;

verus! {

pub open spec fn valid_window(start: int, end: int) -> bool {
    start < end
}

pub open spec fn valid_span(start: int, end: int) -> bool {
    start <= end
}

pub open spec fn span_inside_window(
    window_start: int,
    window_end: int,
    span_start: int,
    span_end: int,
) -> bool {
    valid_window(window_start, window_end)
        && valid_span(span_start, span_end)
        && window_start <= span_start
        && span_end <= window_end
}

pub open spec fn window_width(window_start: int, window_end: int) -> int {
    window_end - window_start
}

pub open spec fn localize_point(window_start: int, point: int) -> int {
    point - window_start
}

pub open spec fn restore_point(window_start: int, local_point: int) -> int {
    window_start + local_point
}

pub proof fn valid_window_has_positive_width(window_start: int, window_end: int)
    requires
        valid_window(window_start, window_end),
    ensures
        0 < window_width(window_start, window_end),
{
}

pub proof fn restore_localized_point_round_trips(window_start: int, point: int)
    ensures
        restore_point(window_start, localize_point(window_start, point)) == point,
{
}

pub proof fn localize_preserves_point_order(window_start: int, left: int, right: int)
    requires
        left <= right,
    ensures
        localize_point(window_start, left) <= localize_point(window_start, right),
{
}

pub proof fn restore_preserves_point_order(window_start: int, left: int, right: int)
    requires
        left <= right,
    ensures
        restore_point(window_start, left) <= restore_point(window_start, right),
{
}

pub proof fn localizing_in_window_span_stays_inside_local_width(
    window_start: int,
    window_end: int,
    span_start: int,
    span_end: int,
)
    requires
        span_inside_window(window_start, window_end, span_start, span_end),
    ensures
        0 <= localize_point(window_start, span_start),
        localize_point(window_start, span_end) <= window_width(window_start, window_end),
        valid_span(
            localize_point(window_start, span_start),
            localize_point(window_start, span_end),
        ),
{
    valid_window_has_positive_width(window_start, window_end);
    localize_preserves_point_order(window_start, span_start, span_end);

    assert(0 <= localize_point(window_start, span_start));
    assert(localize_point(window_start, span_end) <= window_width(window_start, window_end));
    assert(valid_span(
        localize_point(window_start, span_start),
        localize_point(window_start, span_end),
    ));
}

pub proof fn restoring_local_span_stays_inside_window(
    window_start: int,
    window_end: int,
    local_start: int,
    local_end: int,
)
    requires
        valid_window(window_start, window_end),
        valid_span(local_start, local_end),
        0 <= local_start,
        local_end <= window_width(window_start, window_end),
    ensures
        span_inside_window(
            window_start,
            window_end,
            restore_point(window_start, local_start),
            restore_point(window_start, local_end),
        ),
{
    restore_preserves_point_order(window_start, local_start, local_end);
    assert(window_start <= restore_point(window_start, local_start));
    assert(restore_point(window_start, local_end) <= window_end) by {
        assert(restore_point(window_start, local_end) <= restore_point(
            window_start,
            window_width(window_start, window_end),
        ));
        assert(restore_point(window_start, window_width(window_start, window_end)) == window_end);
    }
    assert(span_inside_window(
        window_start,
        window_end,
        restore_point(window_start, local_start),
        restore_point(window_start, local_end),
    ));
}

pub proof fn restoring_localized_span_round_trips(
    window_start: int,
    window_end: int,
    span_start: int,
    span_end: int,
)
    requires
        span_inside_window(window_start, window_end, span_start, span_end),
    ensures
        restore_point(window_start, localize_point(window_start, span_start)) == span_start,
        restore_point(window_start, localize_point(window_start, span_end)) == span_end,
{
    restore_localized_point_round_trips(window_start, span_start);
    restore_localized_point_round_trips(window_start, span_end);
}

} // verus!

fn main() {}
