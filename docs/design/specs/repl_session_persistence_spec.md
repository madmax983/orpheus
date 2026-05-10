# 🔭 Vantage: Spec for REPL Session Persistence

## 👤 User Story
"As a Live Coder, I want the REPL to auto-save my session history and state, so that I can resume my session after a crash or application restart without losing unsaved improvisations."

## ❓ The "So What?" (Business Problem)
Live coding performances and practice sessions are often improvisational. Forcing users to manually save every idea into a `.ode` file breaks their flow. If the application crashes or is accidentally closed, hours of creative experimentation in the REPL can be lost instantly. By implementing REPL session persistence, we remove the anxiety of data loss and lower the friction for experimentation, increasing the perceived reliability and utility of Orpheus as an instrument. Features are liabilities until they are used, but losing user data is a guaranteed way to lose users.

## 🎯 Metric Definition
- **Success** = Users can close and reopen the application and immediately see their previous REPL history and pattern bindings restored.
- **Success** = State serialization adds < 5ms of overhead per command to avoid audio thread blocking or UI stuttering.

## 🔍 Gap Analysis
- **Current State (Orpheus):** The REPL state is completely ephemeral. Only explicitly saved `.ode` files persist between application restarts.
- **Competitors (Python REPL, Node, DAWs):** Standard REPLs (like Python or Node) save command history to a `.history` file. Full DAWs auto-save project state.
- **The Gap:** Orpheus lacks an automatic persistence mechanism for its interactive evaluation state, leaving users vulnerable to data loss during live sessions.

## ✅ Acceptance Criteria
- Must automatically save every successfully evaluated REPL command to a durable history file (e.g., `~/.orpheus_history`).
- Must automatically serialize the current environment bindings (the AST state of active patterns) to a temporary auto-save file after each cycle boundary.
- Must provide a prompt or automatic restoration of the previous state upon launching the application if an unsaved session is detected.
- Must ensure that disk I/O for saving state happens asynchronously and never blocks the audio rendering thread or the main TUI thread.
- Must allow users to opt-out of auto-saving via a CLI flag (e.g., `--no-persist`).

## 🚫 Out of Scope
- Persisting complex DSP state like filter history or delay buffers (only the compositional AST state is saved).
- Full version control or branching of history (handled by Git + `.ode` files).
- Exporting the auto-saved session directly to a `.ode` file (Phase 2 feature).
