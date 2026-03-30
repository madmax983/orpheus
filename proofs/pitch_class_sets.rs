use vstd::prelude::*;

verus! {

pub open spec fn ordered_pitch_class_pair(values: Seq<int>, i: int, j: int) -> bool {
    values[i] < values[j]
}

pub open spec fn valid_pitch_class_set(values: Seq<int>) -> bool {
    &&& values.len() > 0
    &&& values[0] == 0
    &&& forall|i: int| 0 <= i < values.len() ==> {
        &&& 0 <= #[trigger] values[i]
        &&& values[i] < 12
    }
    &&& forall|i: int, j: int|
        0 <= i < j < values.len() ==> #[trigger] ordered_pitch_class_pair(values, i, j)
}

pub proof fn valid_pitch_class_sets_are_rooted_and_in_range(values: Seq<int>)
    requires
        valid_pitch_class_set(values),
    ensures
        values.len() > 0,
        values[0] == 0,
        forall|i: int| 0 <= i < values.len() ==> {
            &&& 0 <= #[trigger] values[i]
            &&& values[i] < 12
        },
{
}

pub proof fn valid_pitch_class_sets_are_strictly_increasing(values: Seq<int>, i: int, j: int)
    requires
        valid_pitch_class_set(values),
        0 <= i < j < values.len(),
    ensures
        values[i] < values[j],
        values[i] != values[j],
{
    assert(ordered_pitch_class_pair(values, i, j));
}

pub proof fn valid_pitch_class_sets_have_unique_entries(values: Seq<int>, i: int, j: int)
    requires
        valid_pitch_class_set(values),
        0 <= i < values.len(),
        0 <= j < values.len(),
        i != j,
    ensures
        values[i] != values[j],
{
    if i < j {
        valid_pitch_class_sets_are_strictly_increasing(values, i, j);
    } else {
        valid_pitch_class_sets_are_strictly_increasing(values, j, i);
    }
}

pub proof fn pitch_class_set_examples_match_runtime_rules()
    ensures
        valid_pitch_class_set(seq![0int, 2int, 3int, 7int, 8int]),
        !valid_pitch_class_set(seq![2int, 3int, 7int, 8int]),
        !valid_pitch_class_set(seq![0int, 3int, 3int, 7int]),
        !valid_pitch_class_set(seq![0int, 7int, 3int]),
        !valid_pitch_class_set(seq![0int, 2int, 12int]),
{
    assert(!valid_pitch_class_set(seq![2int, 3int, 7int, 8int]));
    assert(!valid_pitch_class_set(seq![0int, 3int, 3int, 7int])) by {
        assert(!ordered_pitch_class_pair(seq![0int, 3int, 3int, 7int], 1, 2));
    };
    assert(!valid_pitch_class_set(seq![0int, 7int, 3int])) by {
        assert(!ordered_pitch_class_pair(seq![0int, 7int, 3int], 1, 2));
    };
    assert(!valid_pitch_class_set(seq![0int, 2int, 12int])) by {
        assert(!(0 <= seq![0int, 2int, 12int][2] < 12));
    };
}

} // verus!

fn main() {}
