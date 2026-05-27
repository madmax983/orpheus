**[Avoid Intermediate Collections]
**Learning:** Chaining `.chars().take(n).collect::<String>()` creates an intermediate iterator allocation overhead. A small performance gain can be made by iterating directly and pushing into a pre-allocated String.
**Action:** Use `String::with_capacity(n)` and `.push()` in a loop instead of `.collect()` for small string manipulations.
**[Avoid Intermediate Collections]
**Learning:** Chaining `.chars().take(n).collect::<String>()` creates an intermediate iterator allocation overhead. A small performance gain can be made by iterating directly and pushing into a pre-allocated String.
**Action:** Use `String::with_capacity(n)` and `.push()` in a loop instead of `.collect()` for small string manipulations.

**[String Capacity for Characters]
**Learning:** `String::with_capacity(n)` allocates `n` *bytes*, not `n` *characters*. When iterating and pushing multi-byte Unicode characters, setting capacity exactly to the character count can still trigger re-allocations.
**Action:** When pre-allocating a `String` for an exact number of characters, consider maximum potential byte length (e.g., `n * 4` for UTF-8) or use `reserve` dynamically if exact byte length is unknown, though for small known caps, slightly over-allocating is safer than under-allocating.
