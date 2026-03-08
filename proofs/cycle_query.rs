use vstd::prelude::*;

verus! {

// Model an equal subdivision of the unit cycle with a shared denominator `len`.
// Subdivision `i` occupies the half-open slot `[i, i + 1)` over that common
// denominator, so the corresponding rational span is `[i / len, (i + 1) / len)`.
// Runtime refinement from exact rational arithmetic into this integer-slot
// model is deferred until the pattern core grows more proof surface.
pub open spec fn subdivision_start(i: int) -> int {
    i
}

pub open spec fn subdivision_end(i: int) -> int {
    i + 1
}

pub open spec fn valid_subdivision(i: int, len: nat) -> bool {
    0 <= i < len
}

pub open spec fn slot_is_covered(slot: int, len: nat) -> bool {
    exists|i: int| #![auto] {
        valid_subdivision(i, len)
            && subdivision_start(i) <= slot
            && slot < subdivision_end(i)
    }
}

pub proof fn equal_subdivision_covers_whole_cycle(len: nat)
    requires
        len > 0,
    ensures
        subdivision_start(0) == 0,
        subdivision_end(len - 1) == len,
        forall|i: int| 0 <= i && i + 1 < len ==> subdivision_end(i) == subdivision_start(i + 1),
        forall|slot: int| 0 <= slot < len ==> slot_is_covered(slot, len),
{
    assert(subdivision_start(0) == 0);
    assert(subdivision_end(len - 1) == (len - 1) + 1);
    assert((len - 1) + 1 == len);

    assert forall|i: int| 0 <= i && i + 1 < len implies subdivision_end(i) == subdivision_start(i + 1) by {
        assert(subdivision_end(i) == i + 1);
        assert(subdivision_start(i + 1) == i + 1);
    }

    assert forall|slot: int| 0 <= slot < len implies slot_is_covered(slot, len) by {
        let i = slot;
        assert(valid_subdivision(i, len));
        assert(subdivision_start(i) <= slot);
        assert(slot < subdivision_end(i));
    }
}

} // verus!

fn main() {}
