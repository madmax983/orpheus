# ADR 0001: Workspace Layout

- Status: Accepted
- Date: 2026-03-08

## Context

Orpheus starts from a single binary crate, but the design already separates the system into three architectural layers: pattern semantics, DSP, and language/UI. Keeping everything in one crate would blur those boundaries and make it harder to test the subsystems independently as the REPL, scheduler, and type system come online.

## Decision

Adopt a Cargo workspace with:

- a root binary crate named `orpheus`
- `crates/orpheus-pattern` for temporal types and pattern behavior
- `crates/orpheus-dsp` for engine and scheduling work
- `crates/orpheus-lang` for parsing, typing, evaluation, and interface code

Shared package metadata, path dependencies, and lint policy live in the root `Cargo.toml`. Architecture records live in `docs/adr/`.

## Consequences

Positive:

- each layer can gain tests without dragging the full stack into every compile
- public interfaces between layers stay explicit
- the root binary can remain a thin integration point

Trade-offs:

- more manifests and crate wiring up front
- cross-crate changes require deliberate dependency management
- the Task 1 bootstrap smoke test in `crates/orpheus-pattern/tests/` needs
  `orpheus-dsp` and `orpheus-lang` as `dev-dependencies` so the integration
  test crate can import sibling workspace members

This is acceptable because Orpheus is explicitly designed as a layered system, not a single-crate toy.

Those test-only dependencies do not create a runtime layering violation. They are
limited to the bootstrap smoke test target, while the `orpheus-pattern` library
itself still has no runtime dependency on either higher layer.
