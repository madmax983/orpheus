# Spec: chooseBy

## 👥 **User Story:**
As a Livecoder, I want to deterministically select patterns from a pool using an external index pattern, so that I can decouple my rhythm or melodic sequence from the actual voices/samples being triggered.

## 🤔 **The "So What?":**
What business problem does this solve?
Livecoders currently rely on probabilistic choices (`pchoose`, `randcat`) or sequential choices (`cat`). They lack the ability to *explicitly and deterministically* index into a pool of patterns over time. Adding `chooseBy` provides a crucial layer of structural control, enabling algorithmic composition where one pattern controls the "shape" and another provides the "content." Complexity is a cost, but Utility is a revenue: this decoupling is a core paradigm that Orpheus currently lacks.

## 📈 **Metric Definition:**
- Success = A user can sequence a list of 4 patterns using a 16-step numerical index pattern with zero dropouts.
- Success = `chooseBy` correctly honors the rhythmic structure of the index pattern.

## 🔍 **Gap Analysis:**
The current standard library provides `pchoose`, `wchoose`, `randcat`, and `cat`.
- `pchoose`/`wchoose` are probabilistic. They do not allow explicit sequencing.
- `cat`/`slowcat` are sequential and rigid.
- The `parity-roadmap.md` explicitly lists `chooseBy` driven by an external selector pattern as a remaining gap under the `randcat` / `pchoose` section.

## ✅ **Acceptance Criteria:**
- Must accept a numerical pattern (index) and a list of patterns (pool).
- Must resolve the index pattern and select the corresponding pattern from the pool.
- Must maintain the natural (global) timeline of the children patterns.
- Must be documented in the language reference.

## 🚫 **Out of Scope:**
- Nested `chooseBy` optimizations (should work out of the box without special-casing).
