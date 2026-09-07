use std::time::Instant;

use harbor_domain::{WorkflowJob, WorkflowStatus, WorkflowStep};
use harbor_logs::{parse_workflow_log, parse_workflow_log_with_jobs};

fn main() {
    const LINE_COUNT: usize = 100_000;
    let mut input = String::with_capacity(LINE_COUNT * 48);
    for line in 0..LINE_COUNT {
        input.push_str("2026-07-12T12:00:00Z build output line ");
        input.push_str(&line.to_string());
        input.push('\n');
    }

    let started_at = Instant::now();
    let parsed = parse_workflow_log(42, &input);
    let elapsed = started_at.elapsed();
    assert_eq!(parsed.lines.len(), LINE_COUNT);

    println!(
        "parsed {LINE_COUNT} lines ({} bytes) in {elapsed:?}",
        input.len()
    );

    let start = chrono::DateTime::parse_from_rfc3339("2026-07-12T12:00:00Z")
        .unwrap()
        .with_timezone(&chrono::Utc);
    let jobs = [WorkflowJob {
        id: 1,
        name: "build".into(),
        status: WorkflowStatus::Completed,
        conclusion: None,
        steps: (0..20)
            .map(|index| WorkflowStep {
                number: index + 1,
                name: format!("Step {index}"),
                status: WorkflowStatus::Completed,
                conclusion: None,
                started_at: Some(start + chrono::Duration::seconds(i64::from(index))),
                completed_at: Some(start + chrono::Duration::seconds(i64::from(index + 1))),
            })
            .collect(),
    }];
    input.clear();
    for line in 0..LINE_COUNT {
        let second = line / (LINE_COUNT / 20);
        input.push_str(&format!(
            "build\tOutput\t2026-07-12T12:00:{second:02}.123Z \u{1b}[32mbuild output line {line}\u{1b}[0m\n"
        ));
    }
    let started_at = Instant::now();
    let parsed = parse_workflow_log_with_jobs(42, &input, &jobs);
    let elapsed = started_at.elapsed();
    assert_eq!(parsed.lines.len(), LINE_COUNT);
    assert!(
        parsed
            .lines
            .iter()
            .all(|line| line.step.as_deref().is_some_and(|step| step != "Output"))
    );
    println!("normalized and mapped {LINE_COUNT} colored lines across 20 steps in {elapsed:?}");
}
