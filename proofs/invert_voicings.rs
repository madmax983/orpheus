use vstd::prelude::*;

verus! {

pub open spec fn first_inversion2(low: int, high: int) -> (int, int) {
    (high, low + 12)
}

pub open spec fn first_inversion3(low: int, mid: int, high: int) -> (int, int, int) {
    (mid, high, low + 12)
}

pub open spec fn second_inversion3(low: int, mid: int, high: int) -> (int, int, int) {
    let (a, b, c) = first_inversion3(low, mid, high);
    first_inversion3(a, b, c)
}

pub proof fn first_inversion_major_triad_matches_runtime_design()
    ensures
        first_inversion3(60, 64, 67) == (64int, 67int, 72int),
{
}

pub proof fn second_inversion_major_triad_matches_runtime_design()
    ensures
        second_inversion3(60, 64, 67) == (67int, 72int, 76int),
{
}

pub proof fn first_inversion_dyad_matches_runtime_design()
    ensures
        first_inversion2(60, 67) == (67int, 72int),
        first_inversion2(64, 71) == (71int, 76int),
{
}

pub proof fn first_inversion_preserves_cluster_size_for_triads(low: int, mid: int, high: int)
    ensures
        ({
            let (a, b, c) = first_inversion3(low, mid, high);
            &&& true
            &&& a == mid
            &&& b == high
            &&& c == low + 12
        }),
{
}

pub proof fn transposing_before_or_after_first_inversion_agrees(
    low: int,
    mid: int,
    high: int,
    offset: int,
)
    ensures
        first_inversion3(low + offset, mid + offset, high + offset)
            == ({
                let (a, b, c) = first_inversion3(low, mid, high);
                (a + offset, b + offset, c + offset)
            }),
{
}

} // verus!

fn main() {}
