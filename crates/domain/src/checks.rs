use crate::{CheckConclusion, CheckRun, CheckStatus, ChecksSummary};

pub fn checks_summary_from_runs(check_runs: &[CheckRun]) -> ChecksSummary {
    let mut summary = ChecksSummary {
        total: check_runs.len(),
        ..ChecksSummary::default()
    };

    for check_run in check_runs {
        match (check_run.status, check_run.conclusion) {
            (CheckStatus::Completed, Some(CheckConclusion::Success)) => summary.passed += 1,
            (CheckStatus::Completed, Some(CheckConclusion::Skipped)) => summary.skipped += 1,
            (CheckStatus::Completed, Some(CheckConclusion::Neutral)) => summary.skipped += 1,
            (CheckStatus::Completed, Some(CheckConclusion::Cancelled)) => summary.failed += 1,
            (CheckStatus::Completed, Some(CheckConclusion::Failure)) => summary.failed += 1,
            (CheckStatus::Completed, Some(CheckConclusion::TimedOut)) => summary.failed += 1,
            (CheckStatus::Completed, Some(CheckConclusion::ActionRequired)) => summary.failed += 1,
            (CheckStatus::Completed, None) => summary.failed += 1,
            (CheckStatus::InProgress | CheckStatus::Queued, _) => summary.pending += 1,
        }
    }

    summary
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn summarizes_check_runs() {
        let check_runs = vec![
            check_run(CheckStatus::Completed, Some(CheckConclusion::Success)),
            check_run(CheckStatus::Completed, Some(CheckConclusion::Failure)),
            check_run(CheckStatus::Completed, Some(CheckConclusion::Skipped)),
            check_run(CheckStatus::InProgress, None),
        ];

        let summary = checks_summary_from_runs(&check_runs);

        assert_eq!(summary.total, 4);
        assert_eq!(summary.passed, 1);
        assert_eq!(summary.failed, 1);
        assert_eq!(summary.skipped, 1);
        assert_eq!(summary.pending, 1);
    }

    fn check_run(status: CheckStatus, conclusion: Option<CheckConclusion>) -> CheckRun {
        CheckRun {
            id: None,
            name: "check".to_string(),
            status,
            conclusion,
            details_url: None,
            html_url: None,
            started_at: None,
            completed_at: None,
        }
    }
}
