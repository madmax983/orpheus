use std::io::Write;
use std::path::Path;

use crate::eval::{EvalError, render_span};
use crate::value::NumberPatternValue;

// Assume 120 BPM, 4 beats per cycle -> 1 cycle = 2.0 seconds = 2000 ms.
const SECONDS_PER_CYCLE: f64 = 2.0;

/// Converts a MIDI note number to its corresponding frequency in Hertz.
#[must_use]
pub fn midi_to_hz(midi: f64) -> f64 {
    440.0 * ((midi - 69.0) / 12.0).exp2()
}

/// Exports a number pattern's evaluated events to an HTML file with Web Audio API synthesis.
///
/// Number patterns are assumed to represent pitch, and map to `OscillatorNode` calls
/// in the Web Audio context.
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
/// let path = std::env::temp_dir().join("export_webaudio.html");
/// export_number_pattern_to_webaudio(pattern, &path, 2).unwrap();
/// ```
///
/// # Errors
///
/// Returns [`EvalError`] if pattern querying fails or if the file cannot be written.
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
    events.sort_by(|a, b| a.part.start().cmp(b.part.start()));

    let path = path.as_ref();
    let mut file = std::fs::File::create(path).map_err(|e| EvalError::new(e.to_string()))?;

    writeln!(
        file,
        "<!DOCTYPE html>\n<html lang=\"en\">\n<head>\n<meta charset=\"UTF-8\">\n<title>Orpheus Web Audio Export</title>\n<style>\nbody {{ background-color: #1e1e1e; color: #ffffff; font-family: monospace; padding: 20px; }}\nbutton {{ font-size: 1.2rem; padding: 10px 20px; cursor: pointer; background-color: #2196F3; color: white; border: none; border-radius: 4px; }}\nbutton:hover {{ background-color: #64B5F6; }}\n</style>\n</head>\n<body>\n<h2>Orpheus Web Audio Synthesizer</h2>\n<button id=\"playBtn\">Play Sequence</button>\n<script>"
    )?;

    writeln!(file, "const events = [")?;
    for (i, event) in events.iter().enumerate() {
        let start_sec = f64::from(event.part.start()) * SECONDS_PER_CYCLE;
        let end_sec = f64::from(event.part.end()) * SECONDS_PER_CYCLE;
        let freq = midi_to_hz(event.value);

        let comma = if i < events.len() - 1 { "," } else { "" };
        writeln!(
            file,
            "  {{ start: {start_sec:.4}, end: {end_sec:.4}, freq: {freq:.2} }}{comma}",
        )?;
    }
    writeln!(file, "];")?;

    writeln!(
        file,
        r#"
let isPlaying = false;
document.getElementById("playBtn").addEventListener("click", async () => {{
    if (isPlaying) return;
    isPlaying = true;

    const AudioContext = window.AudioContext || window.webkitAudioContext;
    const ctx = new AudioContext();
    const startTime = ctx.currentTime + 0.1;

    events.forEach(event => {{
        const osc = ctx.createOscillator();
        const gain = ctx.createGain();

        osc.type = "triangle";
        osc.frequency.value = event.freq;

        // Envelope
        gain.gain.setValueAtTime(0, startTime + event.start);
        gain.gain.linearRampToValueAtTime(0.3, startTime + event.start + 0.05); // Attack
        gain.gain.linearRampToValueAtTime(0.0, startTime + event.end); // Release

        osc.connect(gain);
        gain.connect(ctx.destination);

        osc.start(startTime + event.start);
        osc.stop(startTime + event.end);
    }});

    const lastEvent = events[events.length - 1];
    setTimeout(() => {{
        isPlaying = false;
        ctx.close();
    }}, (lastEvent ? lastEvent.end : 0) * 1000 + 500);
}});
"#
    )?;

    writeln!(file, "</script>\n</body>\n</html>")?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ReplMode, eval_module};
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    static UNIQUE_TEMP_ID: AtomicU64 = AtomicU64::new(0);

    fn unique_temp_suffix() -> String {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let counter = UNIQUE_TEMP_ID.fetch_add(1, Ordering::Relaxed);
        format!("{timestamp}-{counter}")
    }

    #[test]
    fn test_webaudio_exporter_generates_html() {
        let source = "pattern = 60 62 64";
        let module = eval_module(source, ReplMode::Loose).unwrap();
        let pattern = module.get("pattern").unwrap().as_number_pattern().unwrap();

        let path = std::env::temp_dir().join(format!(
            "test_webaudio_output_{}.html",
            unique_temp_suffix()
        ));
        export_number_pattern_to_webaudio(pattern, &path, 1).unwrap();

        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains("<!DOCTYPE html>"));
        assert!(content.contains("AudioContext"));
        assert!(content.contains("261.6")); // freq for 60
        assert!(content.contains("293.6")); // freq for 62

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn export_cycle_count_zero_returns_error() {
        let module = eval_module("pat = 1 2", ReplMode::Loose).unwrap();
        let pat = module.get("pat").unwrap().as_number_pattern().unwrap();

        assert_eq!(
            super::export_number_pattern_to_webaudio(pat, "test.html", 0)
                .unwrap_err()
                .to_string(),
            "exporting requires at least one cycle"
        );
    }
}
