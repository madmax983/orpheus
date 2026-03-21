use vstd::prelude::*;

verus! {

pub open spec fn valid_when(period: int, offset: int) -> bool {
    period > 0 && 0 <= offset < period
}

pub open spec fn cycle_in_query(query_start: int, query_end: int, cycle: int) -> bool {
    query_start <= cycle && cycle < query_end
}

pub open spec fn matching_cycle(period: int, offset: int, step: int) -> int {
    offset + step * period
}

pub open spec fn matches_when(period: int, offset: int, cycle: int) -> bool {
    exists|step: int| #![auto] matching_cycle(period, offset, step) == cycle
}

pub open spec fn matching_cycle_in_query(
    query_start: int,
    query_end: int,
    period: int,
    offset: int,
    cycle: int,
) -> bool {
    cycle_in_query(query_start, query_end, cycle) && matches_when(period, offset, cycle)
}

pub open spec fn nonmatching_cycle_in_query(
    query_start: int,
    query_end: int,
    period: int,
    offset: int,
    cycle: int,
) -> bool {
    cycle_in_query(query_start, query_end, cycle) && !matches_when(period, offset, cycle)
}

pub proof fn valid_when_parameters_are_legal(period: int, offset: int)
    requires
        valid_when(period, offset),
    ensures
        period > 0,
        0 <= offset < period,
{
}

pub proof fn matching_cycle_repeats_every_period(period: int, offset: int, cycle: int)
    requires
        valid_when(period, offset),
        matches_when(period, offset, cycle),
    ensures
        matches_when(period, offset, cycle + period),
{
    let step = choose|step: int| matching_cycle(period, offset, step) == cycle;
    assert(matching_cycle(period, offset, step + 1) == cycle + period) by (nonlinear_arith)
        requires
            matching_cycle(period, offset, step) == cycle,
    {
    }
}

pub proof fn positive_delta_smaller_than_period_breaks_match(
    period: int,
    offset: int,
    cycle: int,
    delta: int,
)
    requires
        valid_when(period, offset),
        matches_when(period, offset, cycle),
        0 < delta < period,
    ensures
        !matches_when(period, offset, cycle + delta),
{
    let base_step = choose|step: int| matching_cycle(period, offset, step) == cycle;
    if matches_when(period, offset, cycle + delta) {
        let shifted_step = choose|step: int| matching_cycle(period, offset, step) == cycle + delta;
        assert(delta == (shifted_step - base_step) * period) by (nonlinear_arith)
            requires
                matching_cycle(period, offset, base_step) == cycle,
                matching_cycle(period, offset, shifted_step) == cycle + delta,
        {
        }

        if shifted_step - base_step <= 0 {
            assert((shifted_step - base_step) * period <= 0) by (nonlinear_arith)
                requires
                    period > 0,
                    shifted_step - base_step <= 0,
            {
            }
        } else {
            assert(1 <= shifted_step - base_step);
            assert(period <= (shifted_step - base_step) * period) by (nonlinear_arith)
                requires
                    period > 0,
                    1 <= shifted_step - base_step,
            {
            }
        }
        assert(false);
    }
}

pub proof fn matching_cycle_in_query_stays_in_query(
    query_start: int,
    query_end: int,
    period: int,
    offset: int,
    cycle: int,
)
    requires
        matching_cycle_in_query(query_start, query_end, period, offset, cycle),
    ensures
        cycle_in_query(query_start, query_end, cycle),
{
}

pub proof fn nonmatching_cycle_in_query_stays_in_query(
    query_start: int,
    query_end: int,
    period: int,
    offset: int,
    cycle: int,
)
    requires
        nonmatching_cycle_in_query(query_start, query_end, period, offset, cycle),
    ensures
        cycle_in_query(query_start, query_end, cycle),
{
}

pub proof fn query_cycles_partition_into_matching_and_nonmatching(
    query_start: int,
    query_end: int,
    period: int,
    offset: int,
    cycle: int,
)
    requires
        valid_when(period, offset),
    ensures
        cycle_in_query(query_start, query_end, cycle)
            ==> (matching_cycle_in_query(query_start, query_end, period, offset, cycle)
                || nonmatching_cycle_in_query(query_start, query_end, period, offset, cycle)),
{
}

pub proof fn query_cycle_cannot_be_both_matching_and_nonmatching(
    query_start: int,
    query_end: int,
    period: int,
    offset: int,
    cycle: int,
)
    requires
        valid_when(period, offset),
    ensures
        !(matching_cycle_in_query(query_start, query_end, period, offset, cycle)
            && nonmatching_cycle_in_query(query_start, query_end, period, offset, cycle)),
{
}

} // verus!

fn main() {}
