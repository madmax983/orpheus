#!/bin/bash
cat << 'INNER_EOF' > /tmp/merge.diff
<<<<<<< SEARCH
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
=======
fn export_pattern_events_to_csv<T>(
    events: impl IntoIterator<Item = Event<T>>,
    path: impl AsRef<Path>,
    header: &str,
    mut format_event: impl FnMut(&Event<T>) -> String,
) -> Result<(), EvalError> {
    let mut file = std::fs::File::create(path.as_ref()).map_err(|e| EvalError::new(e.to_string()))?;

    writeln!(
        file,
        "start_num,start_den,start_float,end_num,end_den,end_float,{header}"
    )
    .map_err(|e| EvalError::new(e.to_string()))?;

    for event in events {
        let start_float = f64::from(event.part.start());
        let end_float = f64::from(event.part.end());

        writeln!(
            file,
            "{},{},{:.6},{},{},{:.6},{}",
            event.part.start().numerator(),
            event.part.start().denominator(),
            start_float,
            event.part.end().numerator(),
            event.part.end().denominator(),
            end_float,
            format_event(&event)
        )
        .map_err(|e| EvalError::new(e.to_string()))?;
    }

    Ok(())
}

fn check_export_cycle_count(cycle_count: u64) -> Result<TimeSpan, EvalError> {
    if cycle_count == 0 {
        return Err(EvalError::new("exporting requires at least one cycle"));
    }
    render_span(cycle_count)
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
    let span = check_export_cycle_count(cycle_count)?;
    let events = pattern.try_query(&span)?;

    export_pattern_events_to_csv(
        events,
        path,
        "sample,gain,pan,rate,hpf_cutoff_hz,lpf_cutoff_hz",
        |event| {
            let hpf = event
                .value
                .hpf_cutoff_hz()
                .map_or_else(String::new, |v| format!("{v:.6}"));
            let lpf = event
                .value
                .lpf_cutoff_hz()
                .map_or_else(String::new, |v| format!("{v:.6}"));
            format!(
                "{},{:.6},{:.6},{:.6},{},{}",
                event.value.sample(),
                event.value.gain(),
                event.value.pan(),
                event.value.rate(),
                hpf,
                lpf
            )
        },
    )
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
    let span = check_export_cycle_count(cycle_count)?;
    let events = pattern.try_query(&span)?;

    export_pattern_events_to_csv(events, path, "value", |event| format!("{:.6}", event.value))
}
>>>>>>> REPLACE
INNER_EOF
