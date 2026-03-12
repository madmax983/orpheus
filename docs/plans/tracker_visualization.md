# 🔭 Vantage: Spec for Tracker-Style Visualization

## 👤 User Story
"As a Live Coder, I want to see a real-time tracker-style grid of my active patterns, so that I can visually verify the rhythm, timing, and layering of my code during a performance."

## ❓ The "So What?" (Business Problem)
Orpheus currently relies entirely on auditory feedback and reading raw cycle-based code. When composing complex polyrhythms, rapid subdivisions, or dense multi-layered tracks, users cannot easily debug where an errant event is triggering or verify their code changes without waiting to hear the entire loop play out. A visual representation provides an immediate, real-time cross-check, reducing cognitive load and lowering the barrier to entry for new users trying to understand how cycle-based time maps to linear time. Complexity is a cost, and utility is revenue: making the composition visually digestible increases the utility of the TUI.

## 🎯 Definition of Success
- **Success** = Tracker grid correctly maps active cycle patterns to discrete time steps, rendering up to 16 rows of upcoming events across all active layers with UI render latency < 16ms (smooth 60fps refresh), while imposing absolutely zero locks or allocations on the audio thread.

## ✅ Acceptance Criteria
- Must render a read-only grid where rows represent upcoming time steps (quantized to a reasonable grid like 16th or 32nd notes) and columns represent currently active pattern layers (e.g., `drums`, `bass`).
- Must populate the grid dynamically by querying the active patterns ahead of the playback cursor.
- Must visually highlight the currently playing row/time step.
- Must be togglable on/off within the existing `ratatui` session view (e.g., occupying one of the panes).
- Must gracefully display unquantized or micro-timed events (e.g., an indicator showing an event falls between strict rows).

## 🚫 Out of Scope
- Interactive editing within the tracker grid. The grid is strictly read-only; the language is the only source of truth.
- Mouse support for scrubbing, selecting, or zooming.
- Piano-roll style continuous note-length bars. We are strictly sticking to trigger-based tracker style for Phase 1.
- Displaying real-time effect parameter automation (e.g., LFO curves). Focus solely on event triggers and notes.
