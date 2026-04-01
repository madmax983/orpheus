use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct SampleManifest {
    pub tokens: BTreeMap<String, String>,
    pub aliases: BTreeMap<String, String>,
    pub regions: BTreeMap<String, SampleRegion>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SampleRegion {
    pub token: String,
    pub start: f64,
    pub end: f64,
    pub rate: f64,
}

#[derive(Debug)]
pub enum SampleManifestLoadError {
    Io { path: Box<str>, message: Box<str> },
    Parse { path: Box<str>, message: Box<str> },
}

impl std::fmt::Display for SampleManifestLoadError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io { path, message } => {
                write!(
                    formatter,
                    "failed to read sample manifest `{path}`: {message}"
                )
            }
            Self::Parse { path, message } => {
                write!(
                    formatter,
                    "failed to parse sample manifest `{path}`: {message}"
                )
            }
        }
    }
}

impl std::error::Error for SampleManifestLoadError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io { .. } | Self::Parse { .. } => None,
        }
    }
}

/// Loads a sample manifest file from the specified path, parsing its mappings
/// into a [`SampleManifest`] so custom external audio files can be dynamically
/// scheduled during pattern playback.
///
/// # Errors
///
/// Returns [`SampleManifestLoadError`] if the file cannot be read or if its
/// contents fail to parse correctly.
///
/// ## Examples
///
/// ```ignore
/// use std::io::Write;
/// use tempfile::NamedTempFile;
/// use orpheus_dsp::sample_manifest::load_sample_manifest;
///
/// let mut file = NamedTempFile::new().unwrap();
/// writeln!(file, "kick: /path/to/kick.wav").unwrap();
///
/// let manifest = load_sample_manifest(file.path()).unwrap();
/// assert_eq!(manifest.get("kick"), Some("/path/to/kick.wav"));
/// ```
pub fn load_sample_manifest(
    path: impl AsRef<Path>,
) -> Result<SampleManifest, SampleManifestLoadError> {
    let path = path.as_ref();
    let source = fs::read_to_string(path).map_err(|source| SampleManifestLoadError::Io {
        path: path.display().to_string().into_boxed_str(),
        message: match source.kind() {
            std::io::ErrorKind::NotFound => "file not found".into(),
            std::io::ErrorKind::PermissionDenied => "permission denied".into(),
            _ => source.to_string().into_boxed_str(),
        },
    })?;
    ManifestParser::new(&source)
        .parse_manifest()
        .map_err(|message| SampleManifestLoadError::Parse {
            path: path.display().to_string().into_boxed_str(),
            message: message.into_boxed_str(),
        })
}

struct ManifestParser<'a> {
    source: &'a str,
    offset: usize,
}

impl<'a> ManifestParser<'a> {
    const fn new(source: &'a str) -> Self {
        Self { source, offset: 0 }
    }

    fn parse_manifest(&mut self) -> Result<SampleManifest, String> {
        self.skip_ignored();
        self.expect_char('(')?;
        let mut manifest = SampleManifest::default();
        let mut seen_tokens = false;
        let mut seen_aliases = false;
        let mut seen_regions = false;

        loop {
            self.skip_ignored();
            if self.consume_char(')') {
                break;
            }

            let field = self.parse_identifier()?;
            self.skip_ignored();
            self.expect_char(':')?;
            self.skip_ignored();
            match field.as_str() {
                "tokens" => {
                    if seen_tokens {
                        return Err("duplicate `tokens` section".to_owned());
                    }
                    manifest.tokens = self.parse_string_map()?;
                    seen_tokens = true;
                }
                "aliases" => {
                    if seen_aliases {
                        return Err("duplicate `aliases` section".to_owned());
                    }
                    manifest.aliases = self.parse_string_map()?;
                    seen_aliases = true;
                }
                "regions" => {
                    if seen_regions {
                        return Err("duplicate `regions` section".to_owned());
                    }
                    manifest.regions = self.parse_region_map()?;
                    seen_regions = true;
                }
                other => {
                    return Err(format!(
                        "unknown manifest field `{other}`; expected `tokens`, `aliases`, or `regions`"
                    ));
                }
            }

            self.skip_ignored();
            if self.consume_char(',') {
                continue;
            }
            if self.peek_char() == Some(')') {
                continue;
            }
            return Err(self.expected_message("`,` or `)`"));
        }

        self.skip_ignored();
        if self.offset != self.source.len() {
            return Err(self.expected_message("end of manifest"));
        }

        Ok(manifest)
    }

    fn parse_string_map(&mut self) -> Result<BTreeMap<String, String>, String> {
        self.expect_char('{')?;
        let mut entries = BTreeMap::new();

        loop {
            self.skip_ignored();
            if self.consume_char('}') {
                break;
            }

            let key = self.parse_string()?;
            self.skip_ignored();
            self.expect_char(':')?;
            self.skip_ignored();
            let value = self.parse_string()?;
            if entries.insert(key.clone(), value).is_some() {
                return Err(format!("duplicate manifest key `{key}`"));
            }

            self.skip_ignored();
            if self.consume_char(',') {
                continue;
            }
            if self.peek_char() == Some('}') {
                continue;
            }
            return Err(self.expected_message("`,` or `}`"));
        }

        Ok(entries)
    }

    fn parse_region_map(&mut self) -> Result<BTreeMap<String, SampleRegion>, String> {
        self.expect_char('{')?;
        let mut entries = BTreeMap::new();

        loop {
            self.skip_ignored();
            if self.consume_char('}') {
                break;
            }

            let key = self.parse_string()?;
            self.skip_ignored();
            self.expect_char(':')?;
            self.skip_ignored();
            let value = self.parse_region()?;
            if entries.insert(key.clone(), value).is_some() {
                return Err(format!("duplicate manifest key `{key}`"));
            }

            self.skip_ignored();
            if self.consume_char(',') {
                continue;
            }
            if self.peek_char() == Some('}') {
                continue;
            }
            return Err(self.expected_message("`,` or `}`"));
        }

        Ok(entries)
    }

    fn parse_region(&mut self) -> Result<SampleRegion, String> {
        self.expect_char('(')?;
        let mut token = None;
        let mut start = None;
        let mut end = None;
        let mut rate = 1.0;
        let mut seen_rate = false;

        loop {
            self.skip_ignored();
            if self.consume_char(')') {
                break;
            }

            let field = self.parse_identifier()?;
            self.skip_ignored();
            self.expect_char(':')?;
            self.skip_ignored();

            match field.as_str() {
                "token" => {
                    if token.is_some() {
                        return Err("duplicate `token` field in region".to_owned());
                    }
                    token = Some(self.parse_string()?);
                }
                "start" => {
                    if start.is_some() {
                        return Err("duplicate `start` field in region".to_owned());
                    }
                    start = Some(self.parse_number()?);
                }
                "end" => {
                    if end.is_some() {
                        return Err("duplicate `end` field in region".to_owned());
                    }
                    end = Some(self.parse_number()?);
                }
                "rate" => {
                    if seen_rate {
                        return Err("duplicate `rate` field in region".to_owned());
                    }
                    rate = self.parse_number()?;
                    seen_rate = true;
                }
                other => {
                    return Err(format!(
                        "unknown region field `{other}`; expected `token`, `start`, `end`, or `rate`"
                    ));
                }
            }

            self.skip_ignored();
            if self.consume_char(',') {
                continue;
            }
            if self.peek_char() == Some(')') {
                continue;
            }
            return Err(self.expected_message("`,` or `)`"));
        }

        let token = token.ok_or_else(|| "region missing required `token` field".to_owned())?;
        let start = start.ok_or_else(|| "region missing required `start` field".to_owned())?;
        let end = end.ok_or_else(|| "region missing required `end` field".to_owned())?;
        if !start.is_finite()
            || !end.is_finite()
            || !(0.0..=1.0).contains(&start)
            || !(0.0..=1.0).contains(&end)
            || start >= end
        {
            return Err("region requires 0 <= start < end <= 1".to_owned());
        }
        if !rate.is_finite() || rate <= 0.0 {
            return Err("region `rate` must be a positive finite number".to_owned());
        }

        Ok(SampleRegion {
            token,
            start,
            end,
            rate,
        })
    }

    fn parse_identifier(&mut self) -> Result<String, String> {
        self.skip_ignored();
        let start = self.offset;
        let Some(first) = self.peek_char() else {
            return Err(self.expected_message("identifier"));
        };
        if !(first.is_ascii_alphabetic() || first == '_') {
            return Err(self.expected_message("identifier"));
        }
        self.offset += first.len_utf8();

        while let Some(character) = self.peek_char() {
            if !(character.is_ascii_alphanumeric() || character == '_') {
                break;
            }
            self.offset += character.len_utf8();
        }

        Ok(self.source[start..self.offset].to_owned())
    }

    fn parse_string(&mut self) -> Result<String, String> {
        self.skip_ignored();
        self.expect_char('"')?;
        let mut value = String::new();

        loop {
            let Some(character) = self.peek_char() else {
                return Err("unterminated string literal".to_owned());
            };
            self.offset += character.len_utf8();
            match character {
                '"' => break,
                '\\' => {
                    let Some(escaped) = self.peek_char() else {
                        return Err("unterminated string escape".to_owned());
                    };
                    self.offset += escaped.len_utf8();
                    match escaped {
                        '"' => value.push('"'),
                        '\\' => value.push('\\'),
                        'n' => value.push('\n'),
                        'r' => value.push('\r'),
                        't' => value.push('\t'),
                        other => {
                            return Err(format!(
                                "unsupported string escape `\\{other}` in sample manifest"
                            ));
                        }
                    }
                }
                other => value.push(other),
            }
        }

        Ok(value)
    }

    fn parse_number(&mut self) -> Result<f64, String> {
        self.skip_ignored();
        let start = self.offset;
        if matches!(self.peek_char(), Some('+' | '-')) {
            self.offset += 1;
        }

        let mut seen_digit = false;
        let mut seen_decimal = false;
        while let Some(character) = self.peek_char() {
            if character.is_ascii_digit() {
                seen_digit = true;
                self.offset += character.len_utf8();
                continue;
            }
            if character == '.' && !seen_decimal {
                seen_decimal = true;
                self.offset += character.len_utf8();
                continue;
            }
            break;
        }

        if !seen_digit {
            return Err(self.expected_message("number"));
        }

        self.source[start..self.offset]
            .parse::<f64>()
            .map_err(|_| self.expected_message("number"))
    }

    fn skip_ignored(&mut self) {
        loop {
            let remaining = &self.source[self.offset..];
            if remaining.starts_with("//") {
                while let Some(character) = self.peek_char() {
                    self.offset += character.len_utf8();
                    if character == '\n' {
                        break;
                    }
                }
                continue;
            }

            let Some(character) = self.peek_char() else {
                return;
            };
            if !character.is_whitespace() {
                return;
            }
            self.offset += character.len_utf8();
        }
    }

    fn expect_char(&mut self, expected: char) -> Result<(), String> {
        let Some(character) = self.peek_char() else {
            return Err(self.expected_message(&format!("`{expected}`")));
        };
        if character != expected {
            return Err(self.expected_message(&format!("`{expected}`")));
        }
        self.offset += character.len_utf8();
        Ok(())
    }

    fn consume_char(&mut self, expected: char) -> bool {
        if self.peek_char() == Some(expected) {
            self.offset += expected.len_utf8();
            true
        } else {
            false
        }
    }

    fn peek_char(&self) -> Option<char> {
        self.source[self.offset..].chars().next()
    }

    fn expected_message(&self, expected: &str) -> String {
        format!("expected {expected} near byte {}", self.offset)
    }
}
