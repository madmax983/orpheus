use std::error::Error;
use std::fmt::{self, Display, Formatter};
use std::io::Write;
use std::path::Path;

use orpheus_dsp::{OfflineRenderError, SampleBank, render_events_to_file_with_bank};
use orpheus_pattern::Event;

use crate::eval::{EvalError, render_span, sample_trigger_from_event};
use crate::value::{NumberPatternValue, SamplePatternValue};

/// Error raised while rendering an Orpheus sample pattern to an audio file.
#[derive(Debug)]
pub enum RenderError {
    Eval(EvalError),
    Audio(OfflineRenderError),
}

impl Display for RenderError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Eval(error) => Display::fmt(error, formatter),
            Self::Audio(error) => Display::fmt(error, formatter),
        }
    }
}

impl Error for RenderError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Eval(error) => Some(error),
            Self::Audio(error) => Some(error),
        }
    }
}

impl From<EvalError> for RenderError {
    fn from(error: EvalError) -> Self {
        Self::Eval(error)
    }
}

impl From<OfflineRenderError> for RenderError {
    fn from(error: OfflineRenderError) -> Self {
        Self::Audio(error)
    }
}

/// Renders a sample pattern to a deterministic stereo audio file selected by
/// the target extension.
///
/// Supported extensions:
/// - `.wav`
/// - `.flac`
///
/// # Errors
///
/// Returns [`RenderError`] if pattern querying fails or if the offline audio
/// renderer cannot write the target file.
pub fn render_sample_pattern_to_file(
    pattern: &SamplePatternValue,
    path: impl AsRef<Path>,
    cycle_count: u64,
) -> Result<(), RenderError> {
    let sample_bank = SampleBank::load_builtin();
    render_sample_pattern_to_file_with_bank(pattern, path, cycle_count, &sample_bank)
}

/// Renders a sample pattern to a deterministic stereo WAV file.
///
/// # Errors
///
/// Returns [`RenderError`] if pattern querying fails or if the offline audio
/// renderer cannot write the target WAV file.
pub fn render_sample_pattern_to_wav(
    pattern: &SamplePatternValue,
    path: impl AsRef<Path>,
    cycle_count: u64,
) -> Result<(), RenderError> {
    render_sample_pattern_to_file(pattern, path, cycle_count)
}

/// Renders a sample pattern to a deterministic stereo audio file using the
/// supplied sample bank overrides.
///
/// # Errors
///
/// Returns [`RenderError`] if pattern querying fails or if the offline audio
/// renderer cannot write the target file.
pub fn render_sample_pattern_to_file_with_bank(
    pattern: &SamplePatternValue,
    path: impl AsRef<Path>,
    cycle_count: u64,
    sample_bank: &SampleBank,
) -> Result<(), RenderError> {
    if cycle_count == 0 {
        return Err(EvalError::new("rendering requires at least one cycle").into());
    }

    let span = render_span(cycle_count)?;
    let events = pattern.try_query(&span)?;
    let rendered_events = events
        .into_iter()
        .map(|event| Event {
            whole: event.whole,
            part: event.part,
            value: sample_trigger_from_event(&event.value),
        })
        .collect::<Vec<_>>();

    render_events_to_file_with_bank(path, &rendered_events, cycle_count, sample_bank)?;
    Ok(())
}

/// Exports a sample pattern's evaluated events to a CSV file.
///
/// # Errors
///
/// Returns [`EvalError`] if pattern querying fails or if the file cannot be written.
pub fn export_sample_pattern_to_csv(
    pattern: &SamplePatternValue,
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
    writeln!(
        file,
        "start_num,start_den,start_float,end_num,end_den,end_float,sample,gain,pan,rate,hpf_cutoff_hz,lpf_cutoff_hz"
    )
    .map_err(|e| EvalError::new(e.to_string()))?;

    for event in events {
        let start_float = f64::from(event.part.start());
        let end_float = f64::from(event.part.end());
        let hpf = event
            .value
            .hpf_cutoff_hz()
            .map_or_else(String::new, |v| format!("{v:.6}"));
        let lpf = event
            .value
            .lpf_cutoff_hz()
            .map_or_else(String::new, |v| format!("{v:.6}"));
        writeln!(
            file,
            "{},{},{:.6},{},{},{:.6},{},{:.6},{:.6},{:.6},{},{}",
            event.part.start().numerator(),
            event.part.start().denominator(),
            start_float,
            event.part.end().numerator(),
            event.part.end().denominator(),
            end_float,
            event.value.sample(),
            event.value.gain(),
            event.value.pan(),
            event.value.rate(),
            hpf,
            lpf
        )
        .map_err(|e| EvalError::new(e.to_string()))?;
    }

    Ok(())
}

/// Exports a number pattern's evaluated events to a CSV file.
///
/// # Errors
///
/// Returns [`EvalError`] if pattern querying fails or if the file cannot be written.
pub fn export_number_pattern_to_csv(
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
    writeln!(
        file,
        "start_num,start_den,start_float,end_num,end_den,end_float,value"
    )
    .map_err(|e| EvalError::new(e.to_string()))?;

    for event in events {
        let start_float = f64::from(event.part.start());
        let end_float = f64::from(event.part.end());
        writeln!(
            file,
            "{},{},{:.6},{},{},{:.6},{:.6}",
            event.part.start().numerator(),
            event.part.start().denominator(),
            start_float,
            event.part.end().numerator(),
            event.part.end().denominator(),
            end_float,
            event.value
        )
        .map_err(|e| EvalError::new(e.to_string()))?;
    }

    Ok(())
}
