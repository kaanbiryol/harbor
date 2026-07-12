use harbor_domain::{PullRequest, RepoId};
use harbor_github::{
    GitHubError, PullRequestInboxSource, PullRequestListFilter, PullRequestPage,
    PullRequestPageCursor,
};
use harbor_storage::SqliteStore;

use crate::SyncTarget;

#[path = "inbox/cache.rs"]
mod cache;
#[path = "inbox/enrichment.rs"]
mod enrichment;
#[path = "inbox/http_cache.rs"]
mod http_cache;
#[path = "inbox/light_refresh.rs"]
mod light_refresh;

pub use cache::cache_pull_request_inbox_refresh;
use light_refresh::refresh_light_pull_request_inbox;

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
        let page = source
            .list_repository_pull_request_page(
                request.repository,
                request.mode.list_filter(),
                request.page_cursor,
                PULL_REQUEST_INBOX_PAGE_SIZE,
            )
            .await?;

        return Ok(PullRequestInboxRefresh::Modified {
            page_info: PullRequestInboxPageInfo::from_page(&page),
            pull_requests: page.pull_requests,
            enrichment_error: None,
        });
    }

    refresh_light_pull_request_inbox(
        source,
        request.store,
        request.repository,
        request.mode,
        request.page_cursor,
        request.previous_pull_requests,
        request.force_enrichment,
    )
    .await
}
