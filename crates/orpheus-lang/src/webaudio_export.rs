//! The `webaudio_export` module provides an exporter to a standalone playable HTML file.
//!
//! This exporter generates an `.html` file containing embedded JavaScript that
//! leverages the browser's native `AudioContext` to synthesize and sequence
//! evaluated Orpheus patterns.

use std::io::Write;
use std::path::Path;

use crate::eval::{EvalError, render_span};
use crate::value::{NumberPatternValue, SamplePatternValue};

// Assume 120 BPM, 4 beats per cycle -> 1 cycle = 2.0 seconds = 2000 ms.
const SECONDS_PER_CYCLE: f64 = 2.0;

/// Exports a number pattern's evaluated events to an HTML file using Web Audio API.
///
/// Number patterns are assumed to represent MIDI pitch numbers, mapped to
/// `OscillatorNode` frequencies in JavaScript.
///
/// # Examples
///
/// ```no_run
/// use orpheus_lang::{ReplMode, eval_module};
/// use orpheus_lang::export_number_pattern_to_webaudio;
///
/// let env = eval_module("x = 60 62 64", ReplMode::Loose).unwrap();
/// let pattern = env.get("x").unwrap().as_number_pattern().unwrap();
///
/// let path = std::env::temp_dir().join("export_num_audio.html");
/// export_number_pattern_to_webaudio(pattern, &path, 2).unwrap();
/// ```
///
/// # Errors
///
/// Returns [`EvalError`] if pattern querying fails or if the file cannot be written.
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
pub fn export_number_pattern_to_webaudio(
    pattern: &NumberPatternValue,
    path: impl AsRef<Path>,
    cycle_count: u64,
) -> Result<(), EvalError> {
    if cycle_count == 0 {
        return Err(EvalError::new("exporting requires at least one cycle"));
    }

    let span = render_span(cycle_count)?;
    let mut events = pattern.try_query(&span)?;
    events.sort_unstable_by(|a, b| a.part.start().cmp(b.part.start()));

    let path = path.as_ref();
    let mut file = std::fs::File::create(path)?;

    writeln!(file, "<!DOCTYPE html>")?;
    writeln!(file, "<html lang=\"en\">")?;
    writeln!(file, "<head>")?;
    writeln!(file, "    <meta charset=\"UTF-8\">")?;
    writeln!(file, "    <title>Orpheus Web Audio Export</title>")?;
    writeln!(file, "    <style>")?;
    writeln!(
        file,
        "        body {{ font-family: sans-serif; background-color: #222; color: #eee; display: flex; flex-direction: column; align-items: center; justify-content: center; height: 100vh; margin: 0; }}"
    )?;
    writeln!(
        file,
        "        button {{ padding: 15px 30px; font-size: 20px; cursor: pointer; background-color: #4CAF50; color: white; border: none; border-radius: 5px; }}"
    )?;
    writeln!(
        file,
        "        button:hover {{ background-color: #45a049; }}"
    )?;
    writeln!(file, "    </style>")?;
    writeln!(file, "</head>")?;
    writeln!(file, "<body>")?;
    writeln!(file, "    <h1>Orpheus Playable Export</h1>")?;
    writeln!(file, "    <button id=\"play-btn\">Play Pattern</button>")?;
    writeln!(file, "    <script>")?;
    writeln!(
        file,
        "        const playButton = document.getElementById('play-btn');"
    )?;
    writeln!(
        file,
        "        playButton.addEventListener('click', async () => {{"
    )?;
    writeln!(
        file,
        "            const audioCtx = new (window.AudioContext || window.webkitAudioContext)();"
    )?;
    writeln!(
        file,
        "            const startTime = audioCtx.currentTime + 0.1;"
    )?;

    for event in &events {
        let start_s = f64::from(event.part.start()) * SECONDS_PER_CYCLE;
        let end_s = f64::from(event.part.end()) * SECONDS_PER_CYCLE;
        let freq = 440.0 * ((event.value - 69.0) / 12.0).exp2();

        writeln!(
            file,
            "            playTone(audioCtx, startTime, {start_s}, {end_s}, {freq});",
        )?;
    }

    writeln!(file, "            ")?;
    writeln!(
        file,
        "            function playTone(ctx, baseTime, start, end, freq) {{"
    )?;
    writeln!(file, "                const osc = ctx.createOscillator();")?;
    writeln!(file, "                const gainNode = ctx.createGain();")?;
    writeln!(file, "                osc.type = 'triangle';")?;
    writeln!(
        file,
        "                osc.frequency.setValueAtTime(freq, baseTime + start);"
    )?;
    writeln!(
        file,
        "                gainNode.gain.setValueAtTime(0.1, baseTime + start);"
    )?;
    writeln!(
        file,
        "                gainNode.gain.exponentialRampToValueAtTime(0.001, baseTime + end);"
    )?;
    writeln!(file, "                osc.connect(gainNode);")?;
    writeln!(file, "                gainNode.connect(ctx.destination);")?;
    writeln!(file, "                osc.start(baseTime + start);")?;
    writeln!(file, "                osc.stop(baseTime + end);")?;
    writeln!(file, "            }}")?;
    writeln!(file, "        }});")?;
    writeln!(file, "    </script>")?;
    writeln!(file, "</body>")?;
    writeln!(file, "</html>")?;

    Ok(())
}

/// Exports a sample pattern's evaluated events to an HTML file using Web Audio API.
///
/// Sample names (e.g., 'bd', 'sn') are synthesized procedurally in JavaScript
/// to mimic basic drum machine sounds, making the export standalone without
/// requiring external .wav files.
///
/// # Examples
///
/// ```no_run
/// use orpheus_lang::{ReplMode, eval_module};
/// use orpheus_lang::export_sample_pattern_to_webaudio;
///
/// let env = eval_module("x = bd sn", ReplMode::Loose).unwrap();
/// let pattern = env.get("x").unwrap().as_sample_pattern().unwrap();
///
/// let path = std::env::temp_dir().join("export_drum_audio.html");
/// export_sample_pattern_to_webaudio(pattern, &path, 2).unwrap();
/// ```
///
/// # Errors
///
/// Returns [`EvalError`] if pattern querying fails or if the file cannot be written.
#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::too_many_lines
)]
pub fn export_sample_pattern_to_webaudio(
    pattern: &SamplePatternValue,
    path: impl AsRef<Path>,
    cycle_count: u64,
) -> Result<(), EvalError> {
    if cycle_count == 0 {
        return Err(EvalError::new("exporting requires at least one cycle"));
    }

    let span = render_span(cycle_count)?;
    let mut events = pattern.try_query(&span)?;
    events.sort_unstable_by(|a, b| a.part.start().cmp(b.part.start()));

    let path = path.as_ref();
    let mut file = std::fs::File::create(path)?;

    writeln!(file, "<!DOCTYPE html>")?;
    writeln!(file, "<html lang=\"en\">")?;
    writeln!(file, "<head>")?;
    writeln!(file, "    <meta charset=\"UTF-8\">")?;
    writeln!(file, "    <title>Orpheus Web Audio Drums Export</title>")?;
    writeln!(file, "    <style>")?;
    writeln!(
        file,
        "        body {{ font-family: sans-serif; background-color: #222; color: #eee; display: flex; flex-direction: column; align-items: center; justify-content: center; height: 100vh; margin: 0; }}"
    )?;
    writeln!(
        file,
        "        button {{ padding: 15px 30px; font-size: 20px; cursor: pointer; background-color: #008CBA; color: white; border: none; border-radius: 5px; }}"
    )?;
    writeln!(
        file,
        "        button:hover {{ background-color: #007bb5; }}"
    )?;
    writeln!(file, "    </style>")?;
    writeln!(file, "</head>")?;
    writeln!(file, "<body>")?;
    writeln!(file, "    <h1>Orpheus Playable Drums</h1>")?;
    writeln!(file, "    <button id=\"play-btn\">Play Pattern</button>")?;
    writeln!(file, "    <script>")?;
    writeln!(
        file,
        "        const playButton = document.getElementById('play-btn');"
    )?;
    writeln!(
        file,
        "        playButton.addEventListener('click', async () => {{"
    )?;
    writeln!(
        file,
        "            const audioCtx = new (window.AudioContext || window.webkitAudioContext)();"
    )?;
    writeln!(
        file,
        "            const startTime = audioCtx.currentTime + 0.1;"
    )?;

    for event in &events {
        let start_s = f64::from(event.part.start()) * SECONDS_PER_CYCLE;
        let sample_name = event.value.sample();

        writeln!(
            file,
            "            playSample(audioCtx, startTime, {start_s}, '{sample_name}');",
        )?;
    }

    writeln!(file, "            ")?;
    writeln!(
        file,
        "            function playSample(ctx, baseTime, start, sampleName) {{"
    )?;
    writeln!(file, "                const time = baseTime + start;")?;
    writeln!(
        file,
        "                if (sampleName === 'bd' || sampleName === 'kick') {{"
    )?;
    writeln!(
        file,
        "                    const osc = ctx.createOscillator();"
    )?;
    writeln!(file, "                    const gain = ctx.createGain();")?;
    writeln!(file, "                    osc.connect(gain);")?;
    writeln!(file, "                    gain.connect(ctx.destination);")?;
    writeln!(
        file,
        "                    osc.frequency.setValueAtTime(150, time);"
    )?;
    writeln!(
        file,
        "                    osc.frequency.exponentialRampToValueAtTime(0.001, time + 0.5);"
    )?;
    writeln!(
        file,
        "                    gain.gain.setValueAtTime(1, time);"
    )?;
    writeln!(
        file,
        "                    gain.gain.exponentialRampToValueAtTime(0.001, time + 0.5);"
    )?;
    writeln!(file, "                    osc.start(time);")?;
    writeln!(file, "                    osc.stop(time + 0.5);")?;
    writeln!(
        file,
        "                }} else if (sampleName === 'sn' || sampleName === 'snare') {{"
    )?;
    writeln!(file, "                    // Noise buffer for snare")?;
    writeln!(
        file,
        "                    const bufferSize = ctx.sampleRate * 0.2;"
    )?;
    writeln!(
        file,
        "                    const buffer = ctx.createBuffer(1, bufferSize, ctx.sampleRate);"
    )?;
    writeln!(
        file,
        "                    const data = buffer.getChannelData(0);"
    )?;
    writeln!(
        file,
        "                    for (let i = 0; i < bufferSize; i++) data[i] = Math.random() * 2 - 1;"
    )?;
    writeln!(
        file,
        "                    const noise = ctx.createBufferSource();"
    )?;
    writeln!(file, "                    noise.buffer = buffer;")?;
    writeln!(
        file,
        "                    const filter = ctx.createBiquadFilter();"
    )?;
    writeln!(
        file,
        "                    filter.type = 'highpass'; filter.frequency.value = 1000;"
    )?;
    writeln!(file, "                    const gain = ctx.createGain();")?;
    writeln!(
        file,
        "                    noise.connect(filter); filter.connect(gain); gain.connect(ctx.destination);"
    )?;
    writeln!(
        file,
        "                    gain.gain.setValueAtTime(1, time);"
    )?;
    writeln!(
        file,
        "                    gain.gain.exponentialRampToValueAtTime(0.001, time + 0.2);"
    )?;
    writeln!(file, "                    noise.start(time);")?;
    writeln!(
        file,
        "                }} else if (sampleName === 'hh' || sampleName === 'hat') {{"
    )?;
    writeln!(
        file,
        "                    const bufferSize = ctx.sampleRate * 0.05;"
    )?;
    writeln!(
        file,
        "                    const buffer = ctx.createBuffer(1, bufferSize, ctx.sampleRate);"
    )?;
    writeln!(
        file,
        "                    const data = buffer.getChannelData(0);"
    )?;
    writeln!(
        file,
        "                    for (let i = 0; i < bufferSize; i++) data[i] = Math.random() * 2 - 1;"
    )?;
    writeln!(
        file,
        "                    const noise = ctx.createBufferSource();"
    )?;
    writeln!(file, "                    noise.buffer = buffer;")?;
    writeln!(
        file,
        "                    const filter = ctx.createBiquadFilter();"
    )?;
    writeln!(
        file,
        "                    filter.type = 'bandpass'; filter.frequency.value = 10000;"
    )?;
    writeln!(file, "                    const gain = ctx.createGain();")?;
    writeln!(
        file,
        "                    noise.connect(filter); filter.connect(gain); gain.connect(ctx.destination);"
    )?;
    writeln!(
        file,
        "                    gain.gain.setValueAtTime(0.3, time);"
    )?;
    writeln!(
        file,
        "                    gain.gain.exponentialRampToValueAtTime(0.001, time + 0.05);"
    )?;
    writeln!(file, "                    noise.start(time);")?;
    writeln!(file, "                }} else {{")?;
    writeln!(
        file,
        "                    // Default beep for unknown samples"
    )?;
    writeln!(
        file,
        "                    const osc = ctx.createOscillator();"
    )?;
    writeln!(file, "                    const gain = ctx.createGain();")?;
    writeln!(file, "                    osc.type = 'square';")?;
    writeln!(
        file,
        "                    osc.frequency.setValueAtTime(880, time);"
    )?;
    writeln!(
        file,
        "                    gain.gain.setValueAtTime(0.1, time);"
    )?;
    writeln!(
        file,
        "                    gain.gain.exponentialRampToValueAtTime(0.001, time + 0.1);"
    )?;
    writeln!(
        file,
        "                    osc.connect(gain); gain.connect(ctx.destination);"
    )?;
    writeln!(
        file,
        "                    osc.start(time); osc.stop(time + 0.1);"
    )?;
    writeln!(file, "                }}")?;
    writeln!(file, "            }}")?;
    writeln!(file, "        }});")?;
    writeln!(file, "    </script>")?;
    writeln!(file, "</body>")?;
    writeln!(file, "</html>")?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ReplMode, eval_module};

    #[test]
    fn export_number_pattern_webaudio_generates_valid_format() {
        let source = "x = 60 62 64";
        let module = eval_module(source, ReplMode::Loose).unwrap();
        let pattern = module.get("x").unwrap().as_number_pattern().unwrap();

        let path = std::env::temp_dir().join("test_webaudio_num.html");
        export_number_pattern_to_webaudio(pattern, &path, 1).unwrap();

        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains("<!DOCTYPE html>"));
        assert!(content.contains("AudioContext"));
        assert!(content.contains("playTone(audioCtx, startTime, "));
        assert!(content.contains("261.625")); // MIDI 60 Hz approx
    }

    #[test]
    fn export_sample_pattern_webaudio_generates_valid_format() {
        let source = "x = bd sn hh";
        let module = eval_module(source, ReplMode::Loose).unwrap();
        let pattern = module.get("x").unwrap().as_sample_pattern().unwrap();

        let path = std::env::temp_dir().join("test_webaudio_sample.html");
        export_sample_pattern_to_webaudio(pattern, &path, 1).unwrap();

        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains("<!DOCTYPE html>"));
        assert!(content.contains("AudioContext"));
        assert!(content.contains("playSample("));
        assert!(content.contains("'bd'"));
        assert!(content.contains("'sn'"));
    }

    #[test]
    fn export_cycle_count_zero_returns_error() {
        let module = eval_module("pat = bd sn", ReplMode::Loose).unwrap();
        let pat = module.get("pat").unwrap().as_sample_pattern().unwrap();

        assert_eq!(
            super::export_sample_pattern_to_webaudio(pat, "test.html", 0)
                .unwrap_err()
                .to_string(),
            "exporting requires at least one cycle"
        );

        let module = eval_module("pat = 60 62", ReplMode::Loose).unwrap();
        let pat = module.get("pat").unwrap().as_number_pattern().unwrap();

        assert_eq!(
            super::export_number_pattern_to_webaudio(pat, "test.html", 0)
                .unwrap_err()
                .to_string(),
            "exporting requires at least one cycle"
        );
    }
}
