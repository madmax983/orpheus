use vstd::prelude::*;

verus! {

// Proof scope:
// - model roll as a normalized span split into equal slots
// - keep the arithmetic self-contained and slot-based
// - prove the replay law: cluster size * steps emitted events

pub open spec fn valid_roll_steps(steps: int) -> bool {
    steps > 0
}

pub open spec fn valid_cluster_size(cluster_size: int) -> bool {
    cluster_size > 0
}

pub open spec fn roll_slot_start(origin: int, index: int) -> int {
    origin + index
}

pub open spec fn roll_slot_end(origin: int, index: int) -> int {
    origin + index + 1
}

pub open spec fn roll_valid_slot(index: int, steps: int) -> bool {
    0 <= index < steps
}

pub open spec fn roll_partitioned_slots(origin: int, steps: int) -> Seq<(int, int)>
    recommends
        0 <= steps,
{
    Seq::new(
        steps as nat,
        |index: int| (roll_slot_start(origin, index), roll_slot_end(origin, index)),
    )
}

pub open spec fn roll_replicated_event_count(cluster_size: int, steps: int) -> int {
    cluster_size * steps
}

pub proof fn valid_roll_steps_are_positive(steps: int)
    requires
        valid_roll_steps(steps),
    ensures
        steps > 0,
{
}

pub proof fn roll_equal_slots_cover_the_normalized_span(origin: int, steps: int)
    requires
        valid_roll_steps(steps),
    ensures
        roll_slot_start(origin, 0) == origin,
        roll_slot_end(origin, steps - 1) == origin + steps,
        forall|i: int| #![auto] 0 <= i && i + 1 < steps ==> roll_slot_end(origin, i) == roll_slot_start(origin, i + 1),
        forall|i: int| #![auto] roll_valid_slot(i, steps) ==> roll_slot_start(origin, i) < roll_slot_end(origin, i),
{
    valid_roll_steps_are_positive(steps);

    assert(roll_slot_start(origin, 0) == origin);
    assert(roll_slot_end(origin, steps - 1) == origin + steps) by (nonlinear_arith)
        requires
            steps > 0,
    {
    }

    assert forall|i: int| #![auto] 0 <= i && i + 1 < steps implies roll_slot_end(origin, i) == roll_slot_start(origin, i + 1) by {
        assert(roll_slot_end(origin, i) == origin + i + 1);
        assert(roll_slot_start(origin, i + 1) == origin + i + 1);
    }

    assert forall|i: int| #![auto] roll_valid_slot(i, steps) implies roll_slot_start(origin, i) < roll_slot_end(origin, i) by {
        assert(roll_slot_start(origin, i) == origin + i);
        assert(roll_slot_end(origin, i) == origin + i + 1);
    }
}

pub proof fn roll_slot_width_is_positive(origin: int, steps: int, index: int)
    requires
        valid_roll_steps(steps),
        roll_valid_slot(index, steps),
    ensures
        roll_slot_end(origin, index) - roll_slot_start(origin, index) == 1,
        roll_slot_start(origin, index) < roll_slot_end(origin, index),
{
    assert(roll_slot_start(origin, index) == origin + index);
    assert(roll_slot_end(origin, index) == origin + index + 1);
}

pub proof fn roll_partition_sequence_has_expected_length(origin: int, steps: int)
    requires
        valid_roll_steps(steps),
    ensures
        roll_partitioned_slots(origin, steps).len() == steps,
        forall|i: int| #![auto] roll_valid_slot(i, steps) ==> roll_partitioned_slots(origin, steps)[i]
            == (roll_slot_start(origin, i), roll_slot_end(origin, i)),
{
}

pub proof fn roll_replicated_cluster_event_count_is_multiplicative(cluster_size: int, steps: int)
    requires
        valid_cluster_size(cluster_size),
        valid_roll_steps(steps),
    ensures
        roll_replicated_event_count(cluster_size, steps) == cluster_size * steps,
{
}

pub proof fn roll_replicates_cluster_events_by_step_count(cluster_size: int, steps: int)
    requires
        valid_cluster_size(cluster_size),
        valid_roll_steps(steps),
    ensures
        roll_replicated_event_count(cluster_size, steps) == cluster_size * steps,
{
    roll_replicated_cluster_event_count_is_multiplicative(cluster_size, steps);
}

pub proof fn roll_three_note_four_step_example()
    ensures
        roll_slot_start(0, 0) == 0,
        roll_slot_end(0, 3) == 4,
        roll_slot_end(0, 0) == roll_slot_start(0, 1),
        roll_slot_end(0, 1) == roll_slot_start(0, 2),
        roll_slot_end(0, 2) == roll_slot_start(0, 3),
        roll_replicated_event_count(3, 4) == 12,
{
}

} // verus!

fn main() {}
