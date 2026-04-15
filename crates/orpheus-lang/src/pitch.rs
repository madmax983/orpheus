//! The `pitch` module parses musical pitch notation.
//!
//! This module translates human-readable musical note strings (like `"c4"`,
//! `"fs4"`, `"bf3"`) into integer MIDI note numbers or offsets.
use thiserror::Error;

/// An error that occurs when a string fails to parse as a pitch literal.
///
/// Thrown when the literal has invalid characters (like `"c#4"` instead of `"cs4"`),
/// is an uppercase spelling (like `"C4"` instead of `"c4"`), or contains an
/// unparseable octave suffix.
///
/// # Causes
///
/// Pitch literals must exactly match Orpheus's required lowercase format:
/// `[note][accidental][octave]`.
/// - The note must be `a` through `g`.
/// - The accidental must be `s` (sharp) or `f` (flat). Traditional `#` or `b`
///   are not valid.
/// - The octave must be a parseable integer.
/// - Using uppercase letters or invalid accidentals returns this error.
///
/// # Examples
///
/// ```
/// use orpheus_lang::parse_named_pitch_literal;
///
/// // Missing octave after an accidental is rejected.
/// let error = parse_named_pitch_literal("cf").unwrap_err();
/// assert!(error.to_string().contains("missing an octave suffix"));
///
/// // Uppercase notes are not supported.
/// let error = parse_named_pitch_literal("C4").unwrap_err();
/// assert!(error.to_string().contains("lowercase ASCII"));
/// ```

#[derive(Clone, Debug, Eq, PartialEq, Error)]
#[error("{message}")]
pub struct PitchLiteralError {
    message: Box<str>,
}

impl PitchLiteralError {
    /// Constructs a new `PitchLiteralError` from the provided message string.
    ///
    /// # Examples
    ///
    /// ```
    /// use orpheus_lang::PitchLiteralError;
    ///
    /// let error = PitchLiteralError::new("invalid note parsing");
    /// assert_eq!(error.to_string(), "invalid note parsing");
    /// ```
    pub fn new(message: impl Into<Box<str>>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

/// Parses a named musical pitch literal into an integer MIDI offset.
///
/// Converts a string like `"c4"`, `"cs4"`, `"df3"` into its corresponding
/// numerical representation where C4 is traditionally 0 or MIDI 60
/// (depending on the base tuning system of the synthesizer).
///
/// # Parameters
/// - `token`: The lowercase note name to parse (e.g., `"c4"`).
///
/// # Returns
/// `Ok(Some(i32))` with the pitch integer if successful.
/// `Ok(None)` if the string is completely empty or obviously not a pitch.
///
/// # Errors
/// Returns a [`PitchLiteralError`] if the format is invalid (like `"c#4"` instead
/// of `"cs4"`) or if an uppercase spelling is attempted.
///
/// # Examples
///
/// ```
/// use orpheus_lang::parse_named_pitch_literal;
///
/// assert_eq!(parse_named_pitch_literal("c4").unwrap(), Some(60));
/// assert_eq!(parse_named_pitch_literal("cs4").unwrap(), Some(61));
/// assert_eq!(parse_named_pitch_literal("c5").unwrap(), Some(72));
/// ```
pub fn parse_named_pitch_literal(token: &str) -> Result<Option<i32>, PitchLiteralError> {
    let mut chars = token.chars();
    let Some(first) = chars.next() else {
        return Ok(None);
    };

    if is_uppercase_pitch_attempt(token, first) {
        return Err(PitchLiteralError::new(format!(
            "named pitch literal `{token}` must use lowercase ASCII spellings like `c4`, `fs4`, or `bf3`"
        )));
    }

    let Some(base_pitch_class) = note_pitch_class(first) else {
        return Ok(None);
    };

    let mut body = token[first.len_utf8()..].chars();
    let accidental = match body.next() {
        Some('s') => 1,
        Some('f') => -1,
        Some(_) | None => 0,
    };
    let octave_start = first.len_utf8() + usize::from(accidental != 0);
    let octave_suffix = &token[octave_start..];

    if octave_suffix.is_empty() {
        if accidental != 0 {
            return Err(PitchLiteralError::new(format!(
                "named pitch literal `{token}` is missing an octave suffix"
            )));
        }
        return Ok(None);
    }

    if !octave_suffix
        .chars()
        .all(|character| character.is_ascii_digit())
    {
        if accidental != 0
            || octave_suffix
                .chars()
                .next()
                .is_some_and(|character| character.is_ascii_digit())
        {
            return Err(PitchLiteralError::new(format!(
                "named pitch literal `{token}` uses unsupported syntax; use lowercase ASCII spellings like `c4`, `fs4`, or `bf3`"
            )));
        }
        return Ok(None);
    }

    let octave = octave_suffix.parse::<i32>().map_err(|_| {
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
        })?;

    Ok(Some(absolute))
}

fn is_uppercase_pitch_attempt(token: &str, first: char) -> bool {
    if !matches!(first, 'A'..='G') {
        return false;
    }

    matches!(
        token[first.len_utf8()..].chars().next(),
        Some('s' | 'f' | '0'..='9')
    )
}

const fn note_pitch_class(note: char) -> Option<i32> {
    match note {
        'c' => Some(0),
        'd' => Some(2),
        'e' => Some(4),
        'f' => Some(5),
        'g' => Some(7),
        'a' => Some(9),
        'b' => Some(11),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::parse_named_pitch_literal;

    #[test]
    fn pitch_literal_examples_match_midi_style_mapping() {
        assert_eq!(parse_named_pitch_literal("c4").unwrap(), Some(60));
        assert_eq!(parse_named_pitch_literal("a4").unwrap(), Some(69));
        assert_eq!(parse_named_pitch_literal("bf3").unwrap(), Some(58));
        assert_eq!(parse_named_pitch_literal("fs4").unwrap(), Some(66));
    }

    #[test]
    fn pitch_literal_missing_octave_is_rejected() {
        let error = parse_named_pitch_literal("cf").unwrap_err().to_string();

        assert!(error.contains("octave"));
    }

    #[test]
    fn non_pitch_identifiers_are_ignored() {
        assert_eq!(parse_named_pitch_literal("cutoff").unwrap(), None);
        assert_eq!(parse_named_pitch_literal("drums").unwrap(), None);
    }
}
