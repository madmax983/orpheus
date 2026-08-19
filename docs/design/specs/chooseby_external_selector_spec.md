# chooseBy External Selector Spec

## 👤 User Story
As a Livecoder, I want to use a deterministic number pattern to select from a list of patterns, so that I can decouple my structural rhythm from the source material.

## ❓ The "So What?" (Business Problem)
Decoupling structure from material reduces repetition in code, making the environment more expressive with fewer keystrokes. This allows users to build more complex compositions faster.

## 🎯 Metric Definition
Success = `chooseBy` function evaluates without frame drops, and is adopted in >5% of community shared scripts.

## 🔍 Gap Analysis
TidalCycles has `chooseBy`, which is heavily used by advanced users for structural mapping. Our `parity-roadmap.md` shows we currently only have `choose` and `pchoose` (random), leaving deterministic mapping via an external selector pattern unsupported as a remaining gap.

## ✅ Acceptance Criteria
- Must accept a numeric pattern as a selector and a list/array of patterns to choose from.
- Must deterministically select the target pattern using the value of the selector.
- Must handle out-of-bounds index values gracefully (e.g., modulo wrap).

## 🚫 Out of Scope
Audio-rate selection evaluation.
