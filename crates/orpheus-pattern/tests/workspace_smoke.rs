use orpheus_dsp::EngineHandle;
use orpheus_lang::ReplMode;
use orpheus_pattern::TimeSpan;

#[test]
fn workspace_bootstrap_links_crates() {
    let _mode = ReplMode::Loose;
    let span = TimeSpan::unit();
    let _engine = EngineHandle::stub();
    assert_eq!(span.start_numer(), 0);
}
