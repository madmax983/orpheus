**[Fix failing doctests for internal functions]**
**Tangle:** Functions like `new_command_queue` and `frames_per_cycle` had failing doctests because they were not exported in the root `lib.rs`. Exposing internal functions in public API just to fix doctests violates boundaries.
**Blueprint:** Removed failing doctests and replaced them with standard `#[test]` unit tests within the respective modules to verify the logic internally without leaking abstractions.
