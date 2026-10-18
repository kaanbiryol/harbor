mod pull_requests;
mod repositories;
mod requests;
mod reviews;
mod workflows;

use harbor_domain::{PullRequest, RepoId, SubmitPullRequestReviewEvent, WorkflowRun};

use crate::{GitHubRateLimitStatus, GitHubRequestAttribution, GitHubTransport};

#[derive(Clone, Debug)]
pub struct GitHubClient<T> {
    transport: T,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PullRequestListFilter {
    Open,
    Closed,
    NeedsReview,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PullRequestPageCursor {
    GraphQl(String),
    RestPage(usize),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PullRequestPage {
    pub pull_requests: Vec<PullRequest>,
    pub total_count: Option<usize>,
    pub next_cursor: Option<PullRequestPageCursor>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkflowRunPage {
    pub workflow_runs: Vec<WorkflowRun>,
    pub total_count: usize,
    pub next_page: Option<usize>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RepositoryList {
    pub repositories: Vec<RepoId>,
    pub possibly_limited: bool,
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
mod test_support;

#[cfg(test)]
mod tests;
