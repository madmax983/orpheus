use orpheus_dsp::EngineHandle;
use orpheus_lang::ReplMode;
use orpheus_pattern::TimeSpan;

fn main() {
    let mode = ReplMode::Loose;
    let span = TimeSpan::unit();
    let engine = EngineHandle::stub();
    let mode_name = match mode {
        ReplMode::Loose => "loose",
        ReplMode::Strict => "strict",
    };

    println!(
        "Orpheus workspace bootstrapped in {mode_name} mode; unit span starts at {} and engine handle is {:?}.",
        span.start_numer(),
        engine
    );
}
