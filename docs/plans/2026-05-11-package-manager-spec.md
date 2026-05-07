# 🔭 Vantage: Spec for Package Manager

## 👤 User Story
"As a Live Coder, I want a package manager to share and discover sample packs and synth definitions, so that I can expand my sonic palette without building everything from scratch."

## ❓ The "So What?" (Business Problem)
Currently, sharing an Orpheus composition requires manually zipping up `.ode` files and any referenced `.wav` samples. If a user creates a brilliant subtractive synth definition or a curated 808 drum sample pack, there is no standardized way to distribute it or depend on it in a new project. This friction isolates users and prevents the growth of a vibrant, collaborative ecosystem. Complexity is a cost; utility is revenue. By providing a native package management system (e.g., `orpheus pkg add git@github.com:...`), we transform Orpheus from a standalone tool into a platform. A rich ecosystem of shared resources drastically lowers the barrier to entry for new users and significantly accelerates the creative process for professionals.

## 🎯 Metric Definition
- **Success** = Users can add a remote dependency (e.g., a GitHub repository containing `.ode` files and samples) via a command-line tool or REPL command. The system automatically downloads the package, resolves it, and allows the user to immediately `use "package/module"` without manual file management or restarting the session.

## 🔍 Gap Analysis
- **Current State (Orpheus):** `use` statements only work for local files. Sample directories are strictly local. There is no concept of a "project manifest" with dependencies.
- **Competitors (Node.js/npm, Rust/Cargo, SuperCollider/Quarks):** Ecosystems thrive on package managers. SuperCollider has Quarks for sharing extensions and synth definitions.
- **The Gap:** Orpheus lacks a dependency resolution mechanism, a project manifest format (e.g., `orpheus.toml`), and a tool to fetch remote repositories to a local cache.

## ✅ Acceptance Criteria
- Must introduce a project manifest file (e.g., `orpheus.toml`) to declare dependencies (git URLs or local paths).
- Must provide a CLI command (e.g., `orpheus add <url>`) to fetch and update dependencies into a local shared cache (`~/.orpheus/packages`).
- Must extend the `use` keyword to resolve imports from the local package cache if the module name matches a declared dependency.
- Must ensure that imported packages can contain both `.ode` files (synth definitions/patterns) and audio samples, and that sample paths within the package resolve correctly relative to the package root.
- Must not block the audio thread when a package is added dynamically during a live session (fetching must be asynchronous).

## 🚫 Out of Scope
- Creating a centralized package registry (like crates.io or npmjs.com) in Phase 1; Phase 1 will rely exclusively on Git URLs.
- Complex version resolution (e.g., semantic versioning conflicts). Phase 1 will simply clone the `main` branch or a specified tag.
- Executing arbitrary build scripts from packages.
