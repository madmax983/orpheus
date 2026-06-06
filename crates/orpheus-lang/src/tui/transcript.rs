#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TranscriptEntry {
    Command(String),
    Success(String),
    Error(String),
    Warning(String),
    Info(String),
}
