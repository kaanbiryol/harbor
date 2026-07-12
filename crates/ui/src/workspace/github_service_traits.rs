pub use harbor_github::{
    GitHubAuthApi, GitHubPullRequestApi, GitHubPullRequestMutationApi, GitHubRepositoryApi,
    GitHubReviewApi, GitHubReviewMutationApi, GitHubWorkflowApi, GitHubWorkflowMutationApi,
};
use harbor_sync::{PullRequestCiSource, PullRequestContentSource, PullRequestInboxSource};

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
