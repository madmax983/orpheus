# 🔭 Vantage: Spec for Probabilistic and Stochastic Pattern Sequencing

## 👤 User Story
"As a Live Coder, I want to introduce controlled randomness and probabilistic variations (like Markov chains or weighted random choice) into my patterns, so that my sequences can evolve generatively over time without becoming perfectly repetitive or requiring constant manual intervention."

## ❓ The "So What?" (Business Problem)
Currently, Orpheus has functions like `rand` (for continuous random numbers) and `sometimes` (for random chance application of a transform), but its core composition model is largely deterministic and loop-driven. While repeating cycles are foundational, rigid repetition can lead to static, fatiguing music. If an artist wants generative, evolving structures, they must build complex logic manually. By introducing dedicated probabilistic operators (like choosing between options based on weights, or defining state-transition matrices like Markov chains), we expand Orpheus from a "sequencer" into an "algorithmic composer". Complexity is a cost; utility is a revenue. Providing high-level generative tools allows musicians to craft organic, breathing compositions with minimal code, drastically increasing the creative ceiling of the platform.

## 🎯 Metric Definition
- **Success** = 99% of probabilistic function evaluations (e.g., weighted choice or Markov transitions) are deterministic relative to the cycle/step salt (ensuring reproducible "randomness" during time-travel or playback), and introduce less than 1ms of overhead per cycle evaluation.

## 🔍 Gap Analysis
- **Current State (Orpheus):** Supports `sometimes` (binary chance of applying a function) and `rand` (continuous 0.0-1.0 signal). It lacks mechanisms to choose between discrete musical options probabilistically (e.g., 80% chance of 'C4', 20% chance of 'G4') or state-dependent sequencing.
- **Competitors (TidalCycles, Sonic Pi):** TidalCycles has extensive stochastic functions (`choose`, `wchoose`, `degradeBy`). Sonic Pi has built-in pseudo-random functions and lists. Max/MSP has robust Markov objects.
- **The Gap:** Orpheus needs a suite of stochastic pattern combinators that integrate naturally with its existing time and cycle architecture.

## ✅ Acceptance Criteria
- Must introduce a `choose` function to select uniformly from a list of patterns (e.g., `choose([bd, sn, cp])`).
- Must introduce a `wchoose` (weighted choose) function to select from a list based on relative probabilities.
- Must introduce a `degrade` function to randomly drop events from a pattern based on a probability threshold.
- Must introduce a `markov` function to define state transition probabilities for generative melodies or rhythms.
- Must ensure all randomness is pseudo-random and seeded consistently by the evaluation context (cycle number and time span) so that patterns sound identical every time the same cycle is rendered.
- Must be fully composable with existing pattern modifiers (e.g., `slow`, `every`).

## 🚫 Out of Scope
- Machine-learning based generative models (e.g., integrating LLMs or neural audio). Phase 1 is strictly traditional algorithmic probability.
- Complex graphical editors for Markov chains in the TUI.
