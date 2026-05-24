use orpheus_lang::ReplSession;

#[test]
fn test_plugin_explain() {
    let mut session = ReplSession::with_engine(orpheus_dsp::EngineHandle::stub());
    session.eval_line("my_plugin = vst(\"dummy\")").unwrap();
    let output = session.eval_line(":explain my_plugin").unwrap();
    assert!(output.contains("Plugin Pattern Plan:"));
    assert!(output.contains("External VST3/LV2 Plugin"));
}
