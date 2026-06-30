//! The `web_audio_export` module provides an exporter to an HTML file
//! that plays patterns using the browser's Web Audio API.

use std::io::Write;
use std::path::Path;

use crate::error::EvalError;
use crate::eval::render_span;
use crate::value::NumberPatternValue;

/// Exports a number pattern's evaluated events to an HTML file using the Web Audio API.
///
/// # Errors
///
/// Returns [`EvalError`] if pattern querying fails or if the file cannot be written.
pub fn export_number_pattern_to_web_audio(
    pattern: &NumberPatternValue,
    path: impl AsRef<Path>,
    cycle_count: u64,
) -> Result<(), EvalError> {
    if cycle_count == 0 {
        return Err(EvalError::new("exporting requires at least one cycle"));
    }

    let span = render_span(cycle_count)?;
    let events = pattern.try_query(&span)?;

    let path = path.as_ref();
    let mut file = std::fs::File::create(path).map_err(|e| EvalError::new(e.to_string()))?;

    let cycle_duration = 2.0;

    writeln!(file, "<!DOCTYPE html>")?;
    writeln!(
        file,
        "<html><head><title>Orpheus Web Audio API Export</title></head><body>"
    )?;
    writeln!(file, "<h1>Orpheus Web Audio API Export</h1>")?;
    writeln!(file, "<button id=\"play\">Play</button>")?;
    writeln!(file, "<script>")?;
    writeln!(
        file,
        "document.getElementById('play').addEventListener('click', async () => {{"
    )?;
    writeln!(
        file,
        "  const ctx = new (window.AudioContext || window.webkitAudioContext)();"
    )?;

    for event in events {
        let start_time = f64::from(*event.part.start()) * cycle_duration;
        let end_time = f64::from(*event.part.end()) * cycle_duration;
        let pitch = event.value;
        let freq = 440.0 * ((pitch - 69.0) / 12.0).exp2();

        writeln!(file, "  (() => {{")?;
        writeln!(file, "    const osc = ctx.createOscillator();")?;
        writeln!(file, "    const gain = ctx.createGain();")?;
        writeln!(file, "    osc.type = 'sine';")?;
        writeln!(
            file,
            "    osc.frequency.setValueAtTime({freq:.3}, ctx.currentTime + {start_time:.3});"
        )?;
        writeln!(
            file,
            "    gain.gain.setValueAtTime(0.1, ctx.currentTime + {start_time:.3});"
        )?;
        writeln!(
            file,
            "    gain.gain.exponentialRampToValueAtTime(0.001, ctx.currentTime + {end_time:.3});"
        )?;
        writeln!(file, "    osc.connect(gain);")?;
        writeln!(file, "    gain.connect(ctx.destination);")?;
        writeln!(file, "    osc.start(ctx.currentTime + {start_time:.3});")?;
        writeln!(
            file,
            "    osc.stop(ctx.currentTime + {end_time:.3} + 0.05);"
        )?;
        writeln!(file, "  }})();")?;
    }

    writeln!(file, "}});")?;
    writeln!(file, "</script>")?;
    writeln!(file, "</body></html>")?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ReplMode;
    use crate::eval::eval_module;

    #[test]
    fn web_audio_exporter_generates_number_pattern() {
        let source = "pattern = 60 62 64";
        let module = eval_module(source, ReplMode::Loose).unwrap();
        let pattern = module.get("pattern").unwrap().as_number_pattern().unwrap();

        let path = std::env::temp_dir().join("test_web_audio_output.html");
        export_number_pattern_to_web_audio(pattern, &path, 1).unwrap();

        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains("Orpheus Web Audio API Export"));
        assert!(
            content
                .contains("const ctx = new (window.AudioContext || window.webkitAudioContext)();")
        );
        assert!(content.contains("261.626"));
    }

    #[test]
    fn export_cycle_count_zero_returns_error() {
        let module = eval_module("pat = 1 2", ReplMode::Loose).unwrap();
        let pat = module.get("pat").unwrap().as_number_pattern().unwrap();

        assert_eq!(
            export_number_pattern_to_web_audio(pat, "test.html", 0)
                .unwrap_err()
                .to_string(),
            "exporting requires at least one cycle"
        );
    }
}
