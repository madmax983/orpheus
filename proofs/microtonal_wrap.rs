use vstd::prelude::*;

verus! {

// The runtime uses integer div_euclid / rem_euclid for out-of-octave wrap.
// Spec versions below match Rust's i32 implementations for n > 0.

pub open spec fn rem_euclid(step: int, n: int) -> int
    recommends n > 0
{
    let r = step % n;
    if r < 0 { r + n } else { r }
}

pub open spec fn div_euclid(step: int, n: int) -> int
    recommends n > 0
{
    let r = step % n;
    if r < 0 { step / n - 1 } else { step / n }
}

// Core invariant: step == div_euclid(step, n) * n + rem_euclid(step, n).
pub proof fn div_rem_euclid_reconstruct(step: int, n: int)
    requires n > 0,
    ensures step == div_euclid(step, n) * n + rem_euclid(step, n),
{
}

// Remainder stays inside [0, n).
pub proof fn rem_euclid_bounded(step: int, n: int)
    requires n > 0,
    ensures 0 <= rem_euclid(step, n) < n,
{
}

// Positive-step examples: step 5 under a 4-step scale -> idx 1, octave 1.
pub proof fn positive_wrap_example()
    ensures
        div_euclid(5, 4) == 1,
        rem_euclid(5, 4) == 1,
{
}

// Negative-step examples: step -1 under a 4-step scale -> idx 3, octave -1.
pub proof fn negative_wrap_example()
    ensures
        div_euclid(-1, 4) == -1,
        rem_euclid(-1, 4) == 3,
{
}

// Whole-octave steps land on idx 0 and accrue full octaves.
pub proof fn octave_step_lands_on_root(n: int, k: int)
    requires n > 0,
    ensures
        rem_euclid(n * k, n) == 0,
        div_euclid(n * k, n) == k,
{
}

} // verus!
