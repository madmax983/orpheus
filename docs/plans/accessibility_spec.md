# 🔭 Vantage: Spec for Screen Reader and Accessibility Support

## 👤 User Story
"As a visually impaired Live Coder, I want Orpheus to provide screen reader compatibility and robust accessibility features in the REPL and TUI, so that I can independently navigate the interface, write code, and receive audio feedback without relying on visual cues."

## ❓ The "So What?" (Business Problem)
Currently, Orpheus's REPL and TUI assume a fully sighted user. By neglecting accessibility, Orpheus creates an exclusionary environment, missing out on a passionate community of visually impaired musicians and programmers. True utility requires accessibility. Implementing screen reader support and accessible design patterns transforms Orpheus from an exclusive tool into an inclusive platform. This expands the user base, aligns with modern software standards, and ensures that the power of code-driven music creation is available to everyone. Complexity is a cost; accessibility is a fundamental requirement.

## 🎯 Metric Definition
- **Success** = Users with standard screen readers (e.g., NVDA, JAWS, VoiceOver, Orca) can navigate the REPL and TUI, hear accurate readouts of current context and typed input, and successfully execute patterns without visual assistance. All custom keybindings and visual feedback must have equivalent, configurable audio or text-to-speech alternatives.

## 🔍 Gap Analysis
- **Current State (Orpheus):** The interface is primarily visual. The TUI relies on terminal grids and colors to convey structure. The REPL lacks explicit hooks for screen reader integration.
- **Competitors (Sonic Pi, DAWs):** Sonic Pi has made significant strides in accessibility. DAWs like Reaper have robust screen reader support via OS accessibility APIs.
- **The Gap:** Orpheus lacks an accessibility layer. It needs structured text output that screen readers can easily parse, configurable text-to-speech (TTS) integration, and non-visual feedback mechanisms (e.g., auditory UI cues).

## ✅ Acceptance Criteria
- Must introduce a configuration option to enable an "accessibility mode" which optimizes terminal output for screen readers.
- Must ensure the REPL and TUI provide descriptive text equivalents for visual elements (e.g., announcing the current mode, errors, or successful execution).
- Must provide configurable text-to-speech (TTS) integration for critical alerts and status changes.
- Must support auditory UI cues (earcons) to provide non-visual feedback for actions like successful compilation or errors.
- Must ensure all keybindings are fully documented and navigable via keyboard without requiring a mouse.

## 🚫 Out of Scope
- Building a custom, cross-platform text-to-speech engine from scratch. Phase 1 will rely on OS-level accessibility APIs or existing TTS integrations.
- Braille display support. While important, Phase 1 focuses primarily on screen reader compatibility and audio feedback.
