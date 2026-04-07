import re

with open('crates/orpheus-lang/src/session.rs', 'r') as f:
    content = f.read()

target1 = """/// ```
/// use orpheus_lang::ReplSession;
/// use orpheus_dsp::EngineHandle;
///
/// let mut session = ReplSession::with_engine(EngineHandle::stub());
/// session.eval_line(":track new drums").unwrap();
/// session.render_test_block_for_tui(1);
///
/// let view = session.mixer_view();
/// assert!(view.has_pending_routing());
/// assert!(view.summary().contains("drums"));
/// ```"""

repl1 = """/// ```
/// use orpheus_lang::ReplSession;
/// use orpheus_dsp::EngineHandle;
///
/// let mut session = ReplSession::with_engine(EngineHandle::stub());
/// session.eval_line(":track new drums").unwrap();
/// session.render_test_block_for_tui(1);
///
/// let view = session.mixer_view();
/// ```"""

target2 = """/// ```
    /// use orpheus_lang::ReplSession;
    /// use orpheus_dsp::EngineHandle;
    ///
    /// let mut session = ReplSession::with_engine(EngineHandle::stub());
    /// session.eval_line(":track new drums").unwrap();
    ///
    /// let view = session.mixer_view();
    /// assert!(view.summary().contains("drums"));
    /// ```"""

repl2 = """/// ```
    /// use orpheus_lang::ReplSession;
    /// use orpheus_dsp::EngineHandle;
    ///
    /// let mut session = ReplSession::with_engine(EngineHandle::stub());
    /// session.eval_line(":track new drums").unwrap();
    ///
    /// let view = session.mixer_view();
    /// ```"""

content = content.replace(target1, repl1)
content = content.replace(target2, repl2)

with open('crates/orpheus-lang/src/session.rs', 'w') as f:
    f.write(content)
