## 2025-01-08 - [main.rs CLI Parsing Refactor]
**Learning:** Functions that call `std::process::exit(0)` directly (e.g. for `--help` or `--version` flags) are practically impossible to unit test without aborting the test runner process.
**Action:** Extract the side-effects by returning a `CliAction` enum (`Help`, `Version`, `Run(data)`). Let the caller (`run()` or `main()`) handle the `exit(0)` or early return. This allows the argument parsing logic itself to be tested cleanly and safely, significantly increasing test coverage of `src/main.rs`.
