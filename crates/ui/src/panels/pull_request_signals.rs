use harbor_domain::{
    CheckConclusion, CheckRun, CheckStatus, ChecksSummary, MergeState, PullRequest,
    PullRequestState, ReviewDecision,
};

const MAX_PULL_REQUEST_ROW_SIGNALS: usize = 3;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PullRequestRowSignalTone {
    Danger,
    Warning,
    Success,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PullRequestRowRailTone {
    Neutral,
    Danger,
    Warning,
    Success,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PullRequestRowSignalKind {
    MergeConflict,
    ReviewApproved,
    ReviewChangesRequestedThreads,
    ReviewNeeded,
    UnresolvedThreads,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PullRequestRowSignal {
    pub(crate) kind: PullRequestRowSignalKind,
    pub(crate) label: Option<String>,
}

impl PullRequestRowSignal {
    fn with_label(kind: PullRequestRowSignalKind, label: impl Into<String>) -> Self {
        Self {
            kind,
            label: Some(label.into()),
        }
    }
}

pub(crate) fn checks_summary_from_runs(check_runs: &[CheckRun]) -> ChecksSummary {
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

pub(crate) fn review_action_blocker(pr: &PullRequest) -> Option<String> {
    if pr.state != PullRequestState::Open {
        Some(format!("PR #{} is not open", pr.number))
    } else {
        None
    }
}

pub(crate) fn merge_blocker(pr: &PullRequest) -> Option<String> {
    if pr.state != PullRequestState::Open {
        return Some(format!("PR #{} is not open", pr.number));
    }

    if pr.is_draft {
        return Some(format!("PR #{} is still a draft", pr.number));
    }

    if pr.head_sha.is_empty() {
        return Some(format!("PR #{} is missing a head SHA", pr.number));
    }

    match pr.merge_state {
        Some(MergeState::Clean) => {}
        Some(MergeState::Dirty) => {
            return Some(format!("PR #{} has merge conflicts", pr.number));
        }
        Some(MergeState::Blocked) => {
            return Some(format!("PR #{} is blocked by repository rules", pr.number));
        }
        Some(MergeState::Behind) => {
            return Some(format!("PR #{} is behind the base branch", pr.number));
        }
        Some(MergeState::Unknown) | None => {
            return Some(format!(
                "PR #{} is not confirmed mergeable by GitHub",
                pr.number
            ));
        }
    }

    if pr.checks_summary.failed > 0 {
        return Some(format!("PR #{} still has failing checks", pr.number));
    }

    if pr.checks_summary.pending > 0 {
        return Some(format!("PR #{} still has pending checks", pr.number));
    }

    if pr.unresolved_threads > 0 {
        return Some(format!(
            "PR #{} still has {} unresolved review threads",
            pr.number, pr.unresolved_threads
        ));
    }

    None
}

pub(crate) fn visible_pull_request_row_signals(pr: &PullRequest) -> Vec<PullRequestRowSignal> {
    pull_request_row_signals(pr)
        .into_iter()
        .take(MAX_PULL_REQUEST_ROW_SIGNALS)
        .collect()
}

fn pull_request_row_signals(pr: &PullRequest) -> Vec<PullRequestRowSignal> {
    let mut signals = Vec::new();

    if pr.merge_state == Some(MergeState::Dirty) {
        signals.push(PullRequestRowSignal::with_label(
            PullRequestRowSignalKind::MergeConflict,
            "conflict",
        ));
    }

    match pr.review_decision {
        Some(ReviewDecision::ChangesRequested) => signals.push(PullRequestRowSignal::with_label(
            PullRequestRowSignalKind::ReviewChangesRequestedThreads,
            if pr.unresolved_threads > 0 {
                pr.unresolved_threads.to_string()
            } else {
                "changes".to_string()
            },
        )),
        Some(ReviewDecision::ReviewRequired) => {
            signals.push(PullRequestRowSignal::with_label(
                PullRequestRowSignalKind::ReviewNeeded,
                "review",
            ));
        }
        Some(ReviewDecision::Approved) => {
            signals.push(PullRequestRowSignal::with_label(
                PullRequestRowSignalKind::ReviewApproved,
                "approved",
            ));
        }
        None => {}
    }

    if pr.review_decision != Some(ReviewDecision::ChangesRequested) && pr.unresolved_threads > 0 {
        signals.push(PullRequestRowSignal::with_label(
            PullRequestRowSignalKind::UnresolvedThreads,
            pr.unresolved_threads.to_string(),
        ));
    }

    signals
}

pub(crate) fn is_ready_to_merge(pr: &PullRequest) -> bool {
    pr.state == PullRequestState::Open
        && !pr.is_draft
        && pr.merge_state == Some(MergeState::Clean)
        && pr.checks_summary.total > 0
        && pr.checks_summary.failed == 0
        && pr.checks_summary.pending == 0
        && pr.unresolved_threads == 0
        && !matches!(
            pr.review_decision,
            Some(ReviewDecision::ChangesRequested | ReviewDecision::ReviewRequired)
        )
}

pub(crate) fn pull_request_row_rail_tone(pr: &PullRequest) -> PullRequestRowRailTone {
    if pr.state != PullRequestState::Open {
        return PullRequestRowRailTone::Neutral;
    }

    if pr.merge_state == Some(MergeState::Dirty)
        || pr.checks_summary.failed > 0
        || pr.review_decision == Some(ReviewDecision::ChangesRequested)
    {
        PullRequestRowRailTone::Danger
    } else if pr.checks_summary.pending > 0
        || pr.review_decision == Some(ReviewDecision::ReviewRequired)
        || pr.unresolved_threads > 0
    {
        PullRequestRowRailTone::Warning
    } else if is_ready_to_merge(pr) || pr.review_decision == Some(ReviewDecision::Approved) {
        PullRequestRowRailTone::Success
    } else {
        PullRequestRowRailTone::Neutral
    }
}

#[cfg(test)]
#[path = "pull_request/tests.rs"]
mod tests;
