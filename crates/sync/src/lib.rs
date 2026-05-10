use std::time::Duration;

use chrono::{DateTime, Utc};
use harbor_domain::{
    ChecksSummary, MergeState, PullRequest, PullRequestState, RepoId, ReviewDecision,
    WorkflowConclusion, WorkflowRun, WorkflowStatus,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ActivityState {
    Focused,
    Background,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum SyncTarget {
    ActiveInbox,
    SelectedPullRequestMetadata,
    SelectedPullRequestReviews,
    SelectedPullRequestChecks,
    SelectedPullRequestWorkflows,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SyncReason {
    Scheduled,
    Manual,
    Startup,
    RepositorySwitch,
    FocusGained,
    LocalMutation,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SyncDecision {
    RunNow,
    Wait(Duration),
    SkipInFlight,
    Backoff(Duration),
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct SyncState {
    pub last_successful_fetch_at: Option<DateTime<Utc>>,
    pub last_attempt_at: Option<DateTime<Utc>>,
    pub failure_count: u32,
    pub stale: bool,
    pub in_flight: bool,
}

impl SyncState {
    pub fn mark_attempt(&mut self, now: DateTime<Utc>) {
        self.last_attempt_at = Some(now);
        self.in_flight = true;
    }

    pub fn mark_success(&mut self, now: DateTime<Utc>) {
        self.last_successful_fetch_at = Some(now);
        self.last_attempt_at = Some(now);
        self.failure_count = 0;
        self.stale = false;
        self.in_flight = false;
    }

    pub fn mark_failure(&mut self) {
        self.failure_count = self.failure_count.saturating_add(1);
        self.in_flight = false;
    }

    pub fn mark_stale(&mut self) {
        self.stale = true;
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SyncSignals {
    pub has_running_or_pending_checks: bool,
    pub has_running_workflows: bool,
    pub selected_pr_visible: bool,
}

impl Default for SyncSignals {
    fn default() -> Self {
        Self {
            has_running_or_pending_checks: false,
            has_running_workflows: false,
            selected_pr_visible: true,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SyncPolicy {
    pub focused_inbox_interval: Duration,
    pub focused_inbox_running_checks_interval: Duration,
    pub background_inbox_interval: Duration,
    pub focus_catch_up_after: Duration,
    pub selected_pr_metadata_interval: Duration,
    pub selected_pr_reviews_interval: Duration,
    pub selected_pr_checks_running_interval: Duration,
    pub selected_pr_checks_terminal_interval: Duration,
    pub selected_pr_workflows_running_interval: Duration,
}

impl Default for SyncPolicy {
    fn default() -> Self {
        Self {
            focused_inbox_interval: Duration::from_secs(120),
            focused_inbox_running_checks_interval: Duration::from_secs(45),
            background_inbox_interval: Duration::from_secs(300),
            focus_catch_up_after: Duration::from_secs(30),
            selected_pr_metadata_interval: Duration::from_secs(60),
            selected_pr_reviews_interval: Duration::from_secs(60),
            selected_pr_checks_running_interval: Duration::from_secs(15),
            selected_pr_checks_terminal_interval: Duration::from_secs(60),
            selected_pr_workflows_running_interval: Duration::from_secs(30),
        }
    }
}

impl SyncPolicy {
    pub fn decision(
        &self,
        target: SyncTarget,
        reason: SyncReason,
        activity: ActivityState,
        state: &SyncState,
        signals: SyncSignals,
        now: DateTime<Utc>,
    ) -> SyncDecision {
        if state.in_flight {
            return SyncDecision::SkipInFlight;
        }

        if matches!(
            reason,
            SyncReason::Manual
                | SyncReason::Startup
                | SyncReason::RepositorySwitch
                | SyncReason::LocalMutation
        ) || state.stale
        {
            return SyncDecision::RunNow;
        }

        if reason == SyncReason::FocusGained && self.is_due(state, self.focus_catch_up_after, now) {
            return SyncDecision::RunNow;
        }

        if let Some(backoff) = SyncBackoff::for_failure_count(state.failure_count)
            .and_then(|backoff| backoff.remaining_since(state.last_attempt_at, now))
        {
            return SyncDecision::Backoff(backoff);
        }

        let interval = self.interval(target, activity, signals);
        if target != SyncTarget::ActiveInbox && activity == ActivityState::Background {
            return SyncDecision::Wait(interval);
        }

        match state.last_successful_fetch_at {
            None => SyncDecision::RunNow,
            Some(last_successful_fetch_at) => {
                let elapsed = now
                    .signed_duration_since(last_successful_fetch_at)
                    .to_std()
                    .unwrap_or_default();
                if elapsed >= interval {
                    SyncDecision::RunNow
                } else {
                    SyncDecision::Wait(interval - elapsed)
                }
            }
        }
    }

    pub fn interval(
        &self,
        target: SyncTarget,
        activity: ActivityState,
        signals: SyncSignals,
    ) -> Duration {
        match (target, activity) {
            (SyncTarget::ActiveInbox, ActivityState::Background) => self.background_inbox_interval,
            (SyncTarget::ActiveInbox, ActivityState::Focused)
                if signals.has_running_or_pending_checks =>
            {
                self.focused_inbox_running_checks_interval
            }
            (SyncTarget::ActiveInbox, ActivityState::Focused) => self.focused_inbox_interval,
            (SyncTarget::SelectedPullRequestMetadata, _) => self.selected_pr_metadata_interval,
            (SyncTarget::SelectedPullRequestReviews, _) => self.selected_pr_reviews_interval,
            (SyncTarget::SelectedPullRequestChecks, _) if signals.has_running_or_pending_checks => {
                self.selected_pr_checks_running_interval
            }
            (SyncTarget::SelectedPullRequestChecks, _) => self.selected_pr_checks_terminal_interval,
            (SyncTarget::SelectedPullRequestWorkflows, _) => {
                self.selected_pr_workflows_running_interval
            }
        }
    }

    fn is_due(&self, state: &SyncState, interval: Duration, now: DateTime<Utc>) -> bool {
        state.last_successful_fetch_at.is_none_or(|last| {
            now.signed_duration_since(last).to_std().unwrap_or_default() >= interval
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SyncBackoff {
    delay: Duration,
}

impl SyncBackoff {
    pub fn for_failure_count(failure_count: u32) -> Option<Self> {
        if failure_count == 0 {
            return None;
        }

        let exponent = failure_count.saturating_sub(1).min(5);
        Some(Self {
            delay: Duration::from_secs(30 * 2_u64.pow(exponent)),
        })
    }

    pub fn remaining_since(
        self,
        last_attempt_at: Option<DateTime<Utc>>,
        now: DateTime<Utc>,
    ) -> Option<Duration> {
        let last_attempt_at = last_attempt_at?;
        let elapsed = now
            .signed_duration_since(last_attempt_at)
            .to_std()
            .unwrap_or_default();
        self.delay.checked_sub(elapsed)
    }
}

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

    for current_pull_request in current {
        let Some(previous_pull_request) = previous
            .iter()
            .find(|pull_request| pull_request.number == current_pull_request.number)
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

pub fn workflow_runs_have_running_work(workflow_runs: &[WorkflowRun]) -> bool {
    workflow_runs.iter().any(|run| {
        run.status != WorkflowStatus::Completed
            || matches!(
                run.conclusion,
                None | Some(WorkflowConclusion::ActionRequired)
            )
    })
}

pub fn checks_have_running_or_pending_work(summary: ChecksSummary) -> bool {
    summary.pending > 0
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
    use chrono::TimeZone;

    use super::*;
    use harbor_domain::{ChecksSummary, PullRequestState};

    #[test]
    fn focused_inbox_refreshes_every_two_minutes() {
        let policy = SyncPolicy::default();
        let now = time(10);
        let state = SyncState {
            last_successful_fetch_at: Some(time(9)),
            ..SyncState::default()
        };

        assert_eq!(
            policy.decision(
                SyncTarget::ActiveInbox,
                SyncReason::Scheduled,
                ActivityState::Focused,
                &state,
                SyncSignals::default(),
                now,
            ),
            SyncDecision::Wait(Duration::from_secs(60))
        );

        assert_eq!(
            policy.decision(
                SyncTarget::ActiveInbox,
                SyncReason::Scheduled,
                ActivityState::Focused,
                &SyncState {
                    last_successful_fetch_at: Some(time(8)),
                    ..SyncState::default()
                },
                SyncSignals::default(),
                now,
            ),
            SyncDecision::RunNow
        );
    }

    #[test]
    fn background_inbox_refreshes_every_five_minutes() {
        let policy = SyncPolicy::default();
        let now = time(10);

        assert_eq!(
            policy.decision(
                SyncTarget::ActiveInbox,
                SyncReason::Scheduled,
                ActivityState::Background,
                &SyncState {
                    last_successful_fetch_at: Some(time(6)),
                    ..SyncState::default()
                },
                SyncSignals::default(),
                now,
            ),
            SyncDecision::Wait(Duration::from_secs(60))
        );

        assert_eq!(
            policy.decision(
                SyncTarget::ActiveInbox,
                SyncReason::Scheduled,
                ActivityState::Background,
                &SyncState {
                    last_successful_fetch_at: Some(time(5)),
                    ..SyncState::default()
                },
                SyncSignals::default(),
                now,
            ),
            SyncDecision::RunNow
        );
    }

    #[test]
    fn running_checks_accelerate_inbox_refresh() {
        let policy = SyncPolicy::default();
        let now = time(10);

        assert_eq!(
            policy.decision(
                SyncTarget::ActiveInbox,
                SyncReason::Scheduled,
                ActivityState::Focused,
                &SyncState {
                    last_successful_fetch_at: Some(now - chrono::Duration::seconds(44)),
                    ..SyncState::default()
                },
                SyncSignals {
                    has_running_or_pending_checks: true,
                    ..SyncSignals::default()
                },
                now,
            ),
            SyncDecision::Wait(Duration::from_secs(1))
        );

        assert_eq!(
            policy.decision(
                SyncTarget::ActiveInbox,
                SyncReason::Scheduled,
                ActivityState::Focused,
                &SyncState {
                    last_successful_fetch_at: Some(now - chrono::Duration::seconds(45)),
                    ..SyncState::default()
                },
                SyncSignals {
                    has_running_or_pending_checks: true,
                    ..SyncSignals::default()
                },
                now,
            ),
            SyncDecision::RunNow
        );
    }

    #[test]
    fn manual_and_stale_sync_run_immediately() {
        let policy = SyncPolicy::default();
        let now = time(10);
        let fresh = SyncState {
            last_successful_fetch_at: Some(now),
            ..SyncState::default()
        };

        assert_eq!(
            policy.decision(
                SyncTarget::ActiveInbox,
                SyncReason::Manual,
                ActivityState::Focused,
                &fresh,
                SyncSignals::default(),
                now,
            ),
            SyncDecision::RunNow
        );

        assert_eq!(
            policy.decision(
                SyncTarget::ActiveInbox,
                SyncReason::Scheduled,
                ActivityState::Focused,
                &SyncState {
                    stale: true,
                    ..fresh
                },
                SyncSignals::default(),
                now,
            ),
            SyncDecision::RunNow
        );
    }

    #[test]
    fn failures_back_off_scheduled_work() {
        let policy = SyncPolicy::default();
        let now = time(10);
        let state = SyncState {
            last_successful_fetch_at: Some(time(1)),
            last_attempt_at: Some(now - chrono::Duration::seconds(10)),
            failure_count: 1,
            ..SyncState::default()
        };

        assert_eq!(
            policy.decision(
                SyncTarget::ActiveInbox,
                SyncReason::Scheduled,
                ActivityState::Focused,
                &state,
                SyncSignals::default(),
                now,
            ),
            SyncDecision::Backoff(Duration::from_secs(20))
        );
    }

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
            checks_summary: ChecksSummary {
                total: 1,
                passed: 0,
                failed: 0,
                pending: 1,
                skipped: 0,
            },
            unresolved_threads: 0,
            updated_at: Some(time(1)),
        }
    }

    fn time(minute: i64) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 5, 1, 10, minute as u32, 0)
            .single()
            .expect("valid test time")
    }
}
