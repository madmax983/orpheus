use vstd::prelude::*;
use vstd::arithmetic::div_mod::lemma_div_multiples_vanish;

verus! {

pub open spec fn wrapped_phase(phase: int, turn: int) -> int
    recommends
        turn > 0,
{
    ((phase % turn) + turn) % turn
}

pub open spec fn phase_advance(phase: int, step: int, turn: int) -> int
    recommends
        turn > 0,
{
    wrapped_phase(phase + step, turn)
}

pub open spec fn mix_endpoint(left: int, right: int, balance: int, scale: int) -> int
    recommends
        scale > 0,
        0 <= balance <= scale,
{
    ((scale - balance) * left + balance * right) / scale
}

pub open spec fn sat_bound(limit: int, sample: int) -> bool
    recommends
        limit >= 0,
{
    -limit <= sample <= limit
}

pub proof fn wrapped_phase_stays_in_range(phase: int, step: int, turn: int)
    requires
        turn > 0,
    ensures
        0 <= phase_advance(phase, step, turn) < turn,
{
    assert(0 <= wrapped_phase(phase + step, turn) < turn);
}

pub proof fn mix_endpoints_return_original_inputs(left: int, right: int, scale: int)
    requires
        scale > 0,
    ensures
        mix_endpoint(left, right, 0, scale) == left,
        mix_endpoint(left, right, scale, scale) == right,
{
    assert(mix_endpoint(left, right, 0, scale) == left) by {
        assert(((scale - 0) * left + 0 * right) / scale == (scale * left) / scale);
        lemma_div_multiples_vanish(left, scale);
    };
    assert(mix_endpoint(left, right, scale, scale) == right) by {
        assert(((scale - scale) * left + scale * right) / scale == (scale * right) / scale);
        lemma_div_multiples_vanish(right, scale);
    };
}

pub proof fn bounded_saturation_examples_hold()
    ensures
        sat_bound(1, 0),
        sat_bound(1, 1),
        sat_bound(1, -1),
{
    assert(sat_bound(1, 0));
    assert(sat_bound(1, 1));
    assert(sat_bound(1, -1));
}

} // verus!

fn main() {}
