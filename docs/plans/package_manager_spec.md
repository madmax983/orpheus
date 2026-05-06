# 🔭 Vantage: Spec for Package Manager (Sharing Samples, Synths, and Patterns)

## 👤 User Story
"As a Live Coder, I want a package manager native to Orpheus, so that I can easily discover, install, and share custom samples, synth definitions, and pattern libraries with the community, drastically accelerating my workflow and expanding my sonic palette."

## ❓ The "So What?" (Business Problem)
Currently, a user's Orpheus environment is limited to the built-in assets and whatever custom `.ode` files or `.wav` samples they manually download and organize. This manual management creates friction: users have to worry about file paths, dependencies between shared code, and maintaining a personal library. In modern software ecosystems (NPM, Cargo, Python's pip), a robust package manager accelerates adoption by turning isolated tools into interconnected ecosystems. By not having a package manager, Orpheus misses out on network effects. The community's collective output remains fragmented. Complexity is a cost; utility is revenue. Introducing an integrated package manager makes Orpheus a platform, allowing users to leverage thousands of community-generated sounds and functions immediately, which multiplies the platform's value exponentially.

## 🎯 Metric Definition
- **Success** = Users can run a command (e.g., `:pkg install @community/drum-breaks`) to fetch a remote package containing `.ode` files and audio samples, which are automatically registered and can be instantly imported via `use "@community/drum-breaks"` in <2 seconds without causing audio engine lockups.

## 🔍 Gap Analysis
- **Current State (Orpheus):** Custom imports require manual file path resolution (`use "my_file.ode"`). Sample directories must be configured locally via `:samples ./samples`. No centralized sharing or remote fetching exists.
- **Competitors (TidalCycles, Sonic Pi, Node.js):** Tidal relies on SuperDirt quirks or manual Quarks installation. Sonic Pi relies on local file loading. General-purpose languages like Node or Rust have mature package management (NPM, Cargo) which are often co-opted by audio tools (e.g., Strudel using NPM).
- **The Gap:** Orpheus needs a dedicated manifest format (`orpheus.toml`), a centralized registry (or Git repository support), and commands integrated into the REPL to fetch, unpack, and cache remote assets safely so they can be securely evaluated.

## ✅ Acceptance Criteria
- Must introduce a package manifest specification (e.g., `orpheus.toml`) to define a package's name, version, and dependencies.
- Must provide CLI and REPL commands to install dependencies (e.g., `:pkg install <url/git>`).
- Must safely download and cache packages in a standard local directory (e.g., `~/.orpheus/packages`).
- Must extend the `use` import syntax to support resolving named packages from the local cache rather than just relative file paths.
- Must ensure that downloading a package asynchronously does not block or allocate on the real-time audio thread.

## 🚫 Out of Scope
- A centralized package registry web server (like crates.io). Phase 1 will support installing directly from Git repository URLs or local directories.
- Package publication workflows. Phase 1 focuses on the consumer experience (installing and using).
- Dependency conflict resolution (e.g., SAT solving for versions). Phase 1 will use a simple flat dependency structure or exact URL matching.