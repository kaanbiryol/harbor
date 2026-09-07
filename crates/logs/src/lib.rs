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
    #[serde(default)]
    pub job: Option<String>,
    #[serde(default)]
    pub step: Option<String>,
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
        let mut fields = text.splitn(3, '\t');
        let first = fields.next();
        let second = fields.next();
        let third = fields.next();
        let (job, step, text) = match (first, second, third) {
            (Some(job), Some(step), Some(output)) => (
                Some(job.to_string()),
                Some(step.to_string()),
                output.to_string(),
            ),
            _ => (None, None, text),
        };
        let severity = infer_severity(&text);
        match severity {
            LogSeverity::Warning => self.chunk.warning_line_indices.push(index),
            LogSeverity::Error => self.chunk.error_line_indices.push(index),
            LogSeverity::Trace | LogSeverity::Info => {}
        }
        self.chunk.lines.push(LogLine {
            job,
            step,
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

#[cfg(test)]
mod context_tests {
    use super::*;
    #[test]
    fn preserves_job_step_and_output_tabs() {
        let chunk = parse_workflow_log(
            1,
            "test (2)\tRun tests\t2026-01-01T00:00:00Z error: a\tb\nplain",
        );
        assert_eq!(chunk.lines[0].job.as_deref(), Some("test (2)"));
        assert_eq!(chunk.lines[0].step.as_deref(), Some("Run tests"));
        assert!(chunk.lines[0].text.ends_with("a\tb"));
        assert_eq!(chunk.lines[1].job, None);
    }
}

/// Returns source line indices without copying log output.
pub fn filter_log_lines(
    chunk: &LogChunk,
    section: Option<&(String, Option<String>)>,
    query: &str,
) -> Vec<usize> {
    let query = query.to_lowercase();
    let archive_job = section.map(|(job, _)| job.replace('/', "_"));
    chunk
        .lines
        .iter()
        .enumerate()
        .filter(|(_, line)| {
            section.is_none_or(|(job, step)| {
                (line.job.as_ref() == Some(job) || line.job.as_ref() == archive_job.as_ref())
                    && step
                        .as_ref()
                        .is_none_or(|step| line.step.as_ref() == Some(step))
            }) && (query.is_empty() || line.text.to_lowercase().contains(&query))
        })
        .map(|(index, _)| index)
        .collect()
}

#[cfg(test)]
mod search_tests {
    use super::*;
    #[test]
    fn filters_by_job_step_and_case_insensitive_query() {
        let chunk = parse_workflow_log(
            1,
            "test (1)\tBuild\tok\ntest (2)\tBuild\tERROR: failed\ntest (2)\tTest\tok",
        );
        assert_eq!(
            filter_log_lines(&chunk, Some(&("test (2)".into(), None)), ""),
            vec![1, 2]
        );
        assert_eq!(
            filter_log_lines(
                &chunk,
                Some(&("test (2)".into(), Some("Build".into()))),
                "error"
            ),
            vec![1]
        );
        assert!(filter_log_lines(&chunk, None, "missing").is_empty());
    }
    #[test]
    fn indexes_large_logs() {
        let text = "job\tstep\t2026-01-01T00:00:00Z output\n".repeat(100_000);
        let start = std::time::Instant::now();
        let chunk = parse_workflow_log(1, &text);
        let indices = filter_log_lines(&chunk, None, "output");
        assert_eq!(indices.len(), 100_000);
        eprintln!(
            "100k log lines parsed and searched in {:?}",
            start.elapsed()
        );
    }
}

#[cfg(test)]
mod archive_job_tests {
    use super::*;
    #[test]
    fn matches_sanitized_archive_job_names() {
        let chunk = parse_workflow_log(
            1,
            "profile _ branch\tOutput\tbuild output\nother\tOutput\tother output",
        );
        assert_eq!(
            filter_log_lines(&chunk, Some(&("profile / branch".into(), None)), ""),
            vec![0]
        );
    }
}
