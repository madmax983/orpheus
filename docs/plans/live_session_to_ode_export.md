# 🔭 Vantage: Spec for Live Session to .ode Project Export

## 👤 User Story
"As a Live Coder, I want to instantly export my current live session (active patterns, mixer state, and loaded samples) into a structured `.ode` project directory, so that I can safely transition a spontaneous, improvised performance into a durable, version-controllable composition that I can reload and refine later."

## ❓ The "So What?" (Business Problem)
Live coding often yields "happy accidents"—complex, emergent musical structures that the performer didn't premeditate. Currently, capturing these moments is fragile. If the application crashes, or if the performer simply closes the session, the exact combination of active bindings and mixer states is lost forever, unless they manually copied every command into a separate file during the performance. This creates a "glass ceiling" where Orpheus is great for jamming but terrible for building long-term repertoire. Complexity is a cost; utility is revenue. Bridging the gap between ephemeral live state and durable artifacts exponentially increases the value of time spent in the engine. It allows users to jam, capture the magic, and then polish it later, transforming Orpheus into a complete ideation-to-composition pipeline.

## 🎯 Metric Definition
- **Success** = A user can execute a single command (e.g., `:export project my_song`) and within <100ms, Orpheus writes a structured directory (`my_song/`) containing a main `.ode` file representing the current active bindings and mixer state, alongside any necessary sample references or nested modules, without causing any audio dropouts or locking the real-time audio thread.

## 🔍 Gap Analysis
- **Current State (Orpheus):** The live session state exists only in memory (the `ReplSession` bindings and DSP mixer graph). The REPL history persists raw strings, but doesn't capture the final resolved *state* or dependencies.
- **Competitors (Ableton Live, TidalCycles, Sonic Pi):** DAWs naturally save state as projects (`.als`). Sonic Pi relies on the text buffer being the source of truth, so saving the buffer *is* saving the project. Tidal relies on external text editors.
- **The Gap:** Because Orpheus allows dynamic evaluation and incremental state mutation via the REPL/TUI, the current source of truth is the in-memory state, not a single text file. Orpheus needs a serialization layer that can reconstruct the `.ode` syntax required to recreate the exact current state.

## ✅ Acceptance Criteria
- Must introduce a new command (e.g., `:export project <name>`) in the REPL and TUI.
- Must generate a valid, strictly-typed `.ode` file that, when loaded via `use` or `:load`, exactly reproduces the audio output of the exported session.
- Must serialize all currently active pattern bindings (e.g., `drums = stack(...)`).
- Must serialize the current mixer state, including bus routings and global send levels, converting them into equivalent Orpheus setup commands or DSL syntax.
- Must ensure the export process happens asynchronously or fast enough to guarantee zero audio xruns on the real-time thread.
- Must generate a cohesive directory structure if custom sample definitions or multi-file setups are involved (e.g., copying referenced local samples into a project-local `samples/` folder).

## 🚫 Out of Scope
- Exporting the entire REPL history or "undo stack" into the project file. The export represents only the *current* active snapshot.
- Generating a standalone executable or compiling the project into a VST. Phase 1 simply generates standard `.ode` source code.
- Automatically uploading the exported project to a cloud service or version control system.
