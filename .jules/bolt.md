**[Implement Copy for Rational and TimeSpan]**
**Learning:** Core temporal primitives like `Rational` (two `i128`s) and `TimeSpan` (two `Rational`s) are small enough (32 bytes and 64 bytes respectively) to implement `Copy`. Passing them by reference and relying on `.clone()` adds unnecessary heap/reference indirection overhead on the hottest execution paths inside pattern queries and evaluations.
**Action:** Always derive `Copy` for small, stateless mathematical or temporal primitives and pass them by value to eliminate `.clone()` noise and improve performance.
