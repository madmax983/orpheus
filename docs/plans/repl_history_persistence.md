# 🔭 Vantage: Spec for REPL History Persistence

## 👤 User Story
"As a Live Coder, I want my REPL input history to persist across sessions, so that I can immediately recall, modify, and build upon the patterns I developed in my previous performance or practice session without having to manually re-type them or open an external file."

## ❓ The "So What?" (Business Problem)
Live coding is an iterative process of experimentation. When users close the Orpheus REPL or TUI, they lose their entire ephemeral context. Currently, the only way to persist work is to manually commit to a `.ode` file. This forces the user to context-switch out of the "flow state" of musical exploration into a "file management" mindset. Losing a great but uncommitted rhythm simply because the user had to restart the application introduces friction and frustration. By persisting history automatically, we remove the cognitive burden of saving work-in-progress, increasing user retention and encouraging shorter, more frequent jamming sessions. Complexity is a cost; utility is revenue. Automatically saving history adds massive utility with minimal user complexity.

## 🎯 Definition of Success
- **Success** = 100% of user inputs entered in the REPL or TUI are saved to a durable, local history file upon exit and are immediately available via the `Up`/`Down` arrow keys upon restarting the application, with zero perceptible delay (<5ms) during application startup or teardown.

## ✅ Acceptance Criteria
- Must automatically save all valid inputs (excluding blank lines) to a `.orpheus_history` file in the user's home directory (or appropriate XDG path) upon application exit.
- Must load the history file silently upon application startup and populate the `Up`/`Down` history ring.
- Must limit the history file to a configurable maximum number of entries (e.g., 1000 lines) to prevent unbounded file growth, purging the oldest entries first.
- Must deduplicate consecutive identical inputs in the history ring to avoid clutter.
- Must handle multiple concurrent Orpheus instances gracefully (e.g., appending to the file without corruption).

## 🚫 Out of Scope
- A visual history browser or search interface (e.g., `Ctrl+R` reverse search). Phase 1 is strictly linear `Up`/`Down` traversal.
- Persisting the state of the audio engine or active patterns. We are only saving the raw text input strings, not the evaluated AST or DSP graph.
- Syncing history across different machines or via the cloud.
