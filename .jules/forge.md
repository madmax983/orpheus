**[Refactoring: \`clippy::too_many_lines\` Redundant Wrappers]**
**Learning:** Extracting large match blocks into local helper methods solely to hold a \`#[allow(clippy::too_many_lines)]\` attribute creates unnecessary indirection and worsens readability.
**Action:** Apply the \`#[allow(...)]\` attribute directly on the original function and remove the redundant wrapper entirely to flatten the execution structure.

**[Refactoring: \`clippy::too_many_lines\` Redundant Wrappers]**
**Learning:** Extracting large match blocks into local helper methods solely to hold a \`#[allow(clippy::too_many_lines)]\` attribute creates unnecessary indirection and worsens readability.
**Action:** Apply the \`#[allow(...)]\` attribute directly on the original function and remove the redundant wrapper entirely to flatten the execution structure.
