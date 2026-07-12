use async_trait::async_trait;
use harbor_domain::{MergeMethod, PullRequestCommit};
use harbor_github::Result;

#[async_trait]
pub trait GitHubPullRequestApi: Send + Sync {
    async fn list_pull_request_commits(
        &self,
        owner: &str,
        repo: &str,
        number: u64,
    ) -> Result<Vec<PullRequestCommit>>;
    async fn mark_pull_request_file_viewed(
        &self,
        pull_request_node_id: &str,
        path: &str,
    ) -> Result<()>;
    async fn unmark_pull_request_file_viewed(
        &self,
        pull_request_node_id: &str,
        path: &str,
    ) -> Result<()>;
}

#[async_trait]
pub trait GitHubPullRequestMutationApi: Send + Sync {
    async fn update_pull_request_body(&self, pull_request_node_id: &str, body: &str) -> Result<()>;
    async fn request_pull_request_reviewer(
        &self,
        owner: &str,
        repo: &str,
        number: u64,
        reviewer: &str,
    ) -> Result<()>;
    async fn add_pull_request_assignee(
        &self,
        owner: &str,
        repo: &str,
        number: u64,
        assignee: &str,
    ) -> Result<()>;
    async fn add_pull_request_label(
        &self,
        owner: &str,
        repo: &str,
        number: u64,
        label: &str,
    ) -> Result<()>;
    async fn create_pull_request_comment(
        &self,
        owner: &str,
        repo: &str,
        number: u64,
        body: &str,
    ) -> Result<()>;
    async fn approve_pull_request(
        &self,
        owner: &str,
        repo: &str,
        number: u64,
        body: Option<&str>,
    ) -> Result<()>;
    async fn request_pull_request_changes(
        &self,
        owner: &str,
        repo: &str,
        number: u64,
        body: &str,
    ) -> Result<()>;
    async fn merge_pull_request(
        &self,
        owner: &str,
        repo: &str,
        number: u64,
        head_sha: &str,
        method: MergeMethod,
    ) -> Result<()>;
}
