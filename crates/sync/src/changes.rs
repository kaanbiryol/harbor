use std::collections::HashMap;

use harbor_domain::{MergeState, PullRequest, PullRequestState, RepoId, ReviewDecision};

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum PullRequestChangeKind {
    NewPullRequest,
    Closed,
    Merged,
    ReviewDecisionChanged,
    Approved,
    ChangesRequested,
    ReviewActivity,
    CheckFailed,
    CheckPassed,
    HeadChanged,
    MergeStateChanged,
}

impl PullRequestChangeKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::NewPullRequest => "New pull request",
            Self::Closed => "Pull request closed",
            Self::Merged => "Pull request merged",
            Self::ReviewDecisionChanged => "Review decision changed",
            Self::Approved => "Pull request approved",
            Self::ChangesRequested => "Changes requested",
            Self::ReviewActivity => "Review activity",
            Self::CheckFailed => "Checks failed",
            Self::CheckPassed => "Checks passed",
            Self::HeadChanged => "Pull request updated",
            Self::MergeStateChanged => "Merge state changed",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PullRequestChangeEvent {
    pub repo: RepoId,
    pub number: u64,
    pub title: String,
    pub kind: PullRequestChangeKind,
    pub version: String,
}

impl PullRequestChangeEvent {
    pub fn dedupe_key(&self) -> String {
        format!(
            "{}#{}:{:?}:{}",
            self.repo.full_name(),
            self.number,
            self.kind,
            self.version
        )
    }

    pub fn summary(&self) -> String {
        format!("{} #{}", self.kind.label(), self.number)
    }

    pub fn body(&self) -> String {
        format!("{}: {}", self.repo.full_name(), self.title)
    }
}

pub fn detect_pull_request_changes(
    previous: &[PullRequest],
    current: &[PullRequest],
) -> Vec<PullRequestChangeEvent> {
    let mut events = Vec::new();
    let previous_by_number = previous
        .iter()
        .map(|pull_request| (pull_request.number, pull_request))
        .collect::<HashMap<_, _>>();

    for current_pull_request in current {
        let Some(previous_pull_request) = previous_by_number.get(&current_pull_request.number)
        else {
            events.push(event(
                current_pull_request,
                PullRequestChangeKind::NewPullRequest,
                current_pull_request
                    .updated_at
                    .map(|time| time.to_rfc3339())
                    .unwrap_or_else(|| current_pull_request.head_sha.clone()),
            ));
            continue;
        };

        detect_state_change(previous_pull_request, current_pull_request, &mut events);
        detect_review_decision_change(previous_pull_request, current_pull_request, &mut events);
        detect_review_activity(previous_pull_request, current_pull_request, &mut events);
        detect_check_change(previous_pull_request, current_pull_request, &mut events);
        detect_head_change(previous_pull_request, current_pull_request, &mut events);
        detect_merge_state_change(previous_pull_request, current_pull_request, &mut events);
    }

    events
}

fn detect_state_change(
    previous: &PullRequest,
    current: &PullRequest,
    events: &mut Vec<PullRequestChangeEvent>,
) {
    if previous.state == current.state {
        return;
    }

    let kind = match current.state {
        PullRequestState::Merged => PullRequestChangeKind::Merged,
        PullRequestState::Closed => PullRequestChangeKind::Closed,
        PullRequestState::Open => return,
    };
    events.push(event(current, kind, format!("{:?}", current.state)));
}

fn detect_review_decision_change(
    previous: &PullRequest,
    current: &PullRequest,
    events: &mut Vec<PullRequestChangeEvent>,
) {
    if previous.review_decision == current.review_decision {
        return;
    }

    let kind = match current.review_decision {
        Some(ReviewDecision::Approved) => PullRequestChangeKind::Approved,
        Some(ReviewDecision::ChangesRequested) => PullRequestChangeKind::ChangesRequested,
        _ => PullRequestChangeKind::ReviewDecisionChanged,
    };
    events.push(event(
        current,
        kind,
        format!("{:?}", current.review_decision),
    ));
}

fn detect_review_activity(
    previous: &PullRequest,
    current: &PullRequest,
    events: &mut Vec<PullRequestChangeEvent>,
) {
    let Some(current_updated_at) = current.updated_at else {
        return;
    };

    if previous.updated_at.is_some_and(|previous_updated_at| {
        current_updated_at > previous_updated_at && current.head_sha == previous.head_sha
    }) {
        events.push(event(
            current,
            PullRequestChangeKind::ReviewActivity,
            current_updated_at.to_rfc3339(),
        ));
    }
}

fn detect_check_change(
    previous: &PullRequest,
    current: &PullRequest,
    events: &mut Vec<PullRequestChangeEvent>,
) {
    if previous.checks_summary.failed == 0 && current.checks_summary.failed > 0 {
        events.push(event(
            current,
            PullRequestChangeKind::CheckFailed,
            check_version(current),
        ));
    } else if previous.checks_summary.pending > 0
        && current.checks_summary.pending == 0
        && current.checks_summary.failed == 0
        && current.checks_summary.passed > 0
    {
        events.push(event(
            current,
            PullRequestChangeKind::CheckPassed,
            check_version(current),
        ));
    }
}

fn detect_head_change(
    previous: &PullRequest,
    current: &PullRequest,
    events: &mut Vec<PullRequestChangeEvent>,
) {
    if previous.head_sha != current.head_sha {
        events.push(event(
            current,
            PullRequestChangeKind::HeadChanged,
            current.head_sha.clone(),
        ));
    }
}

fn detect_merge_state_change(
    previous: &PullRequest,
    current: &PullRequest,
    events: &mut Vec<PullRequestChangeEvent>,
) {
    if previous.merge_state != current.merge_state {
        events.push(event(
            current,
            PullRequestChangeKind::MergeStateChanged,
            merge_state_version(current.merge_state),
        ));
    }
}

fn event(
    pull_request: &PullRequest,
    kind: PullRequestChangeKind,
    version: String,
) -> PullRequestChangeEvent {
    PullRequestChangeEvent {
        repo: pull_request.repo.clone(),
        number: pull_request.number,
        title: pull_request.title.clone(),
        kind,
        version,
    }
}

fn check_version(pull_request: &PullRequest) -> String {
    format!(
        "{}:{}:{}:{}",
        pull_request.head_sha,
        pull_request.checks_summary.passed,
        pull_request.checks_summary.failed,
        pull_request.checks_summary.pending
    )
}

fn merge_state_version(state: Option<MergeState>) -> String {
    state
        .map(|state| format!("{state:?}"))
        .unwrap_or_else(|| "none".to_string())
}

#[cfg(test)]
mod tests {
    use chrono::{DateTime, TimeZone, Utc};
    use harbor_domain::{ChecksSummary, PullRequestState};

    use super::*;

    #[test]
    fn detects_major_pull_request_changes() {
        let previous = pull_request(7);
        let mut current = previous.clone();
        current.review_decision = Some(ReviewDecision::ChangesRequested);
        current.checks_summary = ChecksSummary {
            total: 1,
            failed: 1,
            ..ChecksSummary::default()
        };

        let changes = detect_pull_request_changes(&[previous], &[current]);

        assert!(changes.iter().any(|change| {
            change.kind == PullRequestChangeKind::ChangesRequested && change.number == 7
        }));
        assert!(changes.iter().any(|change| {
            change.kind == PullRequestChangeKind::CheckFailed && change.number == 7
        }));
    }

    #[test]
    fn detects_pull_request_changes_with_multiple_previous_rows() {
        let previous = vec![pull_request(7), pull_request(8)];
        let mut current = previous.clone();
        current[1].head_sha = "def456".to_string();

        let changes = detect_pull_request_changes(&previous, &current);

        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].number, 8);
        assert_eq!(changes[0].kind, PullRequestChangeKind::HeadChanged);
        assert_eq!(changes[0].version, "def456");
    }

    #[test]
    fn notification_keys_dedupe_repeated_events() {
        let change = PullRequestChangeEvent {
            repo: RepoId::new("acme", "app"),
            number: 7,
            title: "Add feature".to_string(),
            kind: PullRequestChangeKind::CheckFailed,
            version: "abc:0:1:0".to_string(),
        };

        assert_eq!(
            change.dedupe_key(),
            "acme/app#7:CheckFailed:abc:0:1:0".to_string()
        );
    }

    fn pull_request(number: u64) -> PullRequest {
        PullRequest {
            repo: RepoId::new("acme", "app"),
            node_id: format!("pr-{number}"),
            number,
            title: "Add feature".to_string(),
            body: None,
            author: "octocat".to_string(),
            url: format!("https://github.com/acme/app/pull/{number}"),
            state: PullRequestState::Open,
            is_draft: false,
            head_ref: "feature".to_string(),
            base_ref: "main".to_string(),
            head_sha: "abc123".to_string(),
            review_decision: None,
            merge_state: Some(MergeState::Clean),
            labels: Vec::new(),
            assignees: Vec::new(),
            checks_summary: ChecksSummary {
                total: 1,
                passed: 0,
                failed: 0,
                pending: 1,
                skipped: 0,
            },
            unresolved_threads: 0,
            created_at: Some(time(1)),
            updated_at: Some(time(1)),
        }
    }

    fn time(minute: i64) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 5, 1, 10, minute as u32, 0)
            .single()
            .expect("valid test time")
    }
}
