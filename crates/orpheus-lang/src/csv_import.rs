//! The `csv_import` module provides a CSV importer to create patterns from explicit CSV tabular data.
//!
//! This importer enables bidirectional workflows, allowing users to export a pattern to CSV,
//! modify it in a spreadsheet, and then import it back into Orpheus as a usable explicit-time pattern.

use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;

use orpheus_pattern::{Event, Rational, TimeSpan};

use crate::error::EvalError;
use crate::value::{PatternValueTransform, SampleEvent, SamplePatternValue};

/// Imports a sample pattern from a CSV file.
///
/// The CSV must have a header. Expected columns:
/// `start_num`, `start_den`, `start_float`, `end_num`, `end_den`, `end_float`, `sample`, `gain`, `pan`, `rate`, `hpf`, `lpf`
/// Missing parameters will fallback to defaults (e.g. gain: 1.0).
///
/// # Examples
///
/// ```
/// use orpheus_lang::import_sample_pattern_from_csv;
/// use std::io::Write;
///
/// let path = std::env::temp_dir().join("import_test.csv");
/// let mut file = std::fs::File::create(&path).unwrap();
/// writeln!(file, "start_num,start_den,start_float,end_num,end_den,end_float,sample,gain,pan,rate,hpf,lpf").unwrap();
/// writeln!(file, "0,1,0.0,1,2,0.5,bd,1.0,0.0,1.0,,").unwrap();
/// writeln!(file, "1,2,0.5,1,1,1.0,sn,1.0,0.0,1.0,,").unwrap();
/// file.flush().unwrap();
///
/// let pattern = import_sample_pattern_from_csv(&path).unwrap();
/// ```
///
/// # Errors
///
/// Returns an [`EvalError`] if the CSV cannot be parsed or the file cannot be read.
pub fn import_sample_pattern_from_csv(
    path: impl AsRef<Path>,
) -> Result<SamplePatternValue, EvalError> {
    let file = File::open(path).map_err(|e| EvalError::new(e.to_string()))?;
    let reader = BufReader::new(file);
    let mut lines = reader.lines();

    let header = lines
        .next()
        .ok_or_else(|| EvalError::new("CSV file is empty or missing header"))?
        .map_err(|e| EvalError::new(e.to_string()))?;

    if !header.contains("sample") {
        return Err(EvalError::new("CSV file missing required columns (sample)"));
    }

    let mut events = Vec::new();

    for (line_idx, line) in lines.enumerate() {
        let line = line.map_err(|e| EvalError::new(e.to_string()))?;
        if line.trim().is_empty() {
            continue;
        }

        let parts: Vec<&str> = line.split(',').map(str::trim).collect();
        if parts.len() < 7 {
            return Err(EvalError::new(format!(
                "CSV parse error at line {}: not enough columns",
                line_idx + 2
            )));
        }

        let start_num: i64 = parts[0].parse().unwrap_or(0);
        let start_den: i64 = parts[1].parse().unwrap_or(1);
        let end_num: i64 = parts[3].parse().unwrap_or(0);
        let end_den: i64 = parts[4].parse().unwrap_or(1);

        let sample = parts[6].to_owned();
        let gain: f64 = parts.get(7).and_then(|s| s.parse().ok()).unwrap_or(1.0);
        let pan: f64 = parts.get(8).and_then(|s| s.parse().ok()).unwrap_or(0.0);
        let rate: f64 = parts.get(9).and_then(|s| s.parse().ok()).unwrap_or(1.0);

        let hpf = parts.get(10).and_then(|s| s.parse::<f64>().ok());
        let lpf = parts.get(11).and_then(|s| s.parse::<f64>().ok());

        let start_rational = Rational::new(start_num, start_den).unwrap_or(Rational::zero());
        let end_rational = Rational::new(end_num, end_den).unwrap_or(Rational::one());
        let part_span = TimeSpan::new(start_rational, end_rational)
            .map_err(|e| EvalError::new(e.to_string()))?;

        let mut event_val = SampleEvent::named(&sample)
            .adjust_gain(gain)
            .adjust_pan(pan)
            .adjust_rate(rate);
        if let Some(h) = hpf {
            event_val = event_val.adjust_hpf(h);
        }
        if let Some(l) = lpf {
            event_val = event_val.adjust_lpf(l);
        }

        events.push(Event {
            whole: Some(part_span),
            part: part_span,
            value: event_val,
        });
    }

    Ok(SamplePatternValue::from_events(events))
}
