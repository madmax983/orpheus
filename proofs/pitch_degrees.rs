use vstd::prelude::*;

verus! {

pub open spec fn aeolian_interval(index: int) -> int {
    if index == 0 {
        0
    } else if index == 1 {
        2
    } else if index == 2 {
        3
    } else if index == 3 {
        5
    } else if index == 4 {
        7
    } else if index == 5 {
        8
    } else {
        10
    }
}

pub open spec fn mapped_aeolian_degree(degree: int) -> int
    decreases if degree >= 0 { degree } else { -degree }
{
    if 0 <= degree && degree < 7 {
        aeolian_interval(degree)
    } else if -7 < degree && degree < 0 {
        aeolian_interval(degree + 7) - 12
    } else if degree >= 7 {
        12 + mapped_aeolian_degree(degree - 7)
    } else {
        mapped_aeolian_degree(degree + 7) - 12
    }
}

pub open spec fn transpose_semitones(value: int, offset: int) -> int {
    value + offset
}

pub proof fn aeolian_base_octave_intervals_are_ordered()
    ensures
        aeolian_interval(0) == 0,
        aeolian_interval(1) == 2,
        aeolian_interval(2) == 3,
        aeolian_interval(3) == 5,
        aeolian_interval(4) == 7,
        aeolian_interval(5) == 8,
        aeolian_interval(6) == 10,
        aeolian_interval(0) < aeolian_interval(1),
        aeolian_interval(1) < aeolian_interval(2),
        aeolian_interval(2) < aeolian_interval(3),
        aeolian_interval(3) < aeolian_interval(4),
        aeolian_interval(4) < aeolian_interval(5),
        aeolian_interval(5) < aeolian_interval(6),
        aeolian_interval(6) < 12,
{
}

pub proof fn positive_degrees_carry_upward()
    ensures
        mapped_aeolian_degree(7) == 12,
        mapped_aeolian_degree(8) == 14,
{
    assert(mapped_aeolian_degree(7) == 12 + mapped_aeolian_degree(0));
    assert(mapped_aeolian_degree(0) == 0);
    assert(mapped_aeolian_degree(8) == 12 + mapped_aeolian_degree(1));
    assert(mapped_aeolian_degree(1) == 2);
}

pub proof fn negative_degrees_wrap_downward()
    ensures
        mapped_aeolian_degree(-2) == -4,
        mapped_aeolian_degree(-1) == -2,
{
    assert(mapped_aeolian_degree(-2) == mapped_aeolian_degree(5) - 12);
    assert(mapped_aeolian_degree(5) == 8);
    assert(mapped_aeolian_degree(-1) == mapped_aeolian_degree(6) - 12);
    assert(mapped_aeolian_degree(6) == 10);
}

pub proof fn mapped_degree_examples_match_runtime_design()
    ensures
        mapped_aeolian_degree(0) == 0,
        mapped_aeolian_degree(2) == 3,
        mapped_aeolian_degree(4) == 7,
        mapped_aeolian_degree(7) == 12,
        mapped_aeolian_degree(8) == 14,
{
    positive_degrees_carry_upward();
}

pub proof fn transpose_preserves_interval_difference(left: int, right: int, offset: int)
    ensures
        transpose_semitones(left, offset) - transpose_semitones(right, offset) == left - right,
{
}

} // verus!

fn main() {}
