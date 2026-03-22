use vstd::prelude::*;

verus! {

// Proof scope:
// - model the arp timing layer as a normalized span split into equal slots
// - model traversal as modulo wrapping over a finite pitch cluster
// - keep the arithmetic self-contained; runtime rationals are intentionally
//   out of scope for this first spine

pub open spec fn valid_arp_steps(steps: int) -> bool {
    steps > 0
}

pub open spec fn valid_cluster_size(cluster_size: int) -> bool {
    cluster_size > 0
}

pub open spec fn arp_slot_start(origin: int, index: int) -> int {
    origin + index
}

pub open spec fn arp_slot_end(origin: int, index: int) -> int {
    origin + index + 1
}

pub open spec fn arp_valid_slot(index: int, steps: int) -> bool {
    0 <= index < steps
}

pub open spec fn arp_up_index(step: int, cluster_size: int) -> int {
    step % cluster_size
}

pub open spec fn arp_down_index(step: int, cluster_size: int) -> int {
    cluster_size - 1 - (step % cluster_size)
}

pub proof fn valid_arp_steps_are_positive(steps: int)
    requires
        valid_arp_steps(steps),
    ensures
        steps > 0,
{
}

pub proof fn arp_equal_slots_cover_the_normalized_span(origin: int, steps: int)
    requires
        valid_arp_steps(steps),
    ensures
        arp_slot_start(origin, 0) == origin,
        arp_slot_end(origin, steps - 1) == origin + steps,
        forall|i: int| #![auto] 0 <= i && i + 1 < steps ==> arp_slot_end(origin, i) == arp_slot_start(origin, i + 1),
        forall|i: int| #![auto] arp_valid_slot(i, steps) ==> arp_slot_start(origin, i) < arp_slot_end(origin, i),
{
    valid_arp_steps_are_positive(steps);

    assert(arp_slot_start(origin, 0) == origin);
    assert(arp_slot_end(origin, steps - 1) == origin + steps) by (nonlinear_arith)
        requires
            steps > 0,
    {
    }

    assert forall|i: int| #![auto] 0 <= i && i + 1 < steps implies arp_slot_end(origin, i) == arp_slot_start(origin, i + 1) by {
        assert(arp_slot_end(origin, i) == origin + i + 1);
        assert(arp_slot_start(origin, i + 1) == origin + i + 1);
    }

    assert forall|i: int| #![auto] arp_valid_slot(i, steps) implies arp_slot_start(origin, i) < arp_slot_end(origin, i) by {
        assert(arp_slot_start(origin, i) == origin + i);
        assert(arp_slot_end(origin, i) == origin + i + 1);
    }
}

pub proof fn arp_up_index_stays_in_range(step: int, cluster_size: int)
    requires
        valid_cluster_size(cluster_size),
        0 <= step,
    ensures
        0 <= arp_up_index(step, cluster_size) < cluster_size,
{
    assert(0 <= step % cluster_size < cluster_size);
}

pub proof fn arp_down_index_stays_in_range(step: int, cluster_size: int)
    requires
        valid_cluster_size(cluster_size),
        0 <= step,
    ensures
        0 <= arp_down_index(step, cluster_size) < cluster_size,
{
    assert(0 <= step % cluster_size < cluster_size);
    assert(0 <= cluster_size - 1 - (step % cluster_size)) by (nonlinear_arith)
        requires
            cluster_size > 0,
            0 <= step % cluster_size,
            step % cluster_size < cluster_size,
    {
    }
    assert(cluster_size - 1 - (step % cluster_size) < cluster_size) by (nonlinear_arith)
        requires
            cluster_size > 0,
            0 <= step % cluster_size,
            step % cluster_size < cluster_size,
    {
    }
}

pub proof fn arp_up_wraps_every_cluster_size(step: int, cluster_size: int)
    requires
        valid_cluster_size(cluster_size),
        0 <= step,
    ensures
        arp_up_index(step + cluster_size, cluster_size) == arp_up_index(step, cluster_size),
{
    assert((step + cluster_size) % cluster_size == step % cluster_size) by (nonlinear_arith)
        requires
            cluster_size > 0,
            0 <= step,
    {
    }
}

pub proof fn arp_down_wraps_every_cluster_size(step: int, cluster_size: int)
    requires
        valid_cluster_size(cluster_size),
        0 <= step,
    ensures
        arp_down_index(step + cluster_size, cluster_size) == arp_down_index(step, cluster_size),
{
    arp_up_wraps_every_cluster_size(step, cluster_size);
    assert(arp_down_index(step + cluster_size, cluster_size) == cluster_size - 1 - ((step + cluster_size) % cluster_size));
    assert(arp_down_index(step, cluster_size) == cluster_size - 1 - (step % cluster_size));
}

pub proof fn arp_up_and_down_indices_are_wraps(step: int, cluster_size: int)
    requires
        valid_cluster_size(cluster_size),
        0 <= step,
    ensures
        arp_down_index(step, cluster_size) == cluster_size - 1 - arp_up_index(step, cluster_size),
{
    assert(arp_up_index(step, cluster_size) == step % cluster_size);
    assert(arp_down_index(step, cluster_size) == cluster_size - 1 - (step % cluster_size));
}

pub proof fn arp_three_note_cluster_five_step_example()
    ensures
        arp_up_index(0, 3) == 0,
        arp_up_index(1, 3) == 1,
        arp_up_index(2, 3) == 2,
        arp_up_index(3, 3) == 0,
        arp_up_index(4, 3) == 1,
        arp_down_index(0, 3) == 2,
        arp_down_index(1, 3) == 1,
        arp_down_index(2, 3) == 0,
        arp_down_index(3, 3) == 2,
        arp_down_index(4, 3) == 1,
{
}

pub proof fn arp_partition_example_five_steps()
    ensures
        arp_slot_start(0, 0) == 0,
        arp_slot_end(0, 4) == 5,
        arp_slot_end(0, 0) == arp_slot_start(0, 1),
        arp_slot_end(0, 1) == arp_slot_start(0, 2),
        arp_slot_end(0, 2) == arp_slot_start(0, 3),
        arp_slot_end(0, 3) == arp_slot_start(0, 4),
{
}

} // verus!

fn main() {}
