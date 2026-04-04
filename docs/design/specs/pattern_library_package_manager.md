# 🔭 Vantage: Spec for Pattern Library Package Manager

## 👤 User Story
"As a Composer, I want to easily import and share community-created synth definitions, drum patterns, and DSP effects, so that I can expand my musical vocabulary without having to reinvent the wheel for every session."

## ❓ The "So What?" (Business Problem)
Currently, Orpheus is a closed ecosystem where users must define all their sounds and patterns from scratch or manually copy-paste `.ode` files between projects. This severely limits the growth of the community and increases the barrier to entry for new users who want to make good music quickly. A package manager fosters a vibrant ecosystem of shared knowledge, turning Orpheus from an isolated tool into a collaborative platform. Complexity is a cost; utility is revenue. Making it trivial to share and reuse code dramatically increases the platform's utility and retention rate.

## 🎯 Metric Definition
- **Success** = A user can type a command like `:install @community/techno-drums` in the REPL or declare it in their `.ode` file, and Orpheus will automatically download, cache, and make the module available for immediate use with zero restarts or manual file management.

## 🔍 Gap Analysis
- **Current State (Orpheus):** No mechanism for sharing code beyond manual file distribution. `use "file.ode"` only works for local, manually managed files.
- **Competitors (TidalCycles, Strudel):** Strudel allows importing patterns via URLs. TidalCycles relies on SuperDirt's global sample bank and Haskell's Cabal for library management, which is often clunky.
- **The Gap:** Orpheus needs a seamless, built-in dependency management system tailored specifically for live coding assets (code + samples) that feels as instant as importing a local file.

## ✅ Acceptance Criteria
- Must introduce a REPL command (e.g., `:install <package>`) and `.ode` syntax (e.g., `use "@namespace/package"`) to fetch dependencies.
- Must fetch packages from a centralized or decentralized registry (e.g., GitHub repos or a dedicated index).
- Must cache downloaded packages globally to allow offline use after the first fetch.
- Must support versioning to ensure `.ode` files remain deterministic and sound the same years later.
- Must automatically resolve and install transitive dependencies.

## 🚫 Out of Scope
- Publishing packages directly from the REPL (can be done via CLI or Git in Phase 1).
- Sandboxing or security models for downloaded code (assume trusted code for Phase 1).
- Managing massive sample libraries (focus on code and small synth definitions first).