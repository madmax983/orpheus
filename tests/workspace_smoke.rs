//! General smoke tests for the Orpheus workspace.
//!
//! This suite ensures that the top-level crates (`orpheus-lang`, `orpheus-dsp`, `orpheus-pattern`)
//! can be correctly linked and that basic bootstrap types like `EngineHandle` and `ReplMode`
//! are available and functional in a testing environment without panicking.
use orpheus_dsp::EngineHandle;
use orpheus_lang::ReplMode;
use orpheus_pattern::TimeSpan;

#[test]
fn workspace_bootstrap_links_crates() {
    let mode = ReplMode::Loose;
    let span = TimeSpan::unit();
    let engine = EngineHandle::stub();
    assert!(matches!(mode, ReplMode::Loose));
    assert_eq!(engine, EngineHandle::stub());
    assert_eq!(span.start_numer(), 0);
}
