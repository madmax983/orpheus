cat << 'INNER_EOF' > patch_session_tests.sh
sed -i 's/fn session_explain_returns_sample_pattern_plan() {/fn session_explain_returns_plugin_pattern_plan() {\n        let mut session = ReplSession::with_engine(orpheus_dsp::EngineHandle::stub());\n        session.eval_line("p = vst(\\"test\\")").unwrap();\n\n        let plan = session.eval_line(":explain p").unwrap();\n\n        assert!(plan.contains("Plugin Pattern Plan"));\n        assert!(plan.contains("test"));\n        assert!(plan.contains("VST3"));\n    }\n\n    #[test]\n    fn session_explain_returns_sample_pattern_plan() {/' crates/orpheus-lang/src/session.rs
INNER_EOF
sh patch_session_tests.sh
cargo test -p orpheus-lang explain
