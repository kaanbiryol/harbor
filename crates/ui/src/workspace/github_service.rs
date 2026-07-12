#[path = "github_service_traits.rs"]
mod traits;
pub use traits::{
    GitHubApi, GitHubAuthApi, GitHubPullRequestApi, GitHubPullRequestMutationApi,
    GitHubRepositoryApi, GitHubReviewApi, GitHubReviewMutationApi, GitHubWorkflowApi,
    GitHubWorkflowMutationApi,
};

#[cfg(test)]
#[path = "github_service_test_support.rs"]
pub(crate) mod test_support;
#[cfg(test)]
#[path = "github_service_tests.rs"]
mod tests;
