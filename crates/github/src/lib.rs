mod client;
mod dto;
mod transport;

pub use client::{GitHubClient, PullRequestListFilter, SubmitPullRequestReviewEvent};
pub use transport::{GhCliTransport, GitHubTransport};

pub type Result<T> = std::result::Result<T, GitHubError>;

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct GitHubRateLimitStatus {
    pub retry_after_seconds: Option<u64>,
    pub reset_epoch_seconds: Option<u64>,
    pub resource: Option<String>,
    pub remaining: Option<u64>,
    pub limit: Option<u64>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct GitHubRateLimit {
    pub message: String,
    pub retry_after_seconds: Option<u64>,
    pub reset_epoch_seconds: Option<u64>,
    pub resource: Option<String>,
    pub remaining: Option<u64>,
    pub limit: Option<u64>,
}

#[derive(Clone, Debug, thiserror::Error)]
pub enum GitHubError {
    #[error("github cli is not installed or not available on PATH")]
    MissingCli,
    #[error("github cli is not authenticated")]
    UnauthenticatedCli,
    #[error("github rate limit exceeded: {}", .0.message)]
    RateLimited(GitHubRateLimit),
    #[error("github secondary rate limit exceeded: {}", .0.message)]
    SecondaryRateLimited(GitHubRateLimit),
    #[error("github transport failed: {0}")]
    Transport(String),
    #[error("github request budget exceeded: {0}")]
    RequestBudget(String),
    #[error("github response could not be mapped into the domain model: {0}")]
    Mapping(String),
}
