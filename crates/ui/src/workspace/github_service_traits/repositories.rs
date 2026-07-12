use async_trait::async_trait;
use harbor_domain::{PullRequestMetadataOptions, RepoId};
use harbor_github::{RepositoryList, Result};

#[async_trait]
pub trait GitHubRepositoryApi: Send + Sync {
    async fn list_repositories(&self) -> Result<RepositoryList>;
    async fn get_repository(&self, repository: &RepoId) -> Result<RepoId>;
    async fn list_pull_request_metadata_options(
        &self,
        owner: &str,
        repo: &str,
    ) -> Result<PullRequestMetadataOptions>;
    async fn current_user(&self) -> Result<String>;
}
