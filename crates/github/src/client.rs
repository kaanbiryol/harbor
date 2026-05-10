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

impl<T> GitHubClient<T> {
    pub fn new(transport: T) -> Self {
        Self { transport }
    }

    pub fn transport(&self) -> &T {
        &self.transport
    }
}

#[cfg(test)]
#[path = "client/test_support.rs"]
mod test_support;

#[cfg(test)]
#[path = "client/tests.rs"]
mod tests;
