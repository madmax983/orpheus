use vstd::prelude::*;

verus! {

pub open spec fn valid_partition(total_width: int, count: int) -> bool {
    0 < total_width && 0 < count
}

pub open spec fn partition_step(total_width: int, count: int) -> int
    recommends
        valid_partition(total_width, count),
{
    total_width / count
}

pub open spec fn valid_index(index: int, count: int) -> bool {
    0 <= index < count
}

pub open spec fn part_start(origin: int, step: int, index: int) -> int {
    origin + index * step
}

pub open spec fn part_end(origin: int, step: int, index: int) -> int {
    origin + (index + 1) * step
}

pub open spec fn partitioned_spans(origin: int, step: int, count: int) -> Seq<(int, int)>
    recommends
        0 <= count,
{
    Seq::new(
        count as nat,
        |index: int| (part_start(origin, step, index), part_end(origin, step, index)),
    )
}

pub proof fn equal_partitions_cover_exact_total_width(origin: int, step: int, count: int)
    requires
        0 < step,
        0 < count,
    ensures
        part_start(origin, step, 0) == origin,
        part_end(origin, step, count - 1) == origin + count * step,
{
}

pub proof fn adjacent_partitions_touch_without_gaps(origin: int, step: int, count: int, index: int)
    requires
        0 < step,
        0 < count,
        0 <= index < count - 1,
    ensures
        part_end(origin, step, index) == part_start(origin, step, index + 1),
{
}

pub proof fn partition_starts_are_monotonic(origin: int, step: int, left: int, right: int)
    requires
        0 < step,
        left <= right,
    ensures
        part_start(origin, step, left) <= part_start(origin, step, right),
    decreases right - left,
{
    if left < right {
        partition_starts_are_monotonic(origin, step, left + 1, right);
        assert(part_start(origin, step, left + 1) == part_start(origin, step, left) + step) by (nonlinear_arith)
        {
        }
        assert(part_start(origin, step, left) < part_start(origin, step, left + 1));
    }
}

pub proof fn later_partitions_do_not_overlap(origin: int, step: int, count: int, left: int, right: int)
    requires
        0 < step,
        0 < count,
        valid_index(left, count),
        valid_index(right, count),
        left < right,
    ensures
        part_end(origin, step, left) <= part_start(origin, step, right),
{
    if left + 1 == right {
        adjacent_partitions_touch_without_gaps(origin, step, count, left);
    } else {
        assert(left + 1 <= right);
        assert(part_end(origin, step, left) <= part_start(origin, step, left + 1)) by {
            adjacent_partitions_touch_without_gaps(origin, step, count, left);
        }
        partition_starts_are_monotonic(origin, step, left + 1, right);
        assert(part_start(origin, step, left + 1) <= part_start(origin, step, right));
    }
}

pub proof fn partitioned_span_sequence_preserves_count(origin: int, step: int, count: int)
    requires
        0 < step,
        0 < count,
    ensures
        partitioned_spans(origin, step, count).len() == count,
        partitioned_spans(origin, step, count)[0] == (part_start(origin, step, 0), part_end(origin, step, 0)),
        partitioned_spans(origin, step, count)[count - 1]
            == (part_start(origin, step, count - 1), part_end(origin, step, count - 1)),
{
}

pub proof fn equal_partition_runtime_shape_example()
    ensures
        part_start(0, 1, 0) == 0,
        part_end(0, 1, 0) == 1,
        part_start(0, 1, 1) == 1,
        part_end(0, 1, 1) == 2,
        part_start(0, 1, 2) == 2,
        part_end(0, 1, 2) == 3,
{
}

} // verus!

fn main() {}
