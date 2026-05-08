# 🔭 Vantage: Spec for Language Server Protocol (LSP) Integration

## 👤 User Story
"As a Live Coder, I want a Language Server (LSP) for Orpheus, so that I get real-time type checking, autocomplete, and error highlighting in my external code editor (e.g., VSCode), rather than waiting to load a `.ode` file to discover syntax errors."

## ❓ The "So What?" (Business Problem)
Currently, composers building durable artifacts in `.ode` files are working blind. They lack the immediate feedback of the REPL and only discover type mismatches or syntax errors when they attempt to `:load` the file. This high-friction feedback loop discourages users from transitioning their live jams into structured projects. Complexity is a cost; utility is revenue. By providing an LSP, we meet users where they are (in their preferred editors), drastically lowering the barrier to writing correct Orpheus code and professionalizing the language's tooling ecosystem.

## 🎯 Metric Definition
- **Success** = The Orpheus LSP binary can attach to standard editors (VSCode, Neovim) and return diagnostics (syntax and type errors) in <50ms per keystroke.

## 🔍 Gap Analysis
- **Current State (Orpheus):** The parser and type inference engine (`orpheus-lang`) only run when a file is explicitly loaded or a REPL command is evaluated. There is no background static analysis tool.
- **Competitors (TidalCycles, Sonic Pi):** Sonic Pi has a built-in custom editor with autocomplete. TidalCycles integrates tightly with Haskell tooling and editor plugins (Atom/VSCode) to evaluate blocks and show errors.
- **The Gap:** Orpheus has a clean separation between parsing/typing and audio execution, making it a perfect candidate for an LSP, but it currently lacks the server wrapper to expose this capability to external editors.

## ✅ Acceptance Criteria
- Must implement a standalone `orpheus-lsp` binary (or an `orpheus lsp` subcommand) that communicates via stdin/stdout using standard JSON-RPC.
- Must support `textDocument/didOpen`, `textDocument/didChange`, and `textDocument/didClose` to track in-memory buffer state.
- Must run the Orpheus parser and strict-mode type checker on the buffer, returning `textDocument/publishDiagnostics` for any parse errors or type mismatches.
- Must provide basic `textDocument/completion` for built-in functions (e.g., `fast`, `every`, `stack`) and tokens.

## 🚫 Out of Scope
- Semantic token highlighting (Phase 2).
- Automatic code formatting via `textDocument/formatting` (Phase 2).
- Evaluating patterns or playing audio from the LSP (the LSP is strictly for static analysis; audio execution remains in the Orpheus session).
