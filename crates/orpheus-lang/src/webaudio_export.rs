//! The `webaudio_export` module provides an exporter to an interactive Web Audio API HTML file.
//!
//! This exporter generates a standalone HTML file containing JavaScript that uses
//! the Web Audio API to schedule and synthesize evaluated number patterns (pitches)
//! directly in the browser, without needing external samples.

use std::io::Write;
use std::path::Path;

use crate::eval::{EvalError, render_span};
use crate::value::NumberPatternValue;

// Assume 120 BPM, 4 beats per cycle -> 1 cycle = 2.0 seconds.
const SECONDS_PER_CYCLE: f64 = 2.0;

/// Converts a MIDI note number to its corresponding frequency in Hertz.
#[must_use]
pub fn midi_to_hz(midi: f64) -> f64 {
    440.0 * ((midi - 69.0) / 12.0).exp2()
}

/// Exports a number pattern's evaluated events to an interactive HTML file with Web Audio API.
///
/// Number patterns are assumed to represent pitch (MIDI note numbers), and map to
/// scheduled oscillator nodes in the browser's `AudioContext`.
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
#[allow(clippy::too_many_lines)]
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

    writeln!(file, "<!DOCTYPE html>")?;
    writeln!(file, "<html lang=\"en\">")?;
    writeln!(file, "<head>")?;
    writeln!(file, "    <meta charset=\"UTF-8\">")?;
    writeln!(file, "    <title>Orpheus Web Audio Export</title>")?;
    writeln!(file, "    <style>")?;
    writeln!(
        file,
        "        body {{ font-family: monospace; background: #1e1e1e; color: #fff; padding: 2rem; }}"
    )?;
    writeln!(
        file,
        "        button {{ padding: 1rem 2rem; font-size: 1.2rem; cursor: pointer; background: #4CAF50; color: white; border: none; border-radius: 4px; }}"
    )?;
    writeln!(file, "        button:hover {{ background: #45a049; }}")?;
    writeln!(
        file,
        "        button:disabled {{ background: #666; cursor: not-allowed; }}"
    )?;
    writeln!(file, "    </style>")?;
    writeln!(file, "</head>")?;
    writeln!(file, "<body>")?;
    writeln!(file, "    <h1>Orpheus Web Audio Player</h1>")?;
    writeln!(file, "    <p>Cycles: {cycle_count}</p>")?;
    writeln!(file, "    <button id=\"playBtn\">Play Pattern</button>")?;
    writeln!(file, "    <script>")?;
    writeln!(
        file,
        "        document.getElementById('playBtn').addEventListener('click', async () => {{"
    )?;
    writeln!(
        file,
        "            const btn = document.getElementById('playBtn');"
    )?;
    writeln!(file, "            btn.disabled = true;")?;
    writeln!(
        file,
        "            const ctx = new (window.AudioContext || window.webkitAudioContext)();"
    )?;
    writeln!(file, "            const startTime = ctx.currentTime + 0.1;")?;
    writeln!(file, "            const gainNode = ctx.createGain();")?;
    writeln!(file, "            gainNode.gain.value = 0.2;")?; // General mix volume
    writeln!(file, "            gainNode.connect(ctx.destination);")?;

    let mut end_time_offset = 0.0;

    for event in &events {
        let start_ms = f64::from(event.part.start()) * SECONDS_PER_CYCLE;
        let end_ms = f64::from(event.part.end()) * SECONDS_PER_CYCLE;
        let freq = midi_to_hz(event.value);

        if end_ms > end_time_offset {
            end_time_offset = end_ms;
        }

        writeln!(file, "            {{")?;
        writeln!(file, "                let osc = ctx.createOscillator();")?;
        writeln!(file, "                let env = ctx.createGain();")?;
        writeln!(file, "                osc.type = 'triangle';")?;
        writeln!(
            file,
            "                osc.frequency.setValueAtTime({freq:.2}, startTime + {start_ms:.3});"
        )?;
        writeln!(file, "                osc.connect(env);")?;
        writeln!(file, "                env.connect(gainNode);")?;

        // Simple ADSR envelope to avoid clicks
        writeln!(
            file,
            "                env.gain.setValueAtTime(0, startTime + {start_ms:.3});"
        )?;
        writeln!(
            file,
            "                env.gain.linearRampToValueAtTime(1, startTime + {start_ms:.3} + 0.01);"
        )?;
        writeln!(
            file,
            "                env.gain.setValueAtTime(1, startTime + {end_ms:.3} - 0.02);"
        )?;
        writeln!(
            file,
            "                env.gain.linearRampToValueAtTime(0, startTime + {end_ms:.3});"
        )?;

        writeln!(
            file,
            "                osc.start(startTime + {start_ms:.3});"
        )?;
        writeln!(file, "                osc.stop(startTime + {end_ms:.3});")?;
        writeln!(file, "            }}")?;
    }

    writeln!(file, "            setTimeout(() => {{")?;
    writeln!(file, "                btn.disabled = false;")?;
    writeln!(file, "                ctx.close();")?;
    writeln!(
        file,
        "            }}, (0.1 + {end_time_offset:.3}) * 1000);"
    )?;
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
    fn webaudio_exporter_generates_html_file() {
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
        assert!(
            content
                .contains("const ctx = new (window.AudioContext || window.webkitAudioContext)();")
        );
        assert!(content.contains("osc.frequency.setValueAtTime(261.6")); // ~261.6Hz for C4 (MIDI 60)
        assert!(content.contains("osc.start("));

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
