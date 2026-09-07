use harbor_domain::WorkflowJob;
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
    #[serde(default)]
    format_version: u8,
    pub run_id: u64,
    pub job_id: Option<u64>,
    pub lines: Vec<LogLine>,
    pub warning_line_indices: Vec<usize>,
    pub error_line_indices: Vec<usize>,
}

impl LogChunk {
    /// Older cached chunks must be reloaded to apply log normalization and step mapping.
    pub fn is_current_format(&self) -> bool {
        self.format_version == 1
    }
}

pub fn parse_workflow_log(run_id: u64, text: &str) -> LogChunk {
    let mut parser = WorkflowLogParser::new(run_id);
    parser.push(text);
    parser.finish()
}

/// Recovers step context when an archive contains combined job output instead of step files.
pub fn parse_workflow_log_with_jobs(run_id: u64, text: &str, jobs: &[WorkflowJob]) -> LogChunk {
    let mut chunk = parse_workflow_log(run_id, text);
    let mut jobs_by_name = std::collections::HashMap::new();
    for job in jobs {
        jobs_by_name.insert(job.name.clone(), job);
        jobs_by_name.insert(job.name.replace('/', "_"), job);
    }
    for line in &mut chunk.lines {
        let Some(job) = line.job.as_ref().and_then(|name| jobs_by_name.get(name)) else {
            continue;
        };
        if let Some(step) = job.steps.iter().find(|step| {
            line.step
                .as_ref()
                .is_some_and(|name| *name == step.name || *name == step.name.replace('/', "_"))
        }) {
            line.step = Some(step.name.clone());
        } else if matches!(line.step.as_deref(), Some("Output" | "UNKNOWN STEP") | None) {
            let timestamp = line
                .text
                .split_once(' ')
                .and_then(|(timestamp, _)| chrono::DateTime::parse_from_rfc3339(timestamp).ok());
            if let Some(timestamp) = timestamp {
                // The jobs API reports whole seconds; a step's final second can contain output.
                if let Some(step) = job
                    .steps
                    .iter()
                    .filter(|step| {
                        step.started_at.is_some_and(|start| start <= timestamp)
                            && step
                                .completed_at
                                .is_none_or(|end| timestamp.timestamp() <= end.timestamp())
                    })
                    .max_by_key(|step| (step.started_at, step.number))
                {
                    line.step = Some(step.name.clone());
                }
            }
        }
        line.job = Some(job.name.clone());
    }
    chunk
}

pub struct WorkflowLogParser {
    chunk: LogChunk,
    pending: String,
}

impl WorkflowLogParser {
    pub fn new(run_id: u64) -> Self {
        Self {
            chunk: LogChunk {
                format_version: 1,
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
        let (job, step, mut text) = match (first, second, third) {
            (Some(job), Some(step), Some(output)) => (
                Some(job.to_string()),
                Some(step.to_string()),
                output.to_string(),
            ),
            _ => (None, None, text),
        };
        strip_terminal_sequences(&mut text);
        let severity = infer_severity(&text);
        let message_start = text
            .split_once(' ')
            .filter(|(prefix, _)| prefix.contains('T') && prefix.ends_with('Z'))
            .map_or(0, |(prefix, _)| prefix.len() + 1);
        let message = &text[message_start..];
        if message == "##[endgroup]" {
            return;
        }
        if let Some(marker) = ["##[group]", "##[command]"]
            .into_iter()
            .find(|marker| message.starts_with(marker))
        {
            text.drain(message_start..message_start + marker.len());
        }
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

fn strip_terminal_sequences(text: &mut String) {
    if !text.contains('\u{1b}') {
        return;
    }
    enum State {
        Text,
        Escape,
        ControlSequence,
        String,
        StringEscape,
    }
    let mut state = State::Text;
    text.retain(|character| {
        state = match state {
            State::Text if character != '\u{1b}' => return true,
            State::Text => State::Escape,
            State::Escape if character == '[' => State::ControlSequence,
            State::Escape if matches!(character, ']' | 'P' | '^' | '_') => State::String,
            State::Escape => State::Text,
            State::ControlSequence if ('@'..='~').contains(&character) => State::Text,
            State::ControlSequence => State::ControlSequence,
            State::String if character == '\u{7}' => State::Text,
            State::String if character == '\u{1b}' => State::StringEscape,
            State::String => State::String,
            State::StringEscape if character == '\\' => State::Text,
            State::StringEscape => State::String,
        };
        false
    });
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
    fn maps_combined_job_output_to_steps_using_timestamps() {
        use chrono::{TimeZone, Utc};
        use harbor_domain::{WorkflowStatus, WorkflowStep};

        let start = Utc.with_ymd_and_hms(2026, 9, 7, 12, 0, 0).unwrap();
        let jobs = [WorkflowJob {
            id: 1,
            name: "build / test".to_string(),
            status: WorkflowStatus::Completed,
            conclusion: None,
            steps: ["Set up job", "Run / tests"]
                .into_iter()
                .enumerate()
                .map(|(index, name)| WorkflowStep {
                    name: name.to_string(),
                    number: index as u32 + 1,
                    status: WorkflowStatus::Completed,
                    conclusion: None,
                    started_at: Some(start + chrono::Duration::seconds(index as i64 * 10)),
                    completed_at: Some(start + chrono::Duration::seconds((index as i64 + 1) * 10)),
                })
                .collect(),
        }];
        let chunk = parse_workflow_log_with_jobs(
            1,
            concat!(
                "build _ test\tOutput\t2026-09-07T12:00:01Z setup\n",
                "build _ test\tUNKNOWN STEP\t2026-09-07T12:00:10.100Z test output\n",
                "build _ test\tRun _ tests\toutput without timestamp\n",
                "build _ test\tOutput\t2026-09-07T12:00:20.500Z final output\n",
                "build _ test\tOutput\t2026-09-07T12:00:30Z outside any step\n",
            ),
            &jobs,
        );

        assert_eq!(chunk.lines[0].step.as_deref(), Some("Set up job"));
        assert_eq!(chunk.lines[4].step.as_deref(), Some("Output"));
        assert_eq!(
            filter_log_lines(
                &chunk,
                Some(&("build / test".into(), Some("Run / tests".into()))),
                ""
            ),
            [1, 2, 3]
        );
    }

    #[test]
    fn displays_terminal_output_without_escape_sequences_or_runner_markers() {
        let chunk = parse_workflow_log(
            1,
            concat!(
                "job\tTest\t2026-09-07T12:00:00Z ##[group]Run tests\n",
                "job\tTest\t2026-09-07T12:00:00Z ##[command]cargo test\n",
                "job\tTest\t\u{1b}[31merror: failed café\u{1b}[0m\n",
                "job\tTest\t\u{1b}]8;;https://example.com\u{1b}\\details\u{1b}]8;;\u{1b}\\\n",
                "job\tTest\t2026-09-07T12:00:01Z ##[endgroup]\n",
            ),
        );
        assert_eq!(chunk.lines.len(), 4);
        assert_eq!(chunk.lines[0].text, "2026-09-07T12:00:00Z Run tests");
        assert_eq!(chunk.lines[1].text, "2026-09-07T12:00:00Z cargo test");
        assert_eq!(chunk.lines[2].text, "error: failed café");
        assert_eq!(chunk.lines[3].text, "details");
        assert_eq!(chunk.error_line_indices, [2]);
    }

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
