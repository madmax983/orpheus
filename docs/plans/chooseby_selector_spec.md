# 🔭 Vantage: Spec for chooseBy Selector

## 👤 **User Story:**
"As a Live Coder, I want to dynamically select between multiple patterns using an external control pattern, so that I can structurally orchestrate pattern changes over time rather than relying entirely on randomness."

## ❓ **The "So What?" (Business Problem):**
Currently, Orpheus lacks deterministic pattern indexing via a control signal. This forces users into verbose workarounds. By providing `choose_by`, we unlock precise structural orchestration. Complexity is a cost; utility is a revenue.

## 🎯 **Metric Definition:**
- **Success** = The `choose_by` function processes its index pattern and selects the target pattern with less than 1ms overhead per cycle.

## 🔍 **Gap Analysis:**
- **Current State (Orpheus):** Based on the parity roadmap, Orpheus is missing a deterministic, pattern-driven selector.
- **The Gap:** A deterministic, pattern-driven selector for lists of patterns.

## ✅ **Acceptance Criteria:**
- Must introduce a `choose_by` function taking an index pattern and a list of target patterns.
- Must evaluate deterministically based on the provided index pattern.

## 🚫 **Out of Scope:**
- Audio-rate index modulation for pattern selection.
