use std::time::Instant;

use harbor_logs::parse_workflow_log;

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
}
