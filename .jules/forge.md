**[Title]** Extracted Match Blocks and God Functions to Struct Methods
**Learning:** `clippy::too_many_lines` on functions with large `match` statements over an enum can often be resolved cleanly by moving the match block into an `impl` block on the enum itself. This follows Tell, Don't Ask, shrinks the caller function, and makes the enum operations more modular.
**Action:** When I encounter `too_many_lines` on a function switching over an enum to mutate state, I will implement methods directly on that enum instead of writing separate helper functions in the module scope.
