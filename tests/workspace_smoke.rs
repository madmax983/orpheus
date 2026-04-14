//! Tests that the workspace crates can be linked and accessed correctly.

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
