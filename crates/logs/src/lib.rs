use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum LogSeverity {
    Trace,
    Info,
    Warning,
    Error,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct LogLine {
    pub number: usize,
    pub severity: LogSeverity,
    pub text: String,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct LogChunk {
    pub run_id: u64,
    pub job_id: Option<u64>,
    pub lines: Vec<LogLine>,
}
