1. **Submit "Wreckage Report" PR**
   - Provide a final summary of all chaos engineering attacks, documenting the resilience of the current system architecture, the bounded nature of the AST/evaluator limiters, and the safe memory performance.
   - Include the single required "Havoc" trait check (running `cargo clippy --all-targets --all-features -- -D warnings`, `cargo test`, `cargo fmt --all`) as the pre-commit action.
