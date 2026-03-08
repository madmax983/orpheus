# CLAUDE.md

## Workspace Overview

Orpheus is a Cargo workspace with a root binary crate and three library crates under `crates/`.

- `orpheus-pattern`: temporal data types, query semantics, and pattern transformations
- `orpheus-dsp`: audio scheduling and rendering
- `orpheus-lang`: parser, evaluator, typing, REPL, and TUI surface
- root `orpheus` binary: session entrypoint that wires the workspace together

## Development Rules

- Follow SPEC-PROOF-RED-GREEN-REFACTOR for behavior changes
- Add or update a Rust test before adding runtime behavior
- Put architecture decisions in `docs/adr/`
- Keep proof-oriented work under `proofs/`
- Run `cargo fmt --all`, `cargo clippy --all-targets --all-features -- -D warnings`, and targeted tests for each completed task
- Keep the audio-thread path allocation-free and lock-free once DSP work begins

## Initial Layout

```text
Cargo.toml
CLAUDE.md
docs/
  adr/
  design/
  plans/
crates/
  orpheus-pattern/
  orpheus-dsp/
  orpheus-lang/
src/
  main.rs
```

## Current Status

Task 1 bootstraps the workspace only. Public APIs are intentionally tiny stubs so later tasks can grow them with tests and proofs instead of speculative scaffolding.
