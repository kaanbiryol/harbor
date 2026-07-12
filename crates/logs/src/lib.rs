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
    pub warning_line_indices: Vec<usize>,
    pub error_line_indices: Vec<usize>,
}

pub fn parse_workflow_log(run_id: u64, text: &str) -> LogChunk {
    let mut parser = WorkflowLogParser::new(run_id);
    parser.push(text);
    parser.finish()
}

pub struct WorkflowLogParser {
    chunk: LogChunk,
    pending: String,
}

impl WorkflowLogParser {
    pub fn new(run_id: u64) -> Self {
        Self {
            chunk: LogChunk {
                run_id,
                job_id: None,
                lines: Vec::new(),
                warning_line_indices: Vec::new(),
                error_line_indices: Vec::new(),
            },
            pending: String::new(),
        }
    }

    pub fn push(&mut self, text: &str) {
        self.pending.push_str(text);
        let Some(complete_len) = self.pending.rfind('\n').map(|index| index + 1) else {
            return;
        };

        let mut complete = std::mem::take(&mut self.pending);
        self.pending = complete.split_off(complete_len);
        for line in complete.split_terminator('\n') {
            self.push_line(line.trim_end_matches('\r').to_string());
        }
    }

    pub fn finish(mut self) -> LogChunk {
        if !self.pending.is_empty() {
            let line = std::mem::take(&mut self.pending);
            self.push_line(line);
        }
        self.chunk
    }

    fn push_line(&mut self, text: String) {
        let index = self.chunk.lines.len();
        let severity = infer_severity(&text);
        match severity {
            LogSeverity::Warning => self.chunk.warning_line_indices.push(index),
            LogSeverity::Error => self.chunk.error_line_indices.push(index),
            LogSeverity::Trace | LogSeverity::Info => {}
        }
        self.chunk.lines.push(LogLine {
            number: index + 1,
            severity,
            text,
        });
    }
}

fn infer_severity(line: &str) -> LogSeverity {
    if contains_ignore_ascii_case(line, "::error")
        || contains_ignore_ascii_case(line, "[error]")
        || contains_ignore_ascii_case(line, "error:")
    {
        LogSeverity::Error
    } else if contains_ignore_ascii_case(line, "::warning")
        || contains_ignore_ascii_case(line, "[warning]")
        || contains_ignore_ascii_case(line, "warning:")
    {
        LogSeverity::Warning
    } else if contains_ignore_ascii_case(line, "::debug")
        || contains_ignore_ascii_case(line, "[debug]")
        || contains_ignore_ascii_case(line, "trace:")
    {
        LogSeverity::Trace
    } else {
        LogSeverity::Info
    }
}

fn contains_ignore_ascii_case(value: &str, pattern: &str) -> bool {
    value
        .as_bytes()
        .windows(pattern.len())
        .any(|window| window.eq_ignore_ascii_case(pattern.as_bytes()))
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
        assert_eq!(chunk.warning_line_indices, [1]);
        assert_eq!(chunk.error_line_indices, [2]);
    }

    #[test]
    fn incrementally_parses_lines_split_across_chunks() {
        let mut parser = WorkflowLogParser::new(42);
        parser.push("build\nwarn");
        parser.push("ing: slow\r\n::error::failed");
        let chunk = parser.finish();

        assert_eq!(
            chunk
                .lines
                .iter()
                .map(|line| line.text.as_str())
                .collect::<Vec<_>>(),
            ["build", "warning: slow", "::error::failed"]
        );
        assert_eq!(chunk.warning_line_indices, [1]);
        assert_eq!(chunk.error_line_indices, [2]);
    }

    #[test]
    fn classifies_mixed_case_severity_without_normalizing_line_text() {
        let chunk = parse_workflow_log(42, "ERROR: failed\n[Warning] slow\n::Debug::details\n");

        assert_eq!(chunk.lines[0].severity, LogSeverity::Error);
        assert_eq!(chunk.lines[1].severity, LogSeverity::Warning);
        assert_eq!(chunk.lines[2].severity, LogSeverity::Trace);
        assert_eq!(chunk.lines[0].text, "ERROR: failed");
    }
}
