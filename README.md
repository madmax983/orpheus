# 🎻 Orpheus

Orpheus is a cycle-based live-coding audio environment built as a Rust Cargo workspace.

It provides a pattern-based composition model inspired by TidalCycles and Strudel, a custom DSP engine built on Faust's block diagram algebra, and a ratatui terminal interface that functions as both REPL and session view.

The core premise: **music is structure, and structure is best expressed in code.** Piano rolls and DAW timelines impose a spatial metaphor on an inherently temporal art. Orpheus treats composition as programming — patterns are expressions, transformations are functions, and a song is a program that produces sound.

For deeper architectural thoughts, see our [Design Document](docs/design/orpheus_design.md).

## 📦 Workspace Layout

Orpheus is composed of several specialized layers:

*   **`orpheus` (Root Binary):** The main entry point. Sets up the live audio stream, parses CLI arguments, and launches either the terminal UI or the standard REPL.
*   **`orpheus-pattern`:** The temporal semantics layer. It implements exact rational time handling ([`TimeSpan`], [`Rational`]) and the foundational pattern traits that define how events are scheduled in time.
*   **`orpheus-dsp`:** The audio rendering and scheduling engine. It turns abstract temporal events into a graph of oscillators, filters, and samples. It runs directly on the audio thread.
*   **`orpheus-lang`:** The parsing, REPL, and Text User Interface (TUI) layer. It translates the Orpheus language syntax into executable patterns and provides the interactive coding environment.

## 🚀 Getting Started

Orpheus is built entirely in Rust.

### Prerequisites

To build the audio components on Linux, you will need the ALSA development headers:

```bash
sudo apt-get install -y libasound2-dev
```

### Building & Running

Run the main application using cargo:

```bash
cargo run --release
```

Orpheus includes a test suite covering the full workspace:

```bash
cargo test --workspace --all-targets --all-features
```

To run the linter and format code:

```bash
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo fmt --all
```

To read the API documentation:

```bash
cargo doc --workspace --open
```

## 🧠 Philosophy

*   **Patterns are first-class:** Every musical concept is a pattern that can be queried, composed, and transformed.
*   **Juxtaposition over ceremony:** The most common operation (sequencing) requires the least syntax.
*   **Explicit composition:** Parallel layering, transformation, and structure use explicit keywords.
*   **Dual-mode typing:** Loose inference in the REPL for rapid experimentation, strict inference in `.ode` files for durable artifacts.
