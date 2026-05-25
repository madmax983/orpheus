#[cfg(test)]
mod tests {
    use crate::session::ReplSession;
    use orpheus_dsp::EngineHandle;

    #[test]
    fn test_plugin_explain() {
        let mut session = ReplSession::with_engine(EngineHandle::stub());
        session.eval_line("v = vst(\"test\")").unwrap();
        let res = session.eval_line(":explain v");
        println!("{res:?}");
        assert!(res.is_ok());
    }
}
