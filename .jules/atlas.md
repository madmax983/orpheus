**[Standardize error types]
**Tangle:** Manual implementation of `std::error::Error` for various error types like `EvalError`, `ParseError`, `TypeError`, `LoadError`, `RenderError` and `PitchLiteralError` in `orpheus-lang`, and `PatternError` in `orpheus-pattern`. This caused boilerplate and inconsistencies.
**Blueprint:** Standardized error types across the workspace by adopting the `thiserror` crate in `orpheus-lang` and `orpheus-pattern`, matching the convention already established in `orpheus-dsp`. This simplifies error definitions using `#[derive(Error)]` and `#[error(...)]` attributes.`
**[Enforce Private TUI Internal Modules]
**Tangle:** The `state` and `style` modules in `orpheus-lang/src/tui/mod.rs` were declared as `pub mod`, leaking the internal TUI implementation details.
**Blueprint:** Removed the `pub` visibility modifier from these modules in `tui/mod.rs`, converting them to `mod state;` and `mod style;`. This enforces strong module boundaries and prevents leaky abstractions.

**[Enforce Private Mermaid Module]
**Tangle:** The `mermaid` module in `orpheus-lang/src/lib.rs` was declared as `pub mod`, leaking the internal implementation details of the Mermaid export module.
**Blueprint:** Changed `pub mod mermaid;` to `pub(crate) mod mermaid;` in `crates/orpheus-lang/src/lib.rs`. This enforces strong module boundaries by keeping the module internal while the intended public API (`export_sample_pattern_to_mermaid_gantt`) is explicitly exposed via `pub use`.

**[Enforce Private Number Roll Module]
**Tangle:** The `number_roll` module in `orpheus-lang/src/lib.rs` was declared as `pub mod`, leaking the internal implementation details of the ASCII number roll module.
**Blueprint:** Changed `pub mod number_roll;` to `pub(crate) mod number_roll;` in `crates/orpheus-lang/src/lib.rs`. This enforces strong module boundaries by keeping the module internal while the intended public API (`render_ascii_number_roll`) is explicitly exposed via `pub use`.

**[Standardize EvalError type]
**Tangle:** Manual implementation of `From` for cloneable error types in `EvalError` inside `crates/orpheus-lang/src/eval.rs`, causing boilerplate and losing inner type structure.
**Blueprint:** Converted `EvalError` from a flat struct to an enum using the `thiserror` crate's `#[from]` attribute for cloneable types, standardizing error boundaries.

**[Enforce Private Type Inference Environment]
**Tangle:** The `TypeEnv` and `TypeScheme` structs in `orpheus-lang/src/types/env.rs` were declared as `pub struct`, unnecessarily leaking the internal type-checker abstractions to the public API where only `TypedModule` is expected to be consumed.
**Blueprint:** Modified `TypeEnv` and `TypeScheme` (and their respective methods) to use `pub(crate)` visibility instead. This reinforces strong module boundaries and correctly encapsulates the language's inference engine implementation details.
**[Enforce Public Structure inside Private Module]
**Tangle:** The `TypeScheme` and `TypeEnv` structs in `orpheus-lang/src/types/env.rs` were declared as `pub(crate) struct`, which triggers `clippy::redundant_pub_crate` because the parent module `env` is private.
**Blueprint:** Modified `TypeScheme` and `TypeEnv` to use `pub` visibility instead of `pub(crate)`. This satisfies Clippy while correctly maintaining the private boundary since the module itself is private, making the items effectively crate-visible.
**[Fix Leaky Abstraction in Value and FunctionValue Enums]
**Tangle:** The `Value` and `FunctionValue` enums in `orpheus-lang` were public and exposed inner payload types like `ArpDirectionValue`, `PitchClassSetValue`, `BuiltinFn`, `BuiltinKind`, and `UserFn` as part of their variants. However, these inner types were not re-exported in the crate's `lib.rs`, creating a leaky abstraction where consumers could match on the variants but could not explicitly name the types of the values they extracted.
**Blueprint:** Explicitly re-exported `ArpDirectionValue`, `BuiltinFn`, `BuiltinKind`, `PitchClassSetValue`, and `UserFn` from the `value` module inside `crates/orpheus-lang/src/lib.rs` to ensure all publicly reachable types are fully nameable.
**[Enforce Private SuperCollider Export Module]
**Tangle:** The  module in  was declared as , leaking the internal implementation details of the SuperCollider export module.
**Blueprint:** Changed  to  in . This enforces strong module boundaries by keeping the module internal while the intended public APIs ( and ) are explicitly exposed via .
**[Enforce Private SuperCollider Export Module]
**Tangle:** The `supercollider_export` module in `orpheus-lang/src/lib.rs` was declared as `pub mod`, leaking the internal implementation details of the SuperCollider export module.
**Blueprint:** Changed `pub mod supercollider_export;` to `pub(crate) mod supercollider_export;` in `crates/orpheus-lang/src/lib.rs`. This enforces strong module boundaries by keeping the module internal while the intended public APIs (`export_number_pattern_to_supercollider` and `export_sample_pattern_to_supercollider`) are explicitly exposed via `pub use`.
**[Fix Leaky Abstraction in ValidatedPedalPlan and GatePatternValue Enums]
**Tangle:** The `ValidatedPedalPlan` and `Value` enums in `orpheus-lang` were public and exposed inner payload types like `ValidatedPedalBinding`, `ValidatedPedalNode`, `PedalNodeKind`, and `GatePatternValue` as part of their variants. However, these inner types were either private or not re-exported in the crate's `lib.rs`, creating a leaky abstraction where consumers could not explicitly name the types of the values they extracted or the compiler complained about privacy.
**Blueprint:** Explicitly re-exported `GatePatternValue`, `ValidatedPedalBinding`, and `ValidatedPedalNode` from the `value` and `pedal` modules inside `crates/orpheus-lang/src/lib.rs` and made `PedalNodeKind` public to ensure all publicly reachable types are fully nameable and privacy boundaries are respected.
**[Fix Leaky Abstraction in Type Enum]
**Tangle:** The `Type` enum in `orpheus-lang` was public and exposed the inner payload type `TypeVarId` as part of its `Var` variant. However, this inner type was not re-exported in the crate's `lib.rs`, creating a leaky abstraction.
**Blueprint:** Explicitly re-exported `TypeVarId` from the `types` module inside `crates/orpheus-lang/src/lib.rs` to ensure all publicly reachable types are fully nameable.

**[Float Epsilon Equality Bug]**
**Learning:** Checking floating-point equality in tests using `val - expected < f64::EPSILON` is a logical bug because a negative difference will always evaluate as less than epsilon, causing false positives.
**Action:** Always apply `.abs()` to the difference before comparing to epsilon: `(val - expected).abs() < f64::EPSILON`.

**[Simplify IO Other Error]**
**Learning:** Instantiating generic IO errors using `std::io::Error::new(std::io::ErrorKind::Other, "message")` triggers `clippy::io_other_error`.
**Action:** Use the cleaner, modern shorthand `std::io::Error::other("message")`.
**[Fix Leaky Abstractions and Broken Doctests]
**Tangle:** Several `orpheus_lang` public APIs referenced internal, private types (like `GraphBinding`, `TypeEnv`, and `TypeScheme`), causing leaky abstractions. Furthermore, missing getter methods for variants like `FunctionValue` caused test failures. Several documentation tests were bypassing the crate facade by reaching directly into private submodules.
**Blueprint:** Explicitly re-exported internal types (`GraphBinding`, `TypeEnv`, `TypeScheme`) in `lib.rs` and the `types/mod.rs` module. Refactored doctests to utilize the public facade and exposed a `Value::as_function` method to fulfill the expected public API contract without exposing underlying structural data prematurely.

**[Fix Leaky Abstraction in AST GraphBinding]**
**Tangle:** The `Expr` enum in `orpheus-lang` was public and exposed the inner payload type `GraphBinding` as part of its `Graph` variant. However, this inner type was not re-exported in the crate's `lib.rs`, creating a leaky abstraction where consumers could match on the variant but could not explicitly name the type of the value they extracted.
**Blueprint:** Explicitly re-exported `GraphBinding` from the `ast` module inside `crates/orpheus-lang/src/lib.rs` to ensure all publicly reachable types are fully nameable.
**[Enforce Private Explain Module]
**Tangle:** The `explain` module in `orpheus-lang/src/lib.rs` and its internal `Explain` trait and `explain_table` function were declared as `pub`, leaking internal REPL table rendering details to the public API.
**Blueprint:** Changed the visibility of the `Explain` trait and `explain_table` function to `pub(crate)` in `crates/orpheus-lang/src/explain.rs`. Removed the `pub use explain::Explain;` re-export from `crates/orpheus-lang/src/lib.rs` and changed the module declaration to `pub(crate) mod explain;`. This strictly enforces internal encapsulation.
**[Enforce Private Orca and Env Modules]
**Tangle:** The , , and  modules were publicly exposed in , leaking internal abstractions to the crate's public API.
**Blueprint:** Changed  and  visibility to  and SHELL=/bin/bash
NVM_INC=/home/jules/.nvm/versions/node/v22.22.1/include/node
SUDO_GID=1001
TERM_PROGRAM_VERSION=3.4
TMUX=/tmp/tmux-1001/default,2345,0
JAVA_HOME=/usr/lib/jvm/java-21-openjdk-amd64
DOTNET_ROOT=/usr/lib/dotnet
SUDO_COMMAND=/usr/bin/bash -c echo "${BASHPID}"
tmux new-session -d -s 'default' -c /app -e JULES_SESSION_ID=15830075136931015476 -e GIT_TERMINAL_PROMPT=0 && tmux set-option remain-on-exit on
SUDO_USER=jules
FLUTTER_HOME=/opt/flutter
PWD=/app
LOGNAME=jules
JULES_SESSION_ID=15830075136931015476
HOME=/home/jules
LANG=C.UTF-8
LS_COLORS=rs=0:di=01;34:ln=01;36:mh=00:pi=40;33:so=01;35:do=01;35:bd=40;33;01:cd=40;33;01:or=40;31;01:mi=00:su=37;41:sg=30;43:ca=00:tw=30;42:ow=34;42:st=37;44:ex=01;32:*.tar=01;31:*.tgz=01;31:*.arc=01;31:*.arj=01;31:*.taz=01;31:*.lha=01;31:*.lz4=01;31:*.lzh=01;31:*.lzma=01;31:*.tlz=01;31:*.txz=01;31:*.tzo=01;31:*.t7z=01;31:*.zip=01;31:*.z=01;31:*.dz=01;31:*.gz=01;31:*.lrz=01;31:*.lz=01;31:*.lzo=01;31:*.xz=01;31:*.zst=01;31:*.tzst=01;31:*.bz2=01;31:*.bz=01;31:*.tbz=01;31:*.tbz2=01;31:*.tz=01;31:*.deb=01;31:*.rpm=01;31:*.jar=01;31:*.war=01;31:*.ear=01;31:*.sar=01;31:*.rar=01;31:*.alz=01;31:*.ace=01;31:*.zoo=01;31:*.cpio=01;31:*.7z=01;31:*.rz=01;31:*.cab=01;31:*.wim=01;31:*.swm=01;31:*.dwm=01;31:*.esd=01;31:*.avif=01;35:*.jpg=01;35:*.jpeg=01;35:*.mjpg=01;35:*.mjpeg=01;35:*.gif=01;35:*.bmp=01;35:*.pbm=01;35:*.pgm=01;35:*.ppm=01;35:*.tga=01;35:*.xbm=01;35:*.xpm=01;35:*.tif=01;35:*.tiff=01;35:*.png=01;35:*.svg=01;35:*.svgz=01;35:*.mng=01;35:*.pcx=01;35:*.mov=01;35:*.mpg=01;35:*.mpeg=01;35:*.m2v=01;35:*.mkv=01;35:*.webm=01;35:*.webp=01;35:*.ogm=01;35:*.mp4=01;35:*.m4v=01;35:*.mp4v=01;35:*.vob=01;35:*.qt=01;35:*.nuv=01;35:*.wmv=01;35:*.asf=01;35:*.rm=01;35:*.rmvb=01;35:*.flc=01;35:*.avi=01;35:*.fli=01;35:*.flv=01;35:*.gl=01;35:*.dl=01;35:*.xcf=01;35:*.xwd=01;35:*.yuv=01;35:*.cgm=01;35:*.emf=01;35:*.ogv=01;35:*.ogx=01;35:*.aac=00;36:*.au=00;36:*.flac=00;36:*.m4a=00;36:*.mid=00;36:*.midi=00;36:*.mka=00;36:*.mp3=00;36:*.mpc=00;36:*.ogg=00;36:*.ra=00;36:*.wav=00;36:*.oga=00;36:*.opus=00;36:*.spx=00;36:*.xspf=00;36:*~=00;90:*#=00;90:*.bak=00;90:*.crdownload=00;90:*.dpkg-dist=00;90:*.dpkg-new=00;90:*.dpkg-old=00;90:*.dpkg-tmp=00;90:*.old=00;90:*.orig=00;90:*.part=00;90:*.rej=00;90:*.rpmnew=00;90:*.rpmorig=00;90:*.rpmsave=00;90:*.swp=00;90:*.tmp=00;90:*.ucf-dist=00;90:*.ucf-new=00;90:*.ucf-old=00;90:
DOTNET_BUNDLE_EXTRACT_BASE_DIR=/home/jules/.cache/dotnet_bundle_extract
NVM_DIR=/home/jules/.nvm
LESSCLOSE=/usr/bin/lesspipe %s %s
ANDROID_HOME=/opt/android-sdk
TERM=tmux-256color
LESSOPEN=| /usr/bin/lesspipe %s
USER=jules
TMUX_PANE=%0
SHLVL=1
NVM_CD_FLAGS=
CHROME_EXECUTABLE=/usr/bin/google-chrome
DEBUGINFOD_URLS=https://debuginfod.ubuntu.com
BUN_INSTALL=/usr/local/bun
PATH=/home/jules/.nvm/versions/node/v22.22.1/bin:/home/jules/.pyenv/shims:/home/jules/.pyenv/bin:/home/jules/.local/bin:/opt/flutter/bin:/usr/lib/dotnet:/opt/android-sdk/cmdline-tools/latest/bin:/opt/android-sdk/platform-tools:/go/bin:/usr/local/go/bin:/usr/share/gradle/bin:/usr/share/maven/bin:/home/jules/.local/bin:/home/jules/.cargo/bin:/usr/local/bun/bin:/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin:/snap/bin:/home/jules/.dotnet/tools
SUDO_UID=1001
NVM_BIN=/home/jules/.nvm/versions/node/v22.22.1/bin
MAIL=/var/mail/jules
GIT_TERMINAL_PROMPT=0
OLDPWD=/app
TERM_PROGRAM=tmux
_=/usr/bin/env to . Re-exported the types required by integration tests via  at the crate root to maintain test compatibility without polluting public docs.
**[Enforce Private Orca and Env Modules]
**Tangle:** The `orca`, `orca::transport`, and `types::env` modules were publicly exposed in `orpheus-lang`, leaking internal abstractions to the crate's public API.
**Blueprint:** Changed `orca` and `orca::transport` visibility to `pub(crate)` and `env` to `mod`. Re-exported the types required by integration tests via `#[doc(hidden)] pub use` at the crate root to maintain test compatibility without polluting public docs.
