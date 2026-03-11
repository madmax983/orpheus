use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct SampleManifest {
    pub tokens: BTreeMap<String, String>,
    pub aliases: BTreeMap<String, String>,
}

#[derive(Debug)]
pub enum SampleManifestLoadError {
    Io {
        path: Box<str>,
        source: std::io::Error,
    },
    Parse {
        path: Box<str>,
        message: Box<str>,
    },
}

impl std::fmt::Display for SampleManifestLoadError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io { path, source } => {
                write!(
                    formatter,
                    "failed to read sample manifest `{path}`: {source}"
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
            Self::Io { source, .. } => Some(source),
            Self::Parse { .. } => None,
        }
    }
}

pub fn load_sample_manifest(
    path: impl AsRef<Path>,
) -> Result<SampleManifest, SampleManifestLoadError> {
    let path = path.as_ref();
    let source = fs::read_to_string(path).map_err(|source| SampleManifestLoadError::Io {
        path: path.display().to_string().into_boxed_str(),
        source,
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

        loop {
            self.skip_ignored();
            if self.consume_char(')') {
                break;
            }

            let field = self.parse_identifier()?;
            self.skip_ignored();
            self.expect_char(':')?;
            self.skip_ignored();
            let entries = self.parse_string_map()?;

            match field.as_str() {
                "tokens" => {
                    if seen_tokens {
                        return Err("duplicate `tokens` section".to_owned());
                    }
                    manifest.tokens = entries;
                    seen_tokens = true;
                }
                "aliases" => {
                    if seen_aliases {
                        return Err("duplicate `aliases` section".to_owned());
                    }
                    manifest.aliases = entries;
                    seen_aliases = true;
                }
                other => {
                    return Err(format!(
                        "unknown manifest field `{other}`; expected `tokens` or `aliases`"
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
