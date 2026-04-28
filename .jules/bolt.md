**[Curried Function Allocation Optimization]**
**Learning:** Passing structs containing pre-allocated vectors by reference to internal helpers forces new allocations and deep copies when modifying them.
**Action:** When a public method takes `self` by value, consume it completely by passing it by value to internal helper functions. This transfers ownership and allows reusing existing internal vector allocations (e.g., via `.extend()`), eliminating redundant heap allocations and clones.
