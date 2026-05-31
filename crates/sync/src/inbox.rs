use std::collections::HashMap;

use async_trait::async_trait;
use harbor_domain::{MergeState, PullRequest, RepoId};
use harbor_github::{
    ConditionalFetch, GitHubError, GitHubRateLimitStatus, HttpCacheValidator,
    PullRequestEnrichment, PullRequestListFilter, PullRequestPage, PullRequestPageCursor,
};
use harbor_storage::{SqliteStore, StoredHttpCacheValidator};

use crate::SyncTarget;

pub const PULL_REQUEST_INBOX_PAGE_SIZE: usize = 10;

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
    pub page_cursor: Option<PullRequestPageCursor>,
    pub previous_pull_requests: &'a [PullRequest],
    pub force_enrichment: bool,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct PullRequestInboxPageInfo {
    pub total_count: Option<usize>,
    pub next_cursor: Option<PullRequestPageCursor>,
}

impl PullRequestInboxPageInfo {
    pub fn from_page(page: &PullRequestPage) -> Self {
        Self {
            total_count: page.total_count,
            next_cursor: page.next_cursor.clone(),
        }
    }

    pub fn complete(total_count: usize) -> Self {
        Self {
            total_count: Some(total_count),
            next_cursor: None,
        }
    }

    pub fn has_next_page(&self) -> bool {
        self.next_cursor.is_some()
    }
}

pub enum PullRequestInboxRefresh {
    Modified {
        pull_requests: Vec<PullRequest>,
        page_info: PullRequestInboxPageInfo,
        enrichment_error: Option<String>,
    },
    NotModified,
}

#[async_trait]
pub trait PullRequestInboxSource: Send + Sync {
    fn latest_rate_limits(&self) -> Vec<GitHubRateLimitStatus>;

    async fn list_repository_pull_request_page(
        &self,
        repository: &RepoId,
        filter: PullRequestListFilter,
        cursor: Option<PullRequestPageCursor>,
        page_size: usize,
    ) -> harbor_github::Result<PullRequestPage>;

    async fn count_repository_pull_requests(
        &self,
        repository: &RepoId,
        filter: PullRequestListFilter,
    ) -> harbor_github::Result<usize>;

    async fn list_repository_pull_requests_light_page(
        &self,
        repository: &RepoId,
        filter: PullRequestListFilter,
        cursor: Option<PullRequestPageCursor>,
        page_size: usize,
        validator: Option<HttpCacheValidator>,
    ) -> harbor_github::Result<ConditionalFetch<PullRequestPage>>;

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
    let is_first_page = request.page_cursor.is_none();

    if !request.mode.uses_rest_light_refresh() {
        tracing::info!(
            repository = %request.repository.full_name(),
            mode = request.mode.key(),
            forced = request.force_enrichment,
            "github graphql source: needs review inbox search"
        );
        let page = source
            .list_repository_pull_request_page(
                request.repository,
                request.mode.list_filter(),
                request.page_cursor.clone(),
                PULL_REQUEST_INBOX_PAGE_SIZE,
            )
            .await?;

        return Ok(PullRequestInboxRefresh::Modified {
            page_info: PullRequestInboxPageInfo::from_page(&page),
            pull_requests: page.pull_requests,
            enrichment_error: None,
        });
    }

    let validator_key = http_validator_key(request.repository, request.mode);
    let validator = if is_first_page && !request.previous_pull_requests.is_empty() {
        match request.store {
            Some(store) => store
                .load_http_cache_validator(&validator_key)
                .await
                .map_err(|error| GitHubError::Transport(error.to_string()))?
                .map(github_validator_from_storage),
            None => None,
        }
    } else {
        None
    };

    let fetch = source
        .list_repository_pull_requests_light_page(
            request.repository,
            request.mode.list_filter(),
            request.page_cursor.clone(),
            PULL_REQUEST_INBOX_PAGE_SIZE,
            validator,
        )
        .await?;

    let (page, validator) = match fetch {
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

    if is_first_page && let (Some(store), Some(validator)) = (request.store, validator) {
        store
            .save_http_cache_validator(&validator_key, &storage_validator_from_github(validator))
            .await
            .map_err(|error| GitHubError::Transport(error.to_string()))?;
    }

    let page_info = PullRequestInboxPageInfo::from_page(&page);
    let mut pull_requests = page.pull_requests;

    merge_light_pull_request_rows(request.previous_pull_requests, &mut pull_requests);

    let node_ids = pull_request_enrichment_node_ids(&pull_requests, request.force_enrichment);
    let enrichment_error = if node_ids.is_empty()
        || (!request.force_enrichment
            && graphql_rate_limit_too_low_for_enrichment(&source.latest_rate_limits()))
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
        page_info,
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
        Ok(PullRequestInboxRefresh::Modified { .. }) => store
            .record_sync_success(&harbor_storage::inbox_target_key(repository, mode.key()))
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

fn graphql_rate_limit_too_low_for_enrichment(rate_limits: &[GitHubRateLimitStatus]) -> bool {
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

#[cfg(test)]
mod tests {
    use chrono::{DateTime, TimeZone, Utc};

    use super::*;
    use harbor_domain::{ChecksSummary, PullRequestState, ReviewDecision};

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
