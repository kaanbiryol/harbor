mod client;
mod dto;
mod transport;

pub use client::{
    GitHubClient, PullRequestEnrichment, PullRequestListFilter, PullRequestPage,
    PullRequestPageCursor, RepositoryList, SubmitPullRequestReviewEvent,
};
pub use transport::{
    GhCliTransport, GitHubDeviceFlow, GitHubTransport, GitHubTransportSource, OctocrabTransport,
    start_oauth_device_flow,
};

pub type Result<T> = std::result::Result<T, GitHubError>;

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct GitHubRateLimitStatus {
    pub retry_after_seconds: Option<u64>,
    pub reset_epoch_seconds: Option<u64>,
    pub resource: Option<String>,
    pub remaining: Option<u64>,
    pub limit: Option<u64>,
    pub used: Option<u64>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct GitHubRateLimit {
    pub message: String,
    pub retry_after_seconds: Option<u64>,
    pub reset_epoch_seconds: Option<u64>,
    pub resource: Option<String>,
    pub remaining: Option<u64>,
    pub limit: Option<u64>,
    pub used: Option<u64>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct HttpCacheValidator {
    pub etag: Option<String>,
    pub last_modified: Option<String>,
}

impl HttpCacheValidator {
    pub fn is_empty(&self) -> bool {
        self.etag.is_none() && self.last_modified.is_none()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ConditionalFetch<T> {
    Modified {
        value: T,
        validator: Option<HttpCacheValidator>,
    },
    NotModified {
        validator: Option<HttpCacheValidator>,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GitHubApiFamily {
    Rest,
    GraphQl,
}

impl GitHubApiFamily {
    pub fn label(self) -> &'static str {
        match self {
            Self::Rest => "rest",
            Self::GraphQl => "graphql",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GitHubRequestAttribution {
    pub operation_name: String,
    pub family: GitHubApiFamily,
    pub resource: Option<String>,
    pub graphql_cost: Option<u64>,
    pub remaining: Option<u64>,
    pub limit: Option<u64>,
    pub used: Option<u64>,
    pub spent: Option<u64>,
    pub duration_ms: u64,
}

#[derive(Clone, Debug, thiserror::Error)]
pub enum GitHubError {
    #[error("github authentication is required")]
    Unauthenticated,
    #[error("github cli is not installed or not available on PATH")]
    MissingCli,
    #[error("github cli is not authenticated")]
    UnauthenticatedCli,
    #[error("github rate limit exceeded: {}", .0.message)]
    RateLimited(Box<GitHubRateLimit>),
    #[error("github secondary rate limit exceeded: {}", .0.message)]
    SecondaryRateLimited(Box<GitHubRateLimit>),
    #[error("github transport failed: {0}")]
    Transport(String),
    #[error("github request budget exceeded: {0}")]
    RequestBudget(String),
    #[error("github response could not be mapped into the domain model: {0}")]
    Mapping(String),
}
