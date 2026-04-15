# 🔭 Vantage: Spec for Scene Launching

## 👤 User Story
"As a Live Performer, I want to group multiple pattern bindings, mixing configurations, and tempo settings into distinct 'scenes' and launch them synchronously at the next cycle boundary, so that I can easily transition between major sections of my song (e.g., Intro, Verse, Chorus, Drop) without having to manually execute dozens of commands at exactly the right moment."

## ❓ The "So What?" (Business Problem)
Live coding is often frantic. A performer builds up a complex, multi-layered texture over several minutes. When it's time to drastically change the mood—say, dropping the kick drum, bringing in a new synth lead, and cutting the tempo in half—they must either rapidly type and execute multiple lines of code, or rely on complex mathematical pattern conditionals. Both approaches are error-prone and distracting. Traditional DAWs (like Ableton Live) solve this with Session View "Scenes," allowing entire horizontal rows of clips to launch together. Orpheus lacks this macro-level arrangement tool. Complexity is a cost, but performance fluidity is a core value proposition. Adding scene launching enables structured, multi-part performances, making Orpheus vastly more useful for structured electronic music, rather than just endlessly evolving loops.

## 🎯 Metric Definition
- **Success** = Users can define a scene containing any number of track bindings and parameter changes, and trigger the scene with a single command (e.g., `:scene launch chorus`). The transition must occur precisely at the start of the next cycle boundary, evaluating all state changes simultaneously with zero audio dropouts, and updating the TUI to reflect the new state.

## 🔍 Gap Analysis
- **Current State (Orpheus):** State changes (evaluating patterns) happen immediately upon execution. There is no concept of grouping evaluations or deferring them to a specific musical boundary.
- **Competitors (Ableton Live, Bitwig, Renoise):** Ableton and Bitwig are famous for non-linear Scene launching. Renoise uses a pattern matrix for a similar effect. Live coding environments typically lack this, forcing users to build huge monolithic functions to represent song sections.
- **The Gap:** Orpheus needs a declarative way to group session state mutations and a scheduling mechanism to apply them atomically at an exact future time (the downbeat).

## ✅ Acceptance Criteria
- Must introduce a way to define a "Scene" as a named collection of bindings and parameter overrides (e.g., in the `.ode` file or via a REPL command like `:scene define <name> { ... }`).
- Must provide a command to queue a scene for the next cycle (e.g., `:scene launch <name>`).
- Must provide a command to immediately launch a scene, ignoring cycle boundaries (e.g., `:scene force <name>`).
- Must atomically swap the session state at the target cycle boundary so all patterns in the scene begin playing precisely in sync.
- Must display the "queued" scene and the "active" scene clearly within the TUI transport or session pane.

## 🚫 Out of Scope
- Crossfading between scenes (e.g., a DJ-style transition). Phase 1 is a hard cut at the cycle boundary.
- Automated scene sequencing (e.g., "play Scene A for 16 bars, then Scene B"). Phase 1 relies on manual user triggers.
- Scene variations or complex sub-scenes. A scene is a flat collection of state overrides.
