use harbor_domain::{MergeQueueState, MergeState, PullRequest, PullRequestState, ReviewDecision};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PullRequestRowRailTone {
    Neutral,
    Danger,
    Warning,
    Success,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PullRequestRowSignal {
    MergeConflict,
    ChangesRequested,
    ReviewNeeded,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PullRequestMergeQueueStatus {
    Waiting,
    Queued,
}

impl PullRequestMergeQueueStatus {
    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Waiting => "Waiting to queue",
            Self::Queued => "Queued to merge",
        }
    }

    pub(crate) fn description(self) -> &'static str {
        match self {
            Self::Waiting => "Requirements must pass before this pull request enters the queue",
            Self::Queued => "GitHub will merge this pull request through the merge queue",
        }
    }
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

pub(crate) fn merge_when_ready_blocker(pr: &PullRequest) -> Option<String> {
    if pr.state != PullRequestState::Open {
        return Some(format!("PR #{} is not open", pr.number));
    }
    if pr.is_draft {
        return Some(format!("PR #{} is still a draft", pr.number));
    }
    if pr.node_id.is_empty() || pr.head_sha.is_empty() {
        return Some(format!(
            "PR #{} is missing GitHub merge metadata",
            pr.number
        ));
    }

    match pr.merge_capabilities.queue_state {
        MergeQueueState::Unknown => {
            return Some("Loading merge queue settings from GitHub".to_string());
        }
        MergeQueueState::Disabled => {
            return Some(format!("PR #{} does not target a merge queue", pr.number));
        }
        MergeQueueState::Queued => {
            return Some(format!("PR #{} is already queued to merge", pr.number));
        }
        MergeQueueState::Enabled => {}
    }

    if pr.merge_capabilities.auto_merge_enabled {
        return Some(format!(
            "PR #{} is already set to merge when ready",
            pr.number
        ));
    }
    if !pr.merge_capabilities.viewer_can_queue {
        return Some(format!(
            "You do not have permission to queue PR #{}",
            pr.number
        ));
    }

    None
}

pub(crate) fn merge_without_requirements_blocker(pr: &PullRequest) -> Option<String> {
    if !pr.merge_capabilities.viewer_can_merge_as_admin {
        return Some(format!(
            "You do not have permission to bypass requirements for PR #{}",
            pr.number
        ));
    }
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
        Some(MergeState::Dirty) => Some(format!("PR #{} has merge conflicts", pr.number)),
        Some(MergeState::Unknown) | None => Some(format!(
            "PR #{} is not confirmed mergeable by GitHub",
            pr.number
        )),
        Some(MergeState::Clean | MergeState::Blocked | MergeState::Behind) => None,
    }
}

pub(crate) fn pull_request_row_signal(pr: &PullRequest) -> Option<PullRequestRowSignal> {
    if pr.state != PullRequestState::Open || pr.is_draft {
        return None;
    }

    if pr.merge_state == Some(MergeState::Dirty) {
        Some(PullRequestRowSignal::MergeConflict)
    } else {
        match pr.review_decision {
            Some(ReviewDecision::ChangesRequested) => Some(PullRequestRowSignal::ChangesRequested),
            Some(ReviewDecision::ReviewRequired) => Some(PullRequestRowSignal::ReviewNeeded),
            Some(ReviewDecision::Approved) | None => None,
        }
    }
}

pub(crate) fn pull_request_merge_queue_status(
    pr: &PullRequest,
) -> Option<PullRequestMergeQueueStatus> {
    if pr.state != PullRequestState::Open {
        return None;
    }

    match pr.merge_capabilities.queue_state {
        MergeQueueState::Queued => Some(PullRequestMergeQueueStatus::Queued),
        MergeQueueState::Enabled if pr.merge_capabilities.auto_merge_enabled => {
            Some(PullRequestMergeQueueStatus::Waiting)
        }
        MergeQueueState::Unknown | MergeQueueState::Disabled | MergeQueueState::Enabled => None,
    }
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
    if pr.state != PullRequestState::Open || pr.is_draft {
        return PullRequestRowRailTone::Neutral;
    }

    if pr.merge_state == Some(MergeState::Dirty)
        || pr.checks_summary.failed > 0
        || pr.review_decision == Some(ReviewDecision::ChangesRequested)
    {
        PullRequestRowRailTone::Danger
    } else if pr.review_decision == Some(ReviewDecision::ReviewRequired) {
        PullRequestRowRailTone::Warning
    } else if is_ready_to_merge(pr) {
        PullRequestRowRailTone::Success
    } else {
        PullRequestRowRailTone::Neutral
    }
}

#[cfg(test)]
#[path = "pull_request/tests.rs"]
mod tests;
