//! The `webaudio_export` module provides an exporter to Web Audio API JavaScript.
//!
//! This exporter generates an `.html` file containing embedded JavaScript that
//! uses the browser's Web Audio API to play evaluated number patterns
//! (representing MIDI pitches) using oscillator nodes.

use std::io::Write;
use std::path::Path;

use crate::eval::{EvalError, render_span};
use crate::value::NumberPatternValue;

// Assume 120 BPM, 4 beats per cycle -> 1 cycle = 2.0 seconds.
const SECONDS_PER_CYCLE: f64 = 2.0;

/// Exports a number pattern's evaluated events to a playable HTML file using the Web Audio API.
///
/// Number patterns are assumed to represent pitch, and map to `OscillatorNode`
/// scheduling in the generated JavaScript.
///
/// # Examples
///
/// ```
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
    events.sort_unstable_by(|a, b| a.part.start().cmp(b.part.start()));

    let path = path.as_ref();
    let mut file = std::fs::File::create(path).map_err(|e| EvalError::new(e.to_string()))?;

    writeln!(file, "<!DOCTYPE html>")?;
    writeln!(file, "<html lang=\"en\">")?;
    writeln!(file, "<head>")?;
    writeln!(file, "    <meta charset=\"UTF-8\">")?;
    writeln!(file, "    <title>Orpheus Web Audio API Export</title>")?;
    writeln!(file, "    <style>")?;
    writeln!(
        file,
        "        body {{ font-family: monospace; text-align: center; margin-top: 50px; background-color: #1e1e1e; color: #fff; }}"
    )?;
    writeln!(
        file,
        "        button {{ padding: 15px 30px; font-size: 1.2rem; cursor: pointer; background-color: #2196F3; color: white; border: none; border-radius: 5px; }}"
    )?;
    writeln!(
        file,
        "        button:hover {{ background-color: #64B5F6; }}"
    )?;
    writeln!(
        file,
        "        button:disabled {{ background-color: #555; cursor: not-allowed; }}"
    )?;
    writeln!(file, "    </style>")?;
    writeln!(file, "</head>")?;
    writeln!(file, "<body>")?;
    writeln!(file, "    <h1>Orpheus Web Audio API Export</h1>")?;
    writeln!(file, "    <p>Cycles: {cycle_count}</p>")?;
    writeln!(file, "    <button id=\"playBtn\">Play Pattern</button>")?;
    writeln!(file, "    <script>")?;
    writeln!(
        file,
        "        const playBtn = document.getElementById('playBtn');"
    )?;
    writeln!(
        file,
        "        playBtn.addEventListener('click', async () => {{"
    )?;
    writeln!(file, "            playBtn.disabled = true;")?;
    writeln!(
        file,
        "            const ctx = new (window.AudioContext || window.webkitAudioContext)();"
    )?;
    writeln!(
        file,
        "            const t0 = ctx.currentTime + 0.1; // Small buffer"
    )?;

    for event in events {
        let start_time = f64::from(event.part.start()) * SECONDS_PER_CYCLE;
        let end_time = f64::from(event.part.end()) * SECONDS_PER_CYCLE;
        let pitch = event.value;
        let frequency = 440.0 * ((pitch - 69.0) / 12.0).exp2();

        writeln!(file, "            {{")?;
        writeln!(file, "                const osc = ctx.createOscillator();")?;
        writeln!(file, "                const gain = ctx.createGain();")?;
        writeln!(file, "                osc.type = 'sine';")?;
        writeln!(
            file,
            "                osc.frequency.value = {frequency:.3};"
        )?;
        writeln!(file, "                osc.connect(gain);")?;
        writeln!(file, "                gain.connect(ctx.destination);")?;
        // ADSR envelope to avoid clicks
        writeln!(
            file,
            "                gain.gain.setValueAtTime(0, t0 + {start_time:.3});"
        )?;
        writeln!(
            file,
            "                gain.gain.linearRampToValueAtTime(0.5, t0 + {start_time:.3} + 0.01);"
        )?;
        writeln!(
            file,
            "                gain.gain.setValueAtTime(0.5, t0 + {end_time:.3} - 0.01);"
        )?;
        writeln!(
            file,
            "                gain.gain.linearRampToValueAtTime(0, t0 + {end_time:.3});"
        )?;
        writeln!(file, "                osc.start(t0 + {start_time:.3});")?;
        writeln!(file, "                osc.stop(t0 + {end_time:.3});")?;
        writeln!(file, "            }}")?;
    }

    #[allow(clippy::cast_precision_loss)]
    let total_time = (cycle_count as f64) * SECONDS_PER_CYCLE;

    writeln!(file, "            setTimeout(() => {{")?;
    writeln!(file, "                playBtn.disabled = false;")?;
    writeln!(file, "            }}, ({total_time:.3} + 0.1) * 1000);")?;
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
    fn webaudio_exporter_generates_html() {
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
        assert!(content.contains("const ctx = new (window.AudioContext"));
        // C4 (60) is ~261.63 Hz
        assert!(content.contains("osc.frequency.value = 261.626;"));
        // D4 (62) is ~293.66 Hz
        assert!(content.contains("osc.frequency.value = 293.665;"));
        // E4 (64) is ~329.63 Hz
        assert!(content.contains("osc.frequency.value = 329.628;"));

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
