use vstd::prelude::*;

verus! {

// Current proof scope is the ordered-span invariant enforced by the private
// `TimeSpan` constructors. Refinement from runtime `Rational` normalization and
// checked arithmetic into this integer model is deferred to a later proof.

pub open spec fn valid_span(start: int, end: int) -> bool {
    start <= end
}

pub proof fn unit_span_is_valid()
    ensures
        valid_span(0, 1),
{
}

pub proof fn zero_length_span_is_valid(point: int)
    ensures
        valid_span(point, point),
{
}

pub proof fn span_split_preserves_bounds(start: int, mid: int, end: int)
    requires
        start <= mid,
        mid <= end,
    ensures
        valid_span(start, mid) && valid_span(mid, end),
{
}

} // verus!

fn main() {}
