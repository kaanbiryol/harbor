use harbor_domain::{MergeState, PullRequest, PullRequestState, ReviewDecision};

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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PullRequestReadiness {
    Conflicts,
    ChecksFailed,
    ChecksPending,
    Draft,
    ChangesRequested,
    ReviewRequired,
    ConversationsOpen,
    Ready,
}

impl PullRequestReadiness {
    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Conflicts => "Conflicts",
            Self::ChecksFailed => "Checks failed",
            Self::ChecksPending => "Checks pending",
            Self::Draft => "Draft",
            Self::ChangesRequested => "Changes requested",
            Self::ReviewRequired => "Review required",
            Self::ConversationsOpen => "Conversations open",
            Self::Ready => "Ready",
        }
    }

    pub(crate) fn description(self) -> &'static str {
        match self {
            Self::Conflicts => "Resolve conflicts to merge.",
            Self::ChecksFailed => "Fix failing checks.",
            Self::ChecksPending => "Waiting for checks.",
            Self::Draft => "Not ready for review.",
            Self::ChangesRequested => "Address review feedback.",
            Self::ReviewRequired => "Approval needed to merge.",
            Self::ConversationsOpen => "Resolve threads to merge.",
            Self::Ready => "Ready to merge.",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum MergeReadiness {
    Conflicts,
    Blocked,
    Behind,
    Unknown,
    WaitingForApproval,
    ConversationsOpen,
    ChecksPending,
    Ready,
}

impl MergeReadiness {
    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Conflicts => "Conflicts",
            Self::Blocked
            | Self::WaitingForApproval
            | Self::ConversationsOpen
            | Self::ChecksPending => "Blocked",
            Self::Behind => "Behind",
            Self::Unknown => "Unknown",
            Self::Ready => "Ready",
        }
    }

    pub(crate) fn description(self) -> &'static str {
        match self {
            Self::Conflicts => "Resolve merge conflicts",
            Self::Blocked => "Requirements not met",
            Self::Behind => "Update branch",
            Self::Unknown => "Status unavailable",
            Self::WaitingForApproval => "Waiting for approval",
            Self::ConversationsOpen => "Resolve open threads",
            Self::ChecksPending => "Waiting for checks",
            Self::Ready => "Requirements met",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ReviewReadiness {
    Approved,
    ChangesRequested,
    Pending,
}

impl ReviewReadiness {
    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Approved => "Approved",
            Self::ChangesRequested => "Changes requested",
            Self::Pending => "Pending",
        }
    }

    pub(crate) fn description(self) -> &'static str {
        match self {
            Self::Approved => "Approvals received",
            Self::ChangesRequested => "Changes were requested",
            Self::Pending => "1 approval required",
        }
    }
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

pub(crate) fn review_action_blocker(pr: &PullRequest) -> Option<String> {
    if pr.state != PullRequestState::Open {
        Some(format!("PR #{} is not open", pr.number))
    } else {
        None
    }
}

pub(crate) fn pull_request_readiness(pr: &PullRequest) -> PullRequestReadiness {
    if pr.merge_state == Some(MergeState::Dirty) {
        PullRequestReadiness::Conflicts
    } else if pr.checks_summary.failed > 0 {
        PullRequestReadiness::ChecksFailed
    } else if pr.checks_summary.pending > 0 {
        PullRequestReadiness::ChecksPending
    } else if pr.is_draft {
        PullRequestReadiness::Draft
    } else if pr.review_decision == Some(ReviewDecision::ChangesRequested) {
        PullRequestReadiness::ChangesRequested
    } else if pr.review_decision != Some(ReviewDecision::Approved) {
        PullRequestReadiness::ReviewRequired
    } else if pr.unresolved_threads > 0 {
        PullRequestReadiness::ConversationsOpen
    } else {
        PullRequestReadiness::Ready
    }
}

pub(crate) fn review_readiness(decision: Option<ReviewDecision>) -> ReviewReadiness {
    match decision {
        Some(ReviewDecision::Approved) => ReviewReadiness::Approved,
        Some(ReviewDecision::ChangesRequested) => ReviewReadiness::ChangesRequested,
        Some(ReviewDecision::ReviewRequired) | None => ReviewReadiness::Pending,
    }
}

pub(crate) fn merge_readiness(pr: &PullRequest) -> MergeReadiness {
    match pr.merge_state {
        Some(MergeState::Dirty) => MergeReadiness::Conflicts,
        Some(MergeState::Blocked) => MergeReadiness::Blocked,
        Some(MergeState::Behind) => MergeReadiness::Behind,
        Some(MergeState::Unknown) | None => MergeReadiness::Unknown,
        Some(MergeState::Clean) if pr.review_decision != Some(ReviewDecision::Approved) => {
            MergeReadiness::WaitingForApproval
        }
        Some(MergeState::Clean) if pr.unresolved_threads > 0 => MergeReadiness::ConversationsOpen,
        Some(MergeState::Clean)
            if pr.checks_summary.failed > 0 || pr.checks_summary.pending > 0 =>
        {
            MergeReadiness::ChecksPending
        }
        Some(MergeState::Clean) => MergeReadiness::Ready,
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
