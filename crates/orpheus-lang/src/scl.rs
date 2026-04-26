//! Scala `.scl` tuning-file parser.
//!
//! The Scala format is the de-facto standard for sharing microtonal scales and
//! is recognized by every major piece of microtonal software. A file contains:
//!
//! 1. Zero or more `!` comment lines (and blank lines) that are ignored.
//! 2. A description line (free-form text, may be empty).
//! 3. A note-count integer `N`.
//! 4. `N` entries, each either a ratio (`numerator/denominator`), a plain
//!    integer (implicit `/1`), or a cents value (any line containing a `.`).
//!
//! The last entry is the scale's period; for octave-periodic scales it is the
//! ratio `2/1`. Phase 1 rejects other periods.
//!
//! See the [Scala format reference](https://www.huygens-fokker.org/scala/scl_format.html)
//! for the complete specification.

use std::fs;
use std::io;
use std::path::Path;

use crate::eval::EvalError;
use crate::value::{TUNING_OCTAVE_PERIOD, TuningValue};

/// Errors surfaced while parsing a Scala `.scl` file.
#[derive(Debug, thiserror::Error)]
pub enum SclError {
    /// The file or stream could not be read.
    #[error("failed to read Scala source: {0}")]
    Io(#[from] io::Error),
    /// The header is malformed (missing description, count, or count mismatch).
    #[error("malformed Scala header: {0}")]
    Header(String),
    /// An entry line could not be parsed as a ratio, integer, or cents value.
    #[error("invalid Scala entry `{0}`: {1}")]
    Entry(String, String),
    /// The entry count does not match the number of entries.
    #[error("Scala file note count mismatch: header declared {expected} entries but got {actual}")]
    Count { expected: usize, actual: usize },
    /// Entries violate tuning invariants (non-monotone, non-positive, bad period).
    #[error("invalid Scala tuning: {0}")]
    Invariant(String),
}

impl From<SclError> for EvalError {
    fn from(error: SclError) -> Self {
        Self::new(error.to_string())
    }
}

/// Parses a Scala `.scl` file from disk.
///
/// The file stem (e.g. `"just_intonation"` from `just_intonation.scl`) becomes
/// the returned tuning's name.
///
/// # Examples
///
/// ```no_run
/// use std::path::Path;
/// use orpheus_lang::parse_scala_file;
///
/// let path = Path::new("just_intonation.scl");
/// let tuning = parse_scala_file(path).unwrap();
/// assert_eq!(tuning.name(), "just_intonation");
/// ```
///
/// # Errors
///
/// Returns [`SclError::Io`] if the path cannot be read, and any of the other
/// [`SclError`] variants if the content is malformed.
pub fn parse_scala_file(path: &Path) -> Result<TuningValue, SclError> {
    let source = fs::read_to_string(path)?;
    let name = path
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or("tuning");
    parse_scala_source(&source, name)
}

/// Parses a Scala `.scl` document from an in-memory string.
///
/// # Examples
///
/// ```
/// use orpheus_lang::parse_scala_source;
///
/// let source = "\
/// ! A test scale
/// Test scale
/// 3
/// 5/4
/// 3/2
/// 2/1
/// ";
/// let tuning = parse_scala_source(source, "test").unwrap();
/// assert_eq!(tuning.ratios().len(), 4); // 1.0 is implicitly added
/// assert_eq!(tuning.period(), 2.0);
/// ```
///
/// # Errors
///
/// Returns an [`SclError`] on malformed header, count mismatch, unparseable
/// entries, or invariant violations (e.g., non-monotone ratios, period != 2/1).
pub fn parse_scala_source(source: &str, name: &str) -> Result<TuningValue, SclError> {
    let mut logical = source.lines().filter(|line| {
        let trimmed = line.trim_start();
        !(trimmed.starts_with('!') || trimmed.is_empty())
    });

    let _description = logical
        .next()
        .ok_or_else(|| SclError::Header("missing description line".into()))?;

    let count_line = logical
        .next()
        .ok_or_else(|| SclError::Header("missing note-count line".into()))?;
    let expected: usize = count_line
        .trim()
        .parse()
        .map_err(|_| SclError::Header(format!("invalid note count `{count_line}`")))?;

    let entries: Vec<&str> = logical.map(str::trim).collect();

    if entries.len() != expected {
        return Err(SclError::Count {
            expected,
            actual: entries.len(),
        });
    }

    let mut ratios = Vec::with_capacity(expected + 1);
    ratios.push(1.0);
    for entry in &entries[..entries.len().saturating_sub(1)] {
        ratios.push(parse_scala_entry(entry)?);
    }
    let period = parse_scala_entry(entries.last().copied().unwrap_or(""))?;

    if (period - TUNING_OCTAVE_PERIOD).abs() > f64::EPSILON {
        return Err(SclError::Invariant(format!(
            "period must be 2/1 (octave); got {period}"
        )));
    }

    TuningValue::new(name, ratios, period).map_err(|e| SclError::Invariant(e.to_string()))
}

fn parse_scala_entry(raw: &str) -> Result<f64, SclError> {
    let first_field = raw
        .split_whitespace()
        .next()
        .ok_or_else(|| SclError::Entry(raw.into(), "empty line".into()))?;

    if let Some((num, den)) = first_field.split_once('/') {
        let numerator: f64 = num
            .parse()
            .map_err(|_| SclError::Entry(raw.into(), "numerator must be a number".into()))?;
        let denominator: f64 = den
            .parse()
            .map_err(|_| SclError::Entry(raw.into(), "denominator must be a number".into()))?;
        if denominator == 0.0 {
            return Err(SclError::Entry(raw.into(), "zero denominator".into()));
        }
        Ok(numerator / denominator)
    } else if first_field.contains('.') {
        let cents: f64 = first_field
            .parse()
            .map_err(|_| SclError::Entry(raw.into(), "cents must be numeric".into()))?;
        Ok((cents / 1200.0).exp2())
    } else {
        let integer: f64 = first_field.parse().map_err(|_| {
            SclError::Entry(raw.into(), "entry must be ratio, cents, or integer".into())
        })?;
        Ok(integer)
    }
}
