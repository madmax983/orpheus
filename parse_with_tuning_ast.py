with open("crates/orpheus-lang/src/value.rs") as f:
    text = f.read()

import re

# `with_tuning` has #[allow(clippy::too_many_lines, clippy::match_same_arms)]
# It's an impl method on a large enum PatternRuntime<T>.
# According to forge.md:
# "[Refactored massive function with enum match to impl method]
# Learning: clippy::too_many_lines on a function that pattern-matches an enum with 70+ variants (like an AST or PatternRuntime) is often caused by the match statement being a freestanding function instead of a method. Moving the function into an impl block as a method (e.g., fn with_tuning(self, ...)) follows Tell, Don't Ask, shrinks the caller function, and makes the enum operations more modular. If splitting the match into 70 different helper methods would destroy readability, it's safe to keep the large match and apply #[allow(clippy::too_many_lines)] specifically to the method.
# Action: When I encounter too_many_lines on a freestanding function switching over a massive enum, I will implement it as a method directly on that enum instead of writing separate helper functions in the module scope."

# Since `with_tuning` is ALREADY a method on the enum with `#[allow(clippy::too_many_lines)]`, we should LEAVE it alone. The forge.md specifically says "it's safe to keep the large match and apply #[allow(clippy::too_many_lines)] specifically to the method".

# The other three try_query_*_method methods are NOT switching over all 70+ variants directly. They are just chunking a single large match into multiple smaller methods (try_query_transform, try_query_audio_effect, try_query_modulation_effect).
