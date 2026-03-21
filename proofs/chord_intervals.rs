use vstd::prelude::*;

verus! {

pub open spec fn chord_tone(root: int, interval: int) -> int {
    root + interval
}

pub proof fn major_triad_examples_match_runtime_design()
    ensures
        chord_tone(60, 0) == 60,
        chord_tone(60, 4) == 64,
        chord_tone(60, 7) == 67,
{
}

pub proof fn minor_seventh_examples_match_runtime_design()
    ensures
        chord_tone(60, 0) == 60,
        chord_tone(60, 3) == 63,
        chord_tone(60, 7) == 67,
        chord_tone(60, 10) == 70,
{
}

pub proof fn degree_derived_root_examples_match_runtime_design()
    ensures
        chord_tone(63, 0) == 63,
        chord_tone(63, 3) == 66,
        chord_tone(63, 7) == 70,
{
}

pub proof fn chord_tones_preserve_interval_difference(root: int, left: int, right: int)
    ensures
        chord_tone(root, left) - chord_tone(root, right) == left - right,
{
}

pub proof fn transposing_the_root_transposes_each_chord_tone(root: int, interval: int, offset: int)
    ensures
        chord_tone(root + offset, interval) == chord_tone(root, interval) + offset,
{
}

} // verus!

fn main() {}
