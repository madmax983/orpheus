use orpheus_lang::ReplSession;
use orpheus_dsp::EngineHandle;

fn main() {
    let mut session = ReplSession::with_engine(EngineHandle::stub());
    session.eval_line(":track new drums").unwrap();
    session.render_test_block_for_tui(1);

    let view = session.mixer_view();
    println!("summary: {}", view.summary());
}
