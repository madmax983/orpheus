use vstd::prelude::*;

verus! {

pub open spec fn valid_span(start: int, end: int) -> bool {
    start <= end
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
