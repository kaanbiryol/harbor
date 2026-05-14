use std::{collections::HashMap, time::Duration};

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use harbor_domain::{
    ChecksSummary, MergeState, PullRequest, PullRequestState, RepoId, ReviewDecision,
    WorkflowConclusion, WorkflowRun, WorkflowStatus,
};
use harbor_github::{
    ConditionalFetch, GitHubError, GitHubRateLimitStatus, HttpCacheValidator,
    PullRequestEnrichment, PullRequestListFilter,
};
use harbor_storage::{SqliteStore, StoredHttpCacheValidator};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ActivityState {
    Focused,
    Background,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum SyncTarget {
    ActiveInbox,
    ActiveInboxLight,
    ActiveInboxEnrichment,
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InboxRefreshKind {
    RestLight,
    GraphQlSearch,
    GraphQlEnrichment,
}

#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub enum PullRequestInboxMode {
    #[default]
    Open,
    Closed,
    NeedsReview,
}

impl PullRequestInboxMode {
    pub const ALL: [Self; 3] = [Self::Open, Self::Closed, Self::NeedsReview];

    pub fn label(self) -> &'static str {
        match self {
            Self::Open => "Open",
            Self::Closed => "Closed",
            Self::NeedsReview => "Needs review",
        }
    }

    pub fn status_label(self) -> &'static str {
        match self {
            Self::Open => "open pull requests",
            Self::Closed => "closed pull requests",
            Self::NeedsReview => "pull requests requesting your review",
        }
    }

    pub fn empty_message(self) -> &'static str {
        match self {
            Self::Open => "No open pull requests",
            Self::Closed => "No closed pull requests",
            Self::NeedsReview => "No pull requests require your review",
        }
    }

    pub fn key(self) -> &'static str {
        match self {
            Self::Open => "open",
            Self::Closed => "closed",
            Self::NeedsReview => "needs-review",
        }
    }

    pub fn list_filter(self) -> PullRequestListFilter {
        match self {
            Self::Open => PullRequestListFilter::Open,
            Self::Closed => PullRequestListFilter::Closed,
            Self::NeedsReview => PullRequestListFilter::NeedsReview,
        }
    }

    pub fn refresh_kind(self) -> InboxRefreshKind {
        if self.uses_rest_light_refresh() {
            InboxRefreshKind::RestLight
        } else {
            InboxRefreshKind::GraphQlSearch
        }
    }

    pub fn active_sync_target(self) -> SyncTarget {
        if self.uses_rest_light_refresh() {
            SyncTarget::ActiveInboxLight
        } else {
            SyncTarget::ActiveInbox
        }
    }

    fn uses_rest_light_refresh(self) -> bool {
        matches!(self, Self::Open | Self::Closed)
    }
}

pub struct PullRequestInboxRefreshRequest<'a> {
    pub store: Option<&'a SqliteStore>,
    pub repository: &'a RepoId,
    pub mode: PullRequestInboxMode,
    pub previous_pull_requests: &'a [PullRequest],
    pub force_enrichment: bool,
}

pub enum PullRequestInboxRefresh {
    Modified {
        pull_requests: Vec<PullRequest>,
        enrichment_error: Option<String>,
    },
    NotModified,
}

#[async_trait]
pub trait PullRequestInboxSource: Send + Sync {
    fn latest_rate_limits(&self) -> Vec<GitHubRateLimitStatus>;

    async fn list_repository_pull_requests(
        &self,
        repository: &RepoId,
        filter: PullRequestListFilter,
    ) -> harbor_github::Result<Vec<PullRequest>>;

    async fn count_repository_pull_requests(
        &self,
        repository: &RepoId,
        filter: PullRequestListFilter,
    ) -> harbor_github::Result<usize>;

    async fn list_repository_pull_requests_light(
        &self,
        repository: &RepoId,
        filter: PullRequestListFilter,
        validator: Option<HttpCacheValidator>,
    ) -> harbor_github::Result<ConditionalFetch<Vec<PullRequest>>>;

    async fn enrich_pull_requests_by_node_ids(
        &self,
        node_ids: &[String],
    ) -> harbor_github::Result<Vec<PullRequestEnrichment>>;
}

pub async fn refresh_pull_request_inbox<S>(
    source: &S,
    request: PullRequestInboxRefreshRequest<'_>,
) -> std::result::Result<PullRequestInboxRefresh, GitHubError>
where
    S: PullRequestInboxSource + ?Sized,
{
    if !request.mode.uses_rest_light_refresh() {
        tracing::info!(
            repository = %request.repository.full_name(),
            mode = request.mode.key(),
            forced = request.force_enrichment,
            "github graphql source: needs review inbox search"
        );
        return source
            .list_repository_pull_requests(request.repository, request.mode.list_filter())
            .await
            .map(|pull_requests| PullRequestInboxRefresh::Modified {
                pull_requests,
                enrichment_error: None,
            });
    }

    let validator_key = http_validator_key(request.repository, request.mode);
    let validator = match request.store {
        Some(store) => store
            .load_http_cache_validator(&validator_key)
            .await
            .map_err(|error| GitHubError::Transport(error.to_string()))?
            .map(github_validator_from_storage),
        None => None,
    };

    let fetch = source
        .list_repository_pull_requests_light(
            request.repository,
            request.mode.list_filter(),
            validator,
        )
        .await?;

    let (mut pull_requests, validator) = match fetch {
        ConditionalFetch::NotModified { validator } => {
            if let (Some(store), Some(validator)) = (request.store, validator) {
                store
                    .save_http_cache_validator(
                        &validator_key,
                        &storage_validator_from_github(validator),
                    )
                    .await
                    .map_err(|error| GitHubError::Transport(error.to_string()))?;
            }
            return Ok(PullRequestInboxRefresh::NotModified);
        }
        ConditionalFetch::Modified { value, validator } => (value, validator),
    };

    if let (Some(store), Some(validator)) = (request.store, validator) {
        store
            .save_http_cache_validator(&validator_key, &storage_validator_from_github(validator))
            .await
            .map_err(|error| GitHubError::Transport(error.to_string()))?;
    }

    merge_light_pull_request_rows(request.previous_pull_requests, &mut pull_requests);

    let node_ids = pull_request_enrichment_node_ids(&pull_requests, request.force_enrichment);
    let enrichment_error = if node_ids.is_empty()
        || (!request.force_enrichment && graphql_rate_limit_is_low(&source.latest_rate_limits()))
    {
        None
    } else {
        tracing::info!(
            repository = %request.repository.full_name(),
            mode = request.mode.key(),
            pull_request_count = node_ids.len(),
            forced = request.force_enrichment,
            "github graphql source: pull request row enrichment"
        );
        match source.enrich_pull_requests_by_node_ids(&node_ids).await {
            Ok(enrichments) => {
                apply_pull_request_enrichments(&mut pull_requests, enrichments);
                None
            }
            Err(error) => Some(error.to_string()),
        }
    };

    Ok(PullRequestInboxRefresh::Modified {
        pull_requests,
        enrichment_error,
    })
}

pub async fn cache_pull_request_inbox_refresh(
    store: Option<&SqliteStore>,
    repository: &RepoId,
    mode: PullRequestInboxMode,
    refresh: &std::result::Result<PullRequestInboxRefresh, GitHubError>,
) -> std::result::Result<(), String> {
    let Some(store) = store else {
        return Ok(());
    };

    match refresh {
        Ok(PullRequestInboxRefresh::Modified { pull_requests, .. }) => store
            .save_pull_request_inbox(repository, mode.key(), pull_requests)
            .await
            .map_err(|error| error.to_string()),
        Ok(PullRequestInboxRefresh::NotModified) => store
            .record_sync_success(&harbor_storage::inbox_target_key(repository, mode.key()))
            .await
            .map_err(|error| error.to_string()),
        Err(error) => store
            .record_sync_failure(
                &harbor_storage::inbox_target_key(repository, mode.key()),
                &error.to_string(),
            )
            .await
            .map_err(|error| error.to_string()),
    }
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
    pub focused_inbox_graphql_search_interval: Duration,
    pub background_inbox_interval: Duration,
    pub background_inbox_graphql_search_interval: Duration,
    pub focused_inbox_enrichment_interval: Duration,
    pub background_inbox_enrichment_interval: Duration,
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
            focused_inbox_graphql_search_interval: Duration::from_secs(300),
            background_inbox_interval: Duration::from_secs(300),
            background_inbox_graphql_search_interval: Duration::from_secs(1_800),
            focused_inbox_enrichment_interval: Duration::from_secs(1_800),
            background_inbox_enrichment_interval: Duration::from_secs(3_600),
            focus_catch_up_after: Duration::from_secs(30),
            selected_pr_metadata_interval: Duration::from_secs(120),
            selected_pr_reviews_interval: Duration::from_secs(120),
            selected_pr_checks_running_interval: Duration::from_secs(15),
            selected_pr_checks_terminal_interval: Duration::from_secs(60),
            selected_pr_workflows_running_interval: Duration::from_secs(60),
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

        if reason == SyncReason::FocusGained
            && self.is_due(state, self.focus_catch_up_interval(target), now)
        {
            return SyncDecision::RunNow;
        }

        if let Some(backoff) = SyncBackoff::for_failure_count(state.failure_count)
            .and_then(|backoff| backoff.remaining_since(state.last_attempt_at, now))
        {
            return SyncDecision::Backoff(backoff);
        }

        let interval = self.interval(target, activity, signals);
        if !matches!(
            target,
            SyncTarget::ActiveInbox
                | SyncTarget::ActiveInboxLight
                | SyncTarget::ActiveInboxEnrichment
        ) && activity == ActivityState::Background
        {
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
            (SyncTarget::ActiveInboxLight, ActivityState::Background) => {
                self.background_inbox_interval
            }
            (SyncTarget::ActiveInboxLight, ActivityState::Focused) => self.focused_inbox_interval,
            (SyncTarget::ActiveInboxEnrichment, ActivityState::Background) => {
                self.background_inbox_enrichment_interval
            }
            (SyncTarget::ActiveInboxEnrichment, ActivityState::Focused) => {
                self.focused_inbox_enrichment_interval
            }
            (SyncTarget::ActiveInbox, ActivityState::Background) => {
                self.background_inbox_graphql_search_interval
            }
            (SyncTarget::ActiveInbox, ActivityState::Focused) => {
                self.focused_inbox_graphql_search_interval
            }
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

    fn focus_catch_up_interval(&self, target: SyncTarget) -> Duration {
        match target {
            SyncTarget::ActiveInbox => self.focused_inbox_graphql_search_interval,
            _ => self.focus_catch_up_after,
        }
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

fn http_validator_key(repository: &RepoId, mode: PullRequestInboxMode) -> String {
    format!("rest-inbox:{}:{}", repository.full_name(), mode.key())
}

fn github_validator_from_storage(validator: StoredHttpCacheValidator) -> HttpCacheValidator {
    HttpCacheValidator {
        etag: validator.etag,
        last_modified: validator.last_modified,
    }
}

fn storage_validator_from_github(validator: HttpCacheValidator) -> StoredHttpCacheValidator {
    StoredHttpCacheValidator {
        etag: validator.etag,
        last_modified: validator.last_modified,
    }
}

fn merge_light_pull_request_rows(previous: &[PullRequest], current: &mut [PullRequest]) {
    let previous_by_number = previous
        .iter()
        .map(|pull_request| (pull_request.number, pull_request))
        .collect::<HashMap<_, _>>();

    for pull_request in current {
        let Some(previous_pull_request) = previous_by_number.get(&pull_request.number) else {
            continue;
        };

        if previous_pull_request.head_sha != pull_request.head_sha {
            continue;
        }

        if pull_request.node_id.is_empty() {
            pull_request.node_id = previous_pull_request.node_id.clone();
        }
        pull_request.review_decision = previous_pull_request.review_decision;
        pull_request.checks_summary = previous_pull_request.checks_summary;
        pull_request.unresolved_threads = previous_pull_request.unresolved_threads;
        if pull_request.merge_state == Some(MergeState::Unknown)
            || pull_request.merge_state.is_none()
        {
            pull_request.merge_state = previous_pull_request.merge_state;
        }
    }
}

fn pull_request_enrichment_node_ids(
    current: &[PullRequest],
    force_enrichment: bool,
) -> Vec<String> {
    if !force_enrichment {
        return Vec::new();
    }

    current
        .iter()
        .filter(|pull_request| !pull_request.node_id.is_empty())
        .map(|pull_request| pull_request.node_id.clone())
        .collect()
}

fn apply_pull_request_enrichments(
    pull_requests: &mut [PullRequest],
    enrichments: Vec<PullRequestEnrichment>,
) {
    let mut enrichments_by_node_id = enrichments
        .into_iter()
        .map(|enrichment| (enrichment.node_id.clone(), enrichment))
        .collect::<HashMap<_, _>>();

    for pull_request in pull_requests {
        let Some(enrichment) = enrichments_by_node_id.remove(&pull_request.node_id) else {
            continue;
        };

        pull_request.review_decision = enrichment.review_decision;
        pull_request.merge_state = enrichment.merge_state;
    }
}

fn graphql_rate_limit_is_low(rate_limits: &[GitHubRateLimitStatus]) -> bool {
    rate_limits.iter().any(|rate_limit| {
        rate_limit.resource.as_deref() == Some("graphql")
            && match (rate_limit.remaining, rate_limit.limit) {
                (Some(remaining), Some(limit)) if limit > 0 => {
                    remaining <= 500 || remaining.saturating_mul(10) <= limit
                }
                (Some(remaining), None) => remaining <= 500,
                _ => false,
            }
    })
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
                SyncTarget::ActiveInboxLight,
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
                SyncTarget::ActiveInboxLight,
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
    fn needs_review_graphql_search_uses_slower_cadence() {
        let policy = SyncPolicy::default();
        let now = time(10);

        assert_eq!(
            policy.decision(
                SyncTarget::ActiveInbox,
                SyncReason::Scheduled,
                ActivityState::Focused,
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
                ActivityState::Focused,
                &SyncState {
                    last_successful_fetch_at: Some(time(5)),
                    ..SyncState::default()
                },
                SyncSignals::default(),
                now,
            ),
            SyncDecision::RunNow
        );

        assert_eq!(
            policy.decision(
                SyncTarget::ActiveInbox,
                SyncReason::Scheduled,
                ActivityState::Background,
                &SyncState {
                    last_successful_fetch_at: Some(now - chrono::Duration::seconds(1_740)),
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
                    last_successful_fetch_at: Some(now - chrono::Duration::seconds(1_800)),
                    ..SyncState::default()
                },
                SyncSignals::default(),
                now,
            ),
            SyncDecision::RunNow
        );
    }

    #[test]
    fn focus_catch_up_uses_graphql_search_cadence_for_needs_review() {
        let policy = SyncPolicy::default();
        let now = time(10);

        assert_eq!(
            policy.decision(
                SyncTarget::ActiveInbox,
                SyncReason::FocusGained,
                ActivityState::Focused,
                &SyncState {
                    last_successful_fetch_at: Some(now - chrono::Duration::seconds(31)),
                    ..SyncState::default()
                },
                SyncSignals::default(),
                now,
            ),
            SyncDecision::Wait(Duration::from_secs(269))
        );

        assert_eq!(
            policy.decision(
                SyncTarget::ActiveInboxLight,
                SyncReason::FocusGained,
                ActivityState::Focused,
                &SyncState {
                    last_successful_fetch_at: Some(now - chrono::Duration::seconds(31)),
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
                SyncTarget::ActiveInboxLight,
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
                SyncTarget::ActiveInboxLight,
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
    fn running_checks_do_not_accelerate_full_inbox_refresh() {
        let policy = SyncPolicy::default();
        let now = time(10);

        assert_eq!(
            policy.decision(
                SyncTarget::ActiveInboxLight,
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
            SyncDecision::Wait(Duration::from_secs(76))
        );

        assert_eq!(
            policy.decision(
                SyncTarget::ActiveInboxLight,
                SyncReason::Scheduled,
                ActivityState::Focused,
                &SyncState {
                    last_successful_fetch_at: Some(now - chrono::Duration::seconds(120)),
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
    fn selected_metadata_refreshes_every_two_minutes() {
        let policy = SyncPolicy::default();
        let now = time(10);

        assert_eq!(
            policy.decision(
                SyncTarget::SelectedPullRequestMetadata,
                SyncReason::Scheduled,
                ActivityState::Focused,
                &SyncState {
                    last_successful_fetch_at: Some(time(9)),
                    ..SyncState::default()
                },
                SyncSignals::default(),
                now,
            ),
            SyncDecision::Wait(Duration::from_secs(60))
        );

        assert_eq!(
            policy.decision(
                SyncTarget::SelectedPullRequestMetadata,
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
    fn light_pull_request_merge_preserves_order_and_matching_row_fields() {
        let mut previous = pull_request(7);
        previous.node_id = "node-7".to_string();
        previous.review_decision = Some(ReviewDecision::Approved);
        previous.unresolved_threads = 3;
        previous.checks_summary = ChecksSummary {
            total: 2,
            passed: 1,
            failed: 1,
            pending: 0,
            skipped: 0,
        };
        let mut other_previous = pull_request(8);
        other_previous.node_id = "node-8".to_string();
        let mut current = vec![pull_request(8), pull_request(7)];
        current[0].node_id.clear();
        current[1].node_id.clear();

        merge_light_pull_request_rows(&[previous, other_previous], &mut current);

        assert_eq!(current[0].number, 8);
        assert_eq!(current[0].node_id, "node-8");
        assert_eq!(current[1].number, 7);
        assert_eq!(current[1].node_id, "node-7");
        assert_eq!(current[1].review_decision, Some(ReviewDecision::Approved));
        assert_eq!(current[1].unresolved_threads, 3);
        assert_eq!(current[1].checks_summary.failed, 1);
    }

    #[test]
    fn enrichment_application_preserves_order_and_updates_matching_rows() {
        let mut pull_requests = vec![pull_request(7), pull_request(8)];
        pull_requests[0].node_id = "node-7".to_string();
        pull_requests[1].node_id = "node-8".to_string();
        let enrichments = vec![PullRequestEnrichment {
            node_id: "node-8".to_string(),
            review_decision: Some(ReviewDecision::ChangesRequested),
            merge_state: Some(MergeState::Blocked),
            checks_summary: ChecksSummary::default(),
        }];

        apply_pull_request_enrichments(&mut pull_requests, enrichments);

        assert_eq!(pull_requests[0].number, 7);
        assert_eq!(pull_requests[0].review_decision, None);
        assert_eq!(pull_requests[1].number, 8);
        assert_eq!(
            pull_requests[1].review_decision,
            Some(ReviewDecision::ChangesRequested)
        );
        assert_eq!(pull_requests[1].merge_state, Some(MergeState::Blocked));
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
