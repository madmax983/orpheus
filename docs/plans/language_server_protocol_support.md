# 🔭 Vantage: Spec for Language Server Protocol (LSP) Support

## 👤 User Story
"As a Live Coder and Software Developer, I want Orpheus to provide a Language Server Protocol (LSP) implementation, so that I can use my preferred text editor (like VS Code, Neovim, or Helix) to write, evaluate, and debug Orpheus code with full syntax highlighting, auto-completion, hover documentation, and inline error diagnostics, rather than being restricted to the built-in TUI."

## ❓ The "So What?" (Business Problem)
Currently, Orpheus expects users to code directly within its built-in terminal UI or REPL. While functional, modern developers rely heavily on the advanced editing features of their IDEs (like multicursor editing, fuzzy file finding, and rich plugin ecosystems). Forcing users to abandon their highly-tuned editors to use a bespoke TUI creates friction and limits adoption among professional programmers. Implementing an LSP server turns Orpheus into a first-class language ecosystem. Complexity is a cost; utility is a revenue. By decoupling the editor from the audio engine, we allow users to leverage the tools they already know, massively increasing the accessibility, discoverability, and ergonomics of writing Orpheus compositions.

## 🎯 Metric Definition
- **Success** = An Orpheus LSP binary (`orpheus-lsp`) can attach to standard editors (VS Code, Neovim), providing sub-50ms latency for auto-completion of builtin functions and inline diagnostics (squiggly lines) for syntax/type errors, and supporting code evaluation via a custom LSP command sent to the running Orpheus DSP engine.

## 🔍 Gap Analysis
- **Current State (Orpheus):** The language parser and type checker (`orpheus-lang`) only report errors to stdout or the internal TUI. There is no external RPC interface to query the AST or symbol table.
- **Competitors (TidalCycles, Sonic Pi):** TidalCycles integrates via editor plugins (e.g., VS Code extension, Emacs mode) that send text to a background GHCi process. Sonic Pi has a custom GUI but also has third-party editor plugins.
- **The Gap:** Orpheus needs a standalone server process that speaks the JSON-RPC Language Server Protocol. It must wrap the existing `orpheus-lang` parsing/typing pipeline to expose diagnostics, completions, and hover info over stdin/stdout or TCP.

## ✅ Acceptance Criteria
- Must implement an LSP server supporting text document synchronization (`textDocument/didOpen`, `didChange`, `didClose`).
- Must provide real-time diagnostics (`textDocument/publishDiagnostics`) powered by the Orpheus parser and type checker.
- Must provide auto-completion (`textDocument/completion`) for all built-in language functions, DSP combinators, and currently defined variables.
- Must provide hover documentation (`textDocument/hover`) displaying type signatures and docstrings for functions.
- Must support a custom command/action (e.g., `orpheus/evaluate`) to send the current file or selection to a running Orpheus audio session for live playback.
- Must be packaged as an optional feature or standalone binary within the workspace.

## 🚫 Out of Scope
- Building explicit editor extensions (e.g., publishing a VS Code `.vsix` file). Phase 1 only delivers the backend LSP server; users can configure generic LSP clients to point to the binary.
- Semantic highlighting (`textDocument/semanticTokens`). Syntax highlighting can remain regex/TextMate-based for now.
- Advanced refactoring commands (Rename, Extract Function).
