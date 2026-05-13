#[path = "client/pull_requests.rs"]
mod pull_requests;
#[path = "client/repositories.rs"]
mod repositories;
#[path = "client/requests.rs"]
mod requests;
#[path = "client/reviews.rs"]
mod reviews;
#[path = "client/workflows.rs"]
mod workflows;

use harbor_domain::{ChecksSummary, MergeState, RepoId, ReviewDecision};

use crate::{GitHubRateLimitStatus, GitHubRequestAttribution, GitHubTransport};

#[derive(Clone, Debug)]
pub struct GitHubClient<T> {
    transport: T,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SubmitPullRequestReviewEvent {
    Approve,
    Comment,
    RequestChanges,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PullRequestListFilter {
    Open,
    Closed,
    NeedsReview,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RepositoryList {
    pub repositories: Vec<RepoId>,
    pub possibly_limited: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PullRequestEnrichment {
    pub node_id: String,
    pub review_decision: Option<ReviewDecision>,
    pub merge_state: Option<MergeState>,
    pub checks_summary: ChecksSummary,
}

impl<T> GitHubClient<T> {
    pub fn new(transport: T) -> Self {
        Self { transport }
    }

    pub fn transport(&self) -> &T {
        &self.transport
    }
}

impl<T> GitHubClient<T>
where
    T: GitHubTransport,
{
    pub fn latest_rate_limit(&self) -> Option<GitHubRateLimitStatus> {
        self.transport.latest_rate_limit()
    }

    pub fn latest_rate_limits(&self) -> Vec<GitHubRateLimitStatus> {
        self.transport.latest_rate_limits()
    }

    pub fn latest_request_attribution(&self) -> Option<GitHubRequestAttribution> {
        self.transport.latest_request_attribution()
    }

    pub fn recent_request_attributions(&self) -> Vec<GitHubRequestAttribution> {
        self.transport.recent_request_attributions()
    }
}

#[cfg(test)]
#[path = "client/test_support.rs"]
mod test_support;

#[cfg(test)]
#[path = "client/tests.rs"]
mod tests;
