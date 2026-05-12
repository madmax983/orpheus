use vstd::prelude::*;

verus! {

// Undo/redo history model:
// - a committed session change pushes one snapshot onto the undo stack
// - pushing into a full bounded stack first drops the oldest snapshot
// - a new committed change clears the redo stack, preserving a linear history

pub open spec fn session_history_limit() -> nat {
    50
}

pub open spec fn bounded_push_len(len: nat, limit: nat) -> nat
    recommends
        limit > 0,
        len <= limit,
{
    if len == limit {
        limit
    } else {
        len + 1
    }
}

pub open spec fn undo_len_after_undo(len: int) -> int
    recommends
        len > 0,
{
    len - 1
}

pub open spec fn redo_len_after_record() -> nat {
    0
}

pub proof fn bounded_push_preserves_limit(len: nat, limit: nat)
    requires
        limit > 0,
        len <= limit,
    ensures
        bounded_push_len(len, limit) <= limit,
{
    if len == limit {
        assert(bounded_push_len(len, limit) == limit);
    } else {
        assert(len < limit);
        assert(len + 1 <= limit);
        assert(bounded_push_len(len, limit) == len + 1);
    }
}

pub proof fn undo_reduces_available_undo_depth(len: int)
    requires
        len > 0,
    ensures
        undo_len_after_undo(len) >= 0,
        undo_len_after_undo(len) < len,
{
    assert(undo_len_after_undo(len) == len - 1);
}

pub proof fn recording_new_change_clears_redo_stack()
    ensures
        redo_len_after_record() == 0,
{
}

pub proof fn phase1_history_limit_is_nonzero()
    ensures
        session_history_limit() > 0,
{
    assert(session_history_limit() == 50);
}

} // verus!

fn main() {}
