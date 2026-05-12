use vstd::prelude::*;

verus! {

// Hot-reload model:
// - the file watcher publishes immutable sample-bank snapshots
// - every accepted scan result advances the public revision
// - deletion is represented by a fresh inventory that excludes the removed token

pub open spec fn publish_reload_revision(current: nat) -> nat {
    current + 1
}

pub open spec fn token_is_present(inventory: Set<int>, token: int) -> bool {
    inventory.contains(token)
}

pub open spec fn purge_token(inventory: Set<int>, token: int) -> Set<int> {
    inventory.remove(token)
}

pub proof fn publish_reload_advances_revision(current: nat)
    ensures
        publish_reload_revision(current) > current,
{
    assert(publish_reload_revision(current) == current + 1);
}

pub proof fn purged_inventory_excludes_deleted_token(inventory: Set<int>, token: int)
    ensures
        !token_is_present(purge_token(inventory, token), token),
{
    assert(!purge_token(inventory, token).contains(token));
}

} // verus!

fn main() {}
