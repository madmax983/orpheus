use orpheus_dsp::PluginFormat;
use orpheus_lang::{ReplMode, ReplSession, eval_module};

#[test]
fn vst_plugin_language_primitive_accepts_notes_and_parameter_automation() {
    let env = eval_module(
        r#"lead = vst("Serum") |> notes(c4 e4) |> p("Cutoff", 0.25)"#,
        ReplMode::Loose,
    )
    .unwrap();

    let plugin = env
        .get("lead")
        .and_then(orpheus_lang::Value::as_plugin_pattern)
        .expect("lead should evaluate to a plugin pattern");
    let source = plugin.track_source();

    assert_eq!(source.descriptor().format(), PluginFormat::Vst3);
    assert_eq!(source.descriptor().identifier(), "Serum");
    assert_eq!(
        source
            .notes()
            .iter()
            .map(|event| event.value.note_number())
            .collect::<Vec<_>>(),
        vec![60, 64]
    );
    assert_eq!(source.parameter_lanes()[0].name(), "Cutoff");
    assert!((source.parameter_lanes()[0].events()[0].value - 0.25).abs() < f32::EPSILON);
}

#[test]
fn plugin_bindings_can_be_loaded_into_the_live_mixer() {
    let mut session = ReplSession::with_engine(orpheus_dsp::EngineHandle::stub());

    let message = session
        .eval_line(r#"lead = vst("Serum") |> notes(c4)"#)
        .unwrap();
    let rendered = session.render_test_block_for_tui(256);

    assert_eq!(message, "bound lead: Plugin");
    assert!(
        rendered.iter().any(|sample| sample.abs() > 0.0001),
        "plugin binding should render through the standard mixer path"
    );
}
