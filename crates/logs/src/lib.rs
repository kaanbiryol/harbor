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

pub fn parse_workflow_log(run_id: u64, text: &str) -> LogChunk {
    LogChunk {
        run_id,
        job_id: None,
        lines: text
            .lines()
            .enumerate()
            .map(|(index, line)| LogLine {
                number: index + 1,
                severity: infer_severity(line),
                text: line.to_string(),
            })
            .collect(),
    }
}

fn infer_severity(line: &str) -> LogSeverity {
    let lower = line.to_lowercase();

    if lower.contains("::error") || lower.contains("[error]") || lower.contains("error:") {
        LogSeverity::Error
    } else if lower.contains("::warning")
        || lower.contains("[warning]")
        || lower.contains("warning:")
    {
        LogSeverity::Warning
    } else if lower.contains("::debug") || lower.contains("[debug]") || lower.contains("trace:") {
        LogSeverity::Trace
    } else {
        LogSeverity::Info
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_log_lines_with_severity() {
        let chunk = parse_workflow_log(42, "build\nwarning: slow\n::error::failed\n");

        assert_eq!(chunk.run_id, 42);
        assert_eq!(chunk.lines.len(), 3);
        assert_eq!(chunk.lines[0].number, 1);
        assert_eq!(chunk.lines[0].severity, LogSeverity::Info);
        assert_eq!(chunk.lines[1].severity, LogSeverity::Warning);
        assert_eq!(chunk.lines[2].severity, LogSeverity::Error);
    }
}
