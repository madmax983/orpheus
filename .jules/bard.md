## 2026-03-13 - [Missing Core README]
**Confusion:** The repository root was missing a `README.md`, making it hard for users to understand what Orpheus is, what the crates do, and how to get started (building/running).
**Clarification:** Created a new `README.md` at the root that introduces the Orpheus philosophy, explains the architecture/crates (`orpheus-pattern`, `orpheus-dsp`, `orpheus-lang`), and gives clear getting started instructions including pre-requisites (`libasound2-dev`).## 2024-05-18 - [eval.rs Missing Documentation]
**Confusion:** Several important evaluation functions in `orpheus-lang/src/eval.rs` (`eval_into_bindings`, rendering, and export utilities) lacked documentation, making it difficult for users to know how to call these APIs and what exact CSV format/rendering parameters were produced. The lack of `## Examples` and `## Errors` made the API opaque.
**Clarification:** Added comprehensive `///` documentation blocks describing parameter behavior, detailed CSV outputs, executable examples that utilize `eval_module` to preserve encapsulation, and explicit `## Errors` sections detailing failure states.
## 2026-03-24 - [value.rs Missing Documentation and Empty Doctests]
**Confusion:** The core runtime value types in `orpheus-lang/src/value.rs` (`Value`, `SampleEvent`, `SamplePatternValue`, `NumberPatternValue`) lacked descriptive documentation and had broken/empty doctests. This made it difficult for users to understand how to interact with the evaluator's output and extract meaningful events.
**Clarification:** Added detailed module-level explanations and populated the `/// ```` blocks with executable doc-tests utilizing `eval_module` (preserving the encapsulation rule). This demonstrates exactly how to extract patterns, query the unit cycle, and inspect generated parameters.
## 2026-03-24 - [The "Black Box": Module-Level Documentation in orpheus-lang]
**Confusion:** Several core components of the `orpheus-lang` crate lacked module-level (`//!`) documentation, leaving their high-level concepts and architecture unexplained. Specifically, `session.rs` (the REPL/TUI state manager), `builtins.rs` (the standard library of primitive functions and transformations), and `types/env.rs` (the Hindley-Milner type environment) acted as "Black Boxes".
**Clarification:** Added comprehensive `//!` documentation blocks at the top of these files, explaining *what* each file does and its role in the system before diving into *how* it's implemented. This clarifies the bridge between text and DSP (`session.rs`), the execution logic for transformations (`builtins.rs`), and the polymorphic type environment (`types/env.rs`).
## 2026-03-24 - [The "Black Box": Missing Module-Level Documentation in orpheus-pattern, orpheus-dsp, and orpheus-lang]
**Confusion:** Several core components of the `orpheus-pattern`, `orpheus-dsp` and `orpheus-lang` crates lacked or had incomplete module-level (`//!`) documentation, leaving their high-level concepts and architecture unexplained. Specifically, modules in `orpheus-dsp` (e.g. `engine.rs`, `offline.rs`, `scheduler.rs`), `orpheus-lang` (e.g. `export.rs`, `loader.rs`, `tui.rs`), and `orpheus-pattern` (e.g. `cycle.rs`, `event.rs`, `rational.rs`) acted as "Black Boxes".
**Clarification:** Added comprehensive `//!` documentation blocks at the top of these files, explaining *what* each module does and its role in the system. This clarifies the real-time audio rendering system, offline rendering, lock-free queues in DSP, visual rendering of patterns, and the exact rational number representations for continuous time.

## 2026-03-24 - [Broken Intra-Doc Link in session.rs]
**Confusion:** The `session.rs` module-level documentation linked to the private types `ReplSession` and `EngineHandle` via `[\`ReplSession\`]`, causing `cargo doc` to fail due to the dead intra-doc link.
**Clarification:** I replaced the broken intra-doc links with simple backticks (e.g., `\`ReplSession\``) to render the names as code elements without creating dead links.

## 2026-03-24 - [The "Black Box": Missing Documentation in Type System and Engine]
**Confusion:** The Hindley-Milner type system module (`crates/orpheus-lang/src/types/mod.rs`) and the digital signal processing backend (`crates/orpheus-dsp/src/lib.rs`) were acting as black boxes due to missing or sparse module-level (`//!`) documentation. It was unclear how type inference tied together or how the audio engine components interacted. Furthermore, the core `Type` enum lacked inline descriptions for its variants.
**Clarification:** Added detailed module-level documentation for both modules explaining their purpose and key components. Added inline `///` documentation to all variants of the `Type` enum explaining what semantic meaning they hold.

## 2026-03-24 - [The "Black Box": Missing Documentation in Mixer State]
**Confusion:** The mixer routing logic in `crates/orpheus-lang/src/mixer.rs` was acting as a black box due to missing module-level (`//!`) documentation and missing `///` struct documentation for `MixerState`. It was unclear how tracks, buses, and sends mapped from the evaluated language environment into DSP routing snapshots.
**Clarification:** Added detailed module-level documentation explaining the concepts of Tracks, Buses, and Sends. Added struct-level `///` documentation for `MixerState` along with an executable doctest demonstrating how to construct tracks, buses, and sends programmatically.

## 2026-03-24 - [The "Missing Link": Missing Examples in Export and Stats Functions]
**Confusion:** Export formatting functions (e.g., `export_sample_pattern_to_svg`, `export_sample_pattern_to_html`, `export_number_pattern_to_txt`) and analysis tools (`sample_pattern_stats`) in `orpheus-lang` had no `/// ## Examples` executable blocks. This made it difficult for developers to understand how to correctly extract a pattern from an evaluation environment and interact with these APIs.
**Clarification:** Added executable doctests to all functions in `crates/orpheus-lang/src/svg.rs`, `html.rs`, `txt.rs`, and `stats.rs` that utilize `eval_module` to parse a string, extract the pattern with `.get("...").unwrap().as_sample_pattern().unwrap()`, and pass it to the export functions, demonstrating correct instantiation and usage.

## 2026-03-24 - [Add Examples and Module Documentation]
**Confusion:** Some public features like tui entry points lack executable examples and type inferencing lacks module documentation explaining why it exists.
**Clarification:** I added `/// # Examples` code blocks for `run_with_engine` and `run_with_engine_and_path` in `tui.rs` and added `//!` module documentation for `infer.rs`.
