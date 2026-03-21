use vstd::prelude::*;

verus! {

pub open spec fn valid_euclid(pulses: int, steps: int) -> bool {
    steps > 0 && 0 <= pulses <= steps
}

pub open spec fn small_gap(pulses: int, steps: int) -> int {
    if pulses <= 0 {
        0
    } else {
        steps / pulses
    }
}

pub open spec fn large_gap(pulses: int, steps: int) -> int {
    if pulses <= 0 {
        0
    } else if steps % pulses == 0 {
        small_gap(pulses, steps)
    } else {
        small_gap(pulses, steps) + 1
    }
}

pub open spec fn large_gap_count(pulses: int, steps: int) -> int {
    if pulses <= 0 {
        0
    } else {
        steps % pulses
    }
}

pub open spec fn small_gap_count(pulses: int, steps: int) -> int {
    if pulses <= 0 {
        0
    } else {
        pulses - large_gap_count(pulses, steps)
    }
}

pub open spec fn gap_at(pulses: int, steps: int, index: int) -> int {
    if index < large_gap_count(pulses, steps) {
        large_gap(pulses, steps)
    } else {
        small_gap(pulses, steps)
    }
}

pub open spec fn prefix_large_count(pulses: int, steps: int, count: int) -> int {
    if count <= 0 {
        0
    } else if count < large_gap_count(pulses, steps) {
        count
    } else {
        large_gap_count(pulses, steps)
    }
}

pub open spec fn prefix_small_count(pulses: int, steps: int, count: int) -> int {
    if count <= 0 {
        0
    } else {
        count - prefix_large_count(pulses, steps, count)
    }
}

pub open spec fn prefix_gap_sum(pulses: int, steps: int, count: int) -> int {
    prefix_large_count(pulses, steps, count) * large_gap(pulses, steps)
        + prefix_small_count(pulses, steps, count) * small_gap(pulses, steps)
}

pub open spec fn onset_slot(pulses: int, steps: int, index: int) -> int {
    prefix_gap_sum(pulses, steps, index)
}

pub proof fn euclid_gap_counts_sum_to_pulses(pulses: int, steps: int)
    requires
        valid_euclid(pulses, steps),
    ensures
        small_gap_count(pulses, steps) + large_gap_count(pulses, steps) == pulses,
{
}

pub proof fn positive_pulses_have_bounded_remainder_and_positive_gaps(pulses: int, steps: int)
    requires
        valid_euclid(pulses, steps),
        pulses > 0,
    ensures
        0 <= large_gap_count(pulses, steps) < pulses,
        1 <= small_gap(pulses, steps) <= large_gap(pulses, steps),
        large_gap(pulses, steps) - small_gap(pulses, steps) <= 1,
{
    let q = small_gap(pulses, steps);
    let r = large_gap_count(pulses, steps);

    assert(q == steps / pulses);
    assert(r == steps % pulses);
    assert(0 <= r < pulses);
    assert(steps == pulses * q + r) by (nonlinear_arith)
        requires
            q == steps / pulses,
            r == steps % pulses,
            pulses > 0,
    {
    }

    if q == 0 {
        assert(steps < pulses) by (nonlinear_arith)
            requires
                steps == pulses * q + r,
                q == 0,
                0 <= r < pulses,
        {
        }
        assert(false);
    }

    assert(1 <= q);

    if r == 0 {
        assert(large_gap(pulses, steps) == q);
    } else {
        assert(large_gap(pulses, steps) == q + 1);
    }

    assert(q <= large_gap(pulses, steps));
    assert(large_gap(pulses, steps) - q <= 1);
}

pub proof fn euclid_gap_total_span_matches_steps(pulses: int, steps: int)
    requires
        valid_euclid(pulses, steps),
        pulses > 0,
    ensures
        small_gap_count(pulses, steps) * small_gap(pulses, steps)
            + large_gap_count(pulses, steps) * large_gap(pulses, steps) == steps,
{
    positive_pulses_have_bounded_remainder_and_positive_gaps(pulses, steps);

    let q = small_gap(pulses, steps);
    let r = large_gap_count(pulses, steps);

    assert(q == steps / pulses);
    assert(r == steps % pulses);
    assert(steps == pulses * q + r) by (nonlinear_arith)
        requires
            q == steps / pulses,
            r == steps % pulses,
            pulses > 0,
    {
    }

    if r == 0 {
        assert(large_gap(pulses, steps) == q);
        assert(small_gap_count(pulses, steps) == pulses);
        assert(large_gap_count(pulses, steps) == 0);
        assert(
            small_gap_count(pulses, steps) * small_gap(pulses, steps)
                + large_gap_count(pulses, steps) * large_gap(pulses, steps) == steps
        ) by (nonlinear_arith)
            requires
                large_gap(pulses, steps) == q,
                small_gap_count(pulses, steps) == pulses,
                large_gap_count(pulses, steps) == 0,
                small_gap(pulses, steps) == q,
                steps == pulses * q + r,
                r == 0,
        {
        }
    } else {
        assert(large_gap(pulses, steps) == q + 1);
        assert(small_gap_count(pulses, steps) == pulses - r);
        assert(
            small_gap_count(pulses, steps) * small_gap(pulses, steps)
                + large_gap_count(pulses, steps) * large_gap(pulses, steps) == steps
        ) by (nonlinear_arith)
            requires
                large_gap(pulses, steps) == q + 1,
                small_gap_count(pulses, steps) == pulses - r,
                large_gap_count(pulses, steps) == r,
                small_gap(pulses, steps) == q,
                steps == pulses * q + r,
        {
        }
    }
}

pub proof fn gap_at_is_balanced_and_positive(pulses: int, steps: int, index: int)
    requires
        valid_euclid(pulses, steps),
        pulses > 0,
        0 <= index < pulses,
    ensures
        gap_at(pulses, steps, index) == small_gap(pulses, steps)
            || gap_at(pulses, steps, index) == large_gap(pulses, steps),
        gap_at(pulses, steps, index) > 0,
{
    positive_pulses_have_bounded_remainder_and_positive_gaps(pulses, steps);

    if index < large_gap_count(pulses, steps) {
        assert(gap_at(pulses, steps, index) == large_gap(pulses, steps));
    } else {
        assert(gap_at(pulses, steps, index) == small_gap(pulses, steps));
    }
}

pub proof fn prefix_counts_are_bounded(pulses: int, steps: int, count: int)
    requires
        valid_euclid(pulses, steps),
        pulses > 0,
        0 <= count <= pulses,
    ensures
        0 <= prefix_large_count(pulses, steps, count) <= large_gap_count(pulses, steps),
        0 <= prefix_small_count(pulses, steps, count) <= small_gap_count(pulses, steps),
        prefix_large_count(pulses, steps, count) + prefix_small_count(pulses, steps, count)
            == count,
{
    positive_pulses_have_bounded_remainder_and_positive_gaps(pulses, steps);
    euclid_gap_counts_sum_to_pulses(pulses, steps);

    if count <= 0 {
        assert(prefix_large_count(pulses, steps, count) == 0);
        assert(prefix_small_count(pulses, steps, count) == 0);
    } else if count < large_gap_count(pulses, steps) {
        assert(prefix_large_count(pulses, steps, count) == count);
        assert(prefix_small_count(pulses, steps, count) == 0);
    } else {
        assert(prefix_large_count(pulses, steps, count) == large_gap_count(pulses, steps));
        assert(prefix_small_count(pulses, steps, count) == count - large_gap_count(pulses, steps));
        assert(prefix_small_count(pulses, steps, count) <= pulses - large_gap_count(pulses, steps))
            by (nonlinear_arith)
            requires
                count <= pulses,
                prefix_small_count(pulses, steps, count) == count - large_gap_count(pulses, steps),
        {
        }
        assert(small_gap_count(pulses, steps) == pulses - large_gap_count(pulses, steps));
    }
}

pub proof fn prefix_gap_sum_steps_by_one_gap(pulses: int, steps: int, index: int)
    requires
        valid_euclid(pulses, steps),
        pulses > 0,
        0 <= index < pulses,
    ensures
        prefix_gap_sum(pulses, steps, index + 1)
            == prefix_gap_sum(pulses, steps, index) + gap_at(pulses, steps, index),
{
    positive_pulses_have_bounded_remainder_and_positive_gaps(pulses, steps);

    if index < large_gap_count(pulses, steps) {
        assert(prefix_large_count(pulses, steps, index) == index);
        assert(prefix_large_count(pulses, steps, index + 1) == index + 1);
        assert(prefix_small_count(pulses, steps, index) == 0);
        assert(prefix_small_count(pulses, steps, index + 1) == 0);
        assert(gap_at(pulses, steps, index) == large_gap(pulses, steps));
        assert(
            prefix_gap_sum(pulses, steps, index + 1)
                == prefix_gap_sum(pulses, steps, index) + gap_at(pulses, steps, index)
        ) by (nonlinear_arith)
            requires
                prefix_large_count(pulses, steps, index) == index,
                prefix_large_count(pulses, steps, index + 1) == index + 1,
                prefix_small_count(pulses, steps, index) == 0,
                prefix_small_count(pulses, steps, index + 1) == 0,
                gap_at(pulses, steps, index) == large_gap(pulses, steps),
        {
        }
    } else {
        assert(prefix_large_count(pulses, steps, index) == large_gap_count(pulses, steps));
        assert(prefix_large_count(pulses, steps, index + 1) == large_gap_count(pulses, steps));
        assert(prefix_small_count(pulses, steps, index + 1) == prefix_small_count(pulses, steps, index) + 1);
        assert(gap_at(pulses, steps, index) == small_gap(pulses, steps));
        assert(
            prefix_gap_sum(pulses, steps, index + 1)
                == prefix_gap_sum(pulses, steps, index) + gap_at(pulses, steps, index)
        ) by (nonlinear_arith)
            requires
                prefix_large_count(pulses, steps, index) == large_gap_count(pulses, steps),
                prefix_large_count(pulses, steps, index + 1)
                    == large_gap_count(pulses, steps),
                prefix_small_count(pulses, steps, index + 1)
                    == prefix_small_count(pulses, steps, index) + 1,
                gap_at(pulses, steps, index) == small_gap(pulses, steps),
        {
        }
    }
}

pub proof fn onset_slot_is_in_range(pulses: int, steps: int, index: int)
    requires
        valid_euclid(pulses, steps),
        pulses > 0,
        0 <= index < pulses,
    ensures
        0 <= onset_slot(pulses, steps, index) < steps,
{
    positive_pulses_have_bounded_remainder_and_positive_gaps(pulses, steps);
    euclid_gap_total_span_matches_steps(pulses, steps);
    prefix_counts_are_bounded(pulses, steps, index);

    let prefix_large = prefix_large_count(pulses, steps, index);
    let prefix_small = prefix_small_count(pulses, steps, index);
    let remaining_large = large_gap_count(pulses, steps) - prefix_large;
    let remaining_small = small_gap_count(pulses, steps) - prefix_small;

    assert(0 <= onset_slot(pulses, steps, index));
    assert(0 <= remaining_large);
    assert(0 <= remaining_small);
    assert(remaining_large + remaining_small == pulses - index) by (nonlinear_arith)
        requires
            small_gap_count(pulses, steps) + large_gap_count(pulses, steps) == pulses,
            prefix_large + prefix_small == index,
            remaining_large == large_gap_count(pulses, steps) - prefix_large,
            remaining_small == small_gap_count(pulses, steps) - prefix_small,
    {
    }
    assert(0 < remaining_large + remaining_small) by (nonlinear_arith)
        requires
            remaining_large + remaining_small == pulses - index,
            index < pulses,
    {
    }

    assert(
        steps
            == onset_slot(pulses, steps, index)
                + remaining_large * large_gap(pulses, steps)
                + remaining_small * small_gap(pulses, steps)
    ) by (nonlinear_arith)
        requires
            small_gap_count(pulses, steps) * small_gap(pulses, steps)
                + large_gap_count(pulses, steps) * large_gap(pulses, steps) == steps,
            onset_slot(pulses, steps, index)
                == prefix_large * large_gap(pulses, steps)
                    + prefix_small * small_gap(pulses, steps),
            remaining_large == large_gap_count(pulses, steps) - prefix_large,
            remaining_small == small_gap_count(pulses, steps) - prefix_small,
    {
    }

    if remaining_large > 0 {
        assert(0 < remaining_large * large_gap(pulses, steps)) by (nonlinear_arith)
            requires
                remaining_large > 0,
                large_gap(pulses, steps) > 0,
        {
        }
        assert(
            0 < remaining_large * large_gap(pulses, steps)
                + remaining_small * small_gap(pulses, steps)
        ) by (nonlinear_arith)
            requires
                0 < remaining_large * large_gap(pulses, steps),
                0 <= remaining_small,
                0 < small_gap(pulses, steps),
        {
        }
    } else {
        assert(remaining_small > 0) by (nonlinear_arith)
            requires
                remaining_large + remaining_small > 0,
                remaining_large == 0,
                0 <= remaining_small,
        {
        }
        assert(0 < remaining_small * small_gap(pulses, steps)) by (nonlinear_arith)
            requires
                remaining_small > 0,
                small_gap(pulses, steps) > 0,
        {
        }
        assert(
            0 < remaining_large * large_gap(pulses, steps)
                + remaining_small * small_gap(pulses, steps)
        ) by (nonlinear_arith)
            requires
                remaining_large == 0,
                0 < remaining_small * small_gap(pulses, steps),
        {
        }
    }

    assert(onset_slot(pulses, steps, index) < steps) by (nonlinear_arith)
        requires
            steps
                == onset_slot(pulses, steps, index)
                    + remaining_large * large_gap(pulses, steps)
                    + remaining_small * small_gap(pulses, steps),
            0 < remaining_large * large_gap(pulses, steps)
                + remaining_small * small_gap(pulses, steps),
    {
    }
}

pub proof fn onset_slots_are_strictly_increasing(pulses: int, steps: int, index: int)
    requires
        valid_euclid(pulses, steps),
        pulses > 0,
        0 <= index < pulses - 1,
    ensures
        onset_slot(pulses, steps, index) < onset_slot(pulses, steps, index + 1),
{
    prefix_gap_sum_steps_by_one_gap(pulses, steps, index);
    gap_at_is_balanced_and_positive(pulses, steps, index);
    assert(onset_slot(pulses, steps, index + 1) == onset_slot(pulses, steps, index) + gap_at(pulses, steps, index));
    assert(onset_slot(pulses, steps, index) < onset_slot(pulses, steps, index + 1)) by (nonlinear_arith)
        requires
            onset_slot(pulses, steps, index + 1)
                == onset_slot(pulses, steps, index) + gap_at(pulses, steps, index),
            gap_at(pulses, steps, index) > 0,
    {
    }
}

pub proof fn any_two_euclid_gaps_differ_by_at_most_one(
    pulses: int,
    steps: int,
    left: int,
    right: int,
)
    requires
        valid_euclid(pulses, steps),
        pulses > 0,
        0 <= left < pulses,
        0 <= right < pulses,
    ensures
        gap_at(pulses, steps, left) - gap_at(pulses, steps, right) <= 1,
        gap_at(pulses, steps, right) - gap_at(pulses, steps, left) <= 1,
{
    gap_at_is_balanced_and_positive(pulses, steps, left);
    gap_at_is_balanced_and_positive(pulses, steps, right);
    positive_pulses_have_bounded_remainder_and_positive_gaps(pulses, steps);

    if gap_at(pulses, steps, left) == gap_at(pulses, steps, right) {
        assert(gap_at(pulses, steps, left) - gap_at(pulses, steps, right) == 0);
        assert(gap_at(pulses, steps, right) - gap_at(pulses, steps, left) == 0);
    } else if gap_at(pulses, steps, left) == large_gap(pulses, steps) {
        assert(gap_at(pulses, steps, right) == small_gap(pulses, steps));
        assert(gap_at(pulses, steps, left) - gap_at(pulses, steps, right) <= 1);
        assert(gap_at(pulses, steps, right) - gap_at(pulses, steps, left) <= 0);
    } else {
        assert(gap_at(pulses, steps, left) == small_gap(pulses, steps));
        assert(gap_at(pulses, steps, right) == large_gap(pulses, steps));
        assert(gap_at(pulses, steps, right) - gap_at(pulses, steps, left) <= 1);
        assert(gap_at(pulses, steps, left) - gap_at(pulses, steps, right) <= 0);
    }
}

} // verus!

fn main() {}
