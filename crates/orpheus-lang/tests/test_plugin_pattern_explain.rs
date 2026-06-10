use orpheus_dsp::EngineHandle;
use orpheus_lang::ReplSession;

#[test]
fn test_plugin_pattern_explain_command() {
    let mut session = ReplSession::with_engine(EngineHandle::stub());
    session.eval_line("synth = vst(\"Serum\")").unwrap();
    let msg = session.eval_line(":explain synth").unwrap();
    assert!(msg.contains("Plugin Pattern Plan:"));
    assert!(msg.contains("Vst3"));
    assert!(msg.contains("Serum"));
}
