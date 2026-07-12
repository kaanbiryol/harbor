#[path = "github_service_traits/auth.rs"]
mod auth;
#[path = "github_service_traits/pull_requests.rs"]
mod pull_requests;
#[path = "github_service_traits/repositories.rs"]
mod repositories;
#[path = "github_service_traits/reviews.rs"]
mod reviews;
#[path = "github_service_traits/workflows.rs"]
mod workflows;

use harbor_sync::{PullRequestCiSource, PullRequestContentSource, PullRequestInboxSource};

pub use auth::GitHubAuthApi;
pub use pull_requests::{GitHubPullRequestApi, GitHubPullRequestMutationApi};
pub use repositories::GitHubRepositoryApi;
pub use reviews::{GitHubReviewApi, GitHubReviewMutationApi};
pub use workflows::{GitHubWorkflowApi, GitHubWorkflowMutationApi};

pub trait GitHubApi:
    GitHubAuthApi
    + GitHubPullRequestApi
    + GitHubPullRequestMutationApi
    + GitHubRepositoryApi
    + GitHubReviewApi
    + GitHubReviewMutationApi
    + GitHubWorkflowApi
    + GitHubWorkflowMutationApi
    + PullRequestCiSource
    + PullRequestContentSource
    + PullRequestInboxSource
    + Send
    + Sync
{
}

impl<T> GitHubApi for T where
    T: GitHubAuthApi
        + GitHubPullRequestApi
        + GitHubPullRequestMutationApi
        + GitHubRepositoryApi
        + GitHubReviewApi
        + GitHubReviewMutationApi
        + GitHubWorkflowApi
        + GitHubWorkflowMutationApi
        + PullRequestCiSource
        + PullRequestContentSource
        + PullRequestInboxSource
        + Send
        + Sync
{
}
