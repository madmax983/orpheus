use vstd::prelude::*;

verus! {

pub open spec fn note_pitch_class(note: int) -> int {
    if note == 0 {
        0
    } else if note == 1 {
        2
    } else if note == 2 {
        4
    } else if note == 3 {
        5
    } else if note == 4 {
        7
    } else if note == 5 {
        9
    } else {
        11
    }
}

pub open spec fn accidental_offset(accidental: int) -> int {
    if accidental == -1 {
        -1
    } else if accidental == 1 {
        1
    } else {
        0
    }
}

pub open spec fn absolute_pitch(note: int, accidental: int, octave: int) -> int {
    12 * (octave + 1) + note_pitch_class(note) + accidental_offset(accidental)
}

pub proof fn natural_pitch_classes_are_ordered()
    ensures
        note_pitch_class(0) == 0,
        note_pitch_class(1) == 2,
        note_pitch_class(2) == 4,
        note_pitch_class(3) == 5,
        note_pitch_class(4) == 7,
        note_pitch_class(5) == 9,
        note_pitch_class(6) == 11,
        note_pitch_class(0) < note_pitch_class(1),
        note_pitch_class(1) < note_pitch_class(2),
        note_pitch_class(2) < note_pitch_class(3),
        note_pitch_class(3) < note_pitch_class(4),
        note_pitch_class(4) < note_pitch_class(5),
        note_pitch_class(5) < note_pitch_class(6),
        note_pitch_class(6) < 12,
{
}

pub proof fn accidental_offsets_match_runtime_rules()
    ensures
        accidental_offset(-1) == -1,
        accidental_offset(0) == 0,
        accidental_offset(1) == 1,
{
}

pub proof fn named_pitch_examples_match_runtime_design()
    ensures
        absolute_pitch(0, 0, 4) == 60,
        absolute_pitch(5, 0, 4) == 69,
        absolute_pitch(6, -1, 3) == 58,
        absolute_pitch(3, 1, 4) == 66,
{
    accidental_offsets_match_runtime_rules();
}

pub proof fn transposing_named_pitches_preserves_interval_difference(
    left: int,
    right: int,
    offset: int,
)
    ensures
        (left + offset) - (right + offset) == left - right,
{
}

} // verus!

fn main() {}
