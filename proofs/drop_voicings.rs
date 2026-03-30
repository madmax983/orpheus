use vstd::prelude::*;

verus! {

pub open spec fn drop2_3(low: int, mid: int, high: int) -> (int, int, int) {
    (mid - 12, low, high)
}

pub open spec fn drop2_4(low: int, lower_mid: int, upper_mid: int, high: int) -> (int, int, int, int) {
    (upper_mid - 12, low, lower_mid, high)
}

pub open spec fn drop3_4(low: int, lower_mid: int, upper_mid: int, high: int) -> (int, int, int, int) {
    (lower_mid - 12, low, upper_mid, high)
}

pub proof fn drop2_four_note_voicing_matches_runtime_design()
    ensures
        drop2_4(60, 64, 67, 70) == (55int, 60int, 64int, 70int),
{
}

pub proof fn drop3_four_note_voicing_matches_runtime_design()
    ensures
        drop3_4(60, 64, 67, 70) == (52int, 60int, 67int, 70int),
{
}

pub proof fn drop2_three_note_cluster_matches_runtime_design()
    ensures
        drop2_3(60, 67, 70) == (55int, 60int, 70int),
        drop2_3(64, 71, 74) == (59int, 64int, 74int),
{
}

pub proof fn drop2_preserves_cluster_size_for_four_note_voicings(
    low: int,
    lower_mid: int,
    upper_mid: int,
    high: int,
)
    ensures
        ({
            let (a, b, c, d) = drop2_4(low, lower_mid, upper_mid, high);
            &&& a == upper_mid - 12
            &&& b == low
            &&& c == lower_mid
            &&& d == high
        }),
{
}

pub proof fn transposing_before_or_after_drop2_agrees(
    low: int,
    lower_mid: int,
    upper_mid: int,
    high: int,
    offset: int,
)
    ensures
        drop2_4(low + offset, lower_mid + offset, upper_mid + offset, high + offset)
            == ({
                let (a, b, c, d) = drop2_4(low, lower_mid, upper_mid, high);
                (a + offset, b + offset, c + offset, d + offset)
            }),
{
}

} // verus!

fn main() {}
