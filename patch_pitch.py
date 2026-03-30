with open("crates/orpheus-lang/src/pitch.rs", "r") as f:
    data = f.read()

old_block = """    let octave = octave_suffix.parse::<i32>().map_err(|_| {
        PitchLiteralError::new(format!(
            "named pitch literal `{token}` exceeded the supported octave range"
        ))
    })?;
    let absolute = (octave + 1) * 12 + base_pitch_class + accidental;"""

new_block = """    let octave = octave_suffix.parse::<i32>().map_err(|_| {
        PitchLiteralError::new(format!(
            "named pitch literal `{token}` exceeded the supported octave range"
        ))
    })?;
    let absolute = octave
        .checked_add(1)
        .and_then(|value| value.checked_mul(12))
        .and_then(|value| value.checked_add(base_pitch_class))
        .and_then(|value| value.checked_add(accidental))
        .ok_or_else(|| {
            PitchLiteralError::new(format!(
                "named pitch literal `{token}` exceeded the supported evaluator range"
            ))
        })?;"""

# Oh, looking at my history, I had already fixed the absolute parsing in `crates/orpheus-lang/src/pitch.rs` using checked_add and checked_mul.
# The `git diff` showed no changes when I checked, because I had accidentally run `git reset` or `git commit -am` and it cleared my staging but committed it correctly!
# Let me double check if the previous commit had the files.
