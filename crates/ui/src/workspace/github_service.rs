use async_trait::async_trait;
use harbor_domain::{
    CheckRun, DiffFile, MergeMethod, PullRequest, PullRequestComment, PullRequestReview,
    ReactionContent, RepoId, ReviewCommentRange, ReviewThread, WorkflowJob, WorkflowRun,
};
use harbor_github::{
    ConditionalFetch, GitHubClient, GitHubRateLimitStatus, GitHubTransportSource,
    HttpCacheValidator, PullRequestEnrichment, PullRequestListFilter, PullRequestPage,
    PullRequestPageCursor, RepositoryList, Result, SubmitPullRequestReviewEvent,
};
use harbor_sync::PullRequestInboxSource;
use std::sync::{Arc, Mutex};

use super::GitHubAuthSource;

#[path = "github_service_auth.rs"]
mod auth;
#[path = "github_service_traits.rs"]
mod traits;
pub(crate) use traits::{
    GitHubApi, GitHubAuthApi, GitHubPullRequestActionApi, GitHubPullRequestDetailApi,
    GitHubRateLimitApi, GitHubRepositoryApi, GitHubReviewApi, GitHubReviewMutationApi,
    GitHubWorkflowActionApi, GitHubWorkflowApi,
};

#[derive(Clone, Debug)]
pub(crate) struct RealGitHubApi {
    client: Arc<Mutex<Option<GitHubClient<GitHubTransportSource>>>>,
    current_user_login: Arc<Mutex<Option<String>>>,
}

impl Default for RealGitHubApi {
    fn default() -> Self {
        Self {
            client: Arc::new(Mutex::new(None)),
            current_user_login: Arc::new(Mutex::new(None)),
        }
    }
}

impl RealGitHubApi {
    fn has_configured_client(&self) -> bool {
        self.client
            .lock()
            .map(|client| client.is_some())
            .unwrap_or(false)
    }
}

impl GitHubRateLimitApi for RealGitHubApi {
    fn latest_rate_limit(&self) -> Option<GitHubRateLimitStatus> {
        self.client
            .lock()
            .ok()
            .and_then(|client| client.as_ref().and_then(GitHubClient::latest_rate_limit))
    }
}

#[async_trait]
impl PullRequestInboxSource for RealGitHubApi {
    fn latest_rate_limits(&self) -> Vec<GitHubRateLimitStatus> {
        self.client
            .lock()
            .ok()
            .and_then(|client| client.as_ref().map(GitHubClient::latest_rate_limits))
            .unwrap_or_default()
    }

    async fn list_repository_pull_request_page(
        &self,
        repository: &RepoId,
        filter: PullRequestListFilter,
        cursor: Option<PullRequestPageCursor>,
        page_size: usize,
    ) -> Result<PullRequestPage> {
        self.client()?
            .list_repository_pull_request_page(repository, filter, cursor, page_size)
            .await
    }

    async fn count_repository_pull_requests(
        &self,
        repository: &RepoId,
        filter: PullRequestListFilter,
    ) -> Result<usize> {
        self.client()?
            .count_repository_pull_requests(repository, filter)
            .await
    }

    async fn list_repository_pull_requests_light_page(
        &self,
        repository: &RepoId,
        filter: PullRequestListFilter,
        cursor: Option<PullRequestPageCursor>,
        page_size: usize,
        validator: Option<HttpCacheValidator>,
    ) -> Result<ConditionalFetch<PullRequestPage>> {
        self.client()?
            .list_repository_pull_requests_light_page(
                repository,
                filter,
                cursor,
                page_size,
                validator.as_ref(),
            )
            .await
    }

    async fn enrich_pull_requests_by_node_ids(
        &self,
        node_ids: &[String],
    ) -> Result<Vec<PullRequestEnrichment>> {
        self.client()?
            .enrich_pull_requests_by_node_ids(node_ids)
            .await
    }
}

#[async_trait]
impl GitHubRepositoryApi for RealGitHubApi {
    async fn list_repositories(&self) -> Result<RepositoryList> {
        self.client()?.list_repositories().await
    }

    async fn get_repository(&self, repository: &RepoId) -> Result<RepoId> {
        self.client()?.get_repository(repository).await
    }
}

#[async_trait]
impl GitHubPullRequestDetailApi for RealGitHubApi {
    async fn get_pull_request(&self, owner: &str, repo: &str, number: u64) -> Result<PullRequest> {
        self.client()?.get_pull_request(owner, repo, number).await
    }

    async fn list_pull_request_files(
        &self,
        owner: &str,
        repo: &str,
        number: u64,
    ) -> Result<Vec<DiffFile>> {
        self.client()?
            .list_pull_request_files(owner, repo, number)
            .await
    }

    async fn list_check_runs(
        &self,
        owner: &str,
        repo: &str,
        head_sha: &str,
    ) -> Result<Vec<CheckRun>> {
        self.client()?.list_check_runs(owner, repo, head_sha).await
    }

    async fn list_workflow_runs_for_head(
        &self,
        owner: &str,
        repo: &str,
        head_sha: &str,
    ) -> Result<Vec<WorkflowRun>> {
        self.client()?
            .list_workflow_runs_for_head(owner, repo, head_sha)
            .await
    }
}

#[async_trait]
impl GitHubWorkflowApi for RealGitHubApi {
    async fn list_workflow_jobs_for_run(
        &self,
        owner: &str,
        repo: &str,
        run_id: u64,
    ) -> Result<Vec<WorkflowJob>> {
        self.client()?
            .list_workflow_jobs_for_run(owner, repo, run_id)
            .await
    }

    async fn workflow_run_log(&self, owner: &str, repo: &str, run_id: u64) -> Result<String> {
        self.client()?.workflow_run_log(owner, repo, run_id).await
    }
}

#[async_trait]
impl GitHubWorkflowActionApi for RealGitHubApi {
    async fn dispatch_workflow(
        &self,
        owner: &str,
        repo: &str,
        workflow_id: u64,
        git_ref: &str,
    ) -> Result<()> {
        self.client()?
            .dispatch_workflow(owner, repo, workflow_id, git_ref)
            .await
    }

    async fn rerun_failed_jobs(&self, owner: &str, repo: &str, run_id: u64) -> Result<()> {
        self.client()?.rerun_failed_jobs(owner, repo, run_id).await
    }
}

#[async_trait]
impl GitHubReviewApi for RealGitHubApi {
    async fn current_user(&self) -> Result<String> {
        if let Some(login) = self.cached_current_user_login()? {
            return Ok(login);
        }

        let login = self.client()?.current_user().await?;
        self.cache_current_user_login(login.clone())?;

        Ok(login)
    }

    async fn list_pull_request_reviews(
        &self,
        owner: &str,
        repo: &str,
        number: u64,
    ) -> Result<Vec<PullRequestReview>> {
        self.client()?
            .list_pull_request_reviews(owner, repo, number)
            .await
    }

    async fn list_pull_request_comments(
        &self,
        owner: &str,
        repo: &str,
        number: u64,
    ) -> Result<Vec<PullRequestComment>> {
        self.client()?
            .list_pull_request_comments(owner, repo, number)
            .await
    }

    async fn pull_request_review_comment_count(
        &self,
        owner: &str,
        repo: &str,
        number: u64,
        review_id: &str,
    ) -> Result<usize> {
        self.client()?
            .pull_request_review_comment_count(owner, repo, number, review_id)
            .await
    }

    async fn list_review_threads(
        &self,
        owner: &str,
        repo: &str,
        number: u64,
    ) -> Result<Vec<ReviewThread>> {
        self.client()?
            .list_review_threads(owner, repo, number)
            .await
    }
}

#[async_trait]
impl GitHubReviewMutationApi for RealGitHubApi {
    async fn submit_pull_request_review(
        &self,
        pull_request_review_node_id: &str,
        event: SubmitPullRequestReviewEvent,
        body: Option<&str>,
    ) -> Result<()> {
        self.client()?
            .submit_pull_request_review(pull_request_review_node_id, event, body)
            .await
    }

    async fn create_pull_request_review_comment(
        &self,
        owner: &str,
        repo: &str,
        number: u64,
        commit_id: &str,
        range: &ReviewCommentRange,
        body: &str,
    ) -> Result<()> {
        self.client()?
            .create_pull_request_review_comment(owner, repo, number, commit_id, range, body)
            .await
    }

    async fn start_pull_request_review(
        &self,
        pull_request_node_id: &str,
        commit_id: &str,
        range: &ReviewCommentRange,
        body: &str,
    ) -> Result<String> {
        self.client()?
            .start_pull_request_review(pull_request_node_id, commit_id, range, body)
            .await
    }

    async fn add_pending_review_thread(
        &self,
        pull_request_review_node_id: &str,
        range: &ReviewCommentRange,
        body: &str,
    ) -> Result<()> {
        self.client()?
            .add_pending_review_thread(pull_request_review_node_id, range, body)
            .await
    }

    async fn add_review_thread_reply(
        &self,
        thread_id: &str,
        pull_request_review_node_id: Option<&str>,
        body: &str,
    ) -> Result<()> {
        self.client()?
            .add_review_thread_reply(thread_id, pull_request_review_node_id, body)
            .await
    }

    async fn resolve_review_thread(&self, thread_id: &str) -> Result<()> {
        self.client()?.resolve_review_thread(thread_id).await
    }

    async fn unresolve_review_thread(&self, thread_id: &str) -> Result<()> {
        self.client()?.unresolve_review_thread(thread_id).await
    }

    async fn update_review_comment(&self, comment_id: &str, body: &str) -> Result<()> {
        self.client()?.update_review_comment(comment_id, body).await
    }

    async fn delete_review_comment(&self, comment_id: &str) -> Result<()> {
        self.client()?.delete_review_comment(comment_id).await
    }

    async fn add_review_comment_reaction(
        &self,
        comment_id: &str,
        content: ReactionContent,
    ) -> Result<()> {
        self.client()?
            .add_review_comment_reaction(comment_id, content)
            .await
    }

    async fn remove_review_comment_reaction(
        &self,
        comment_id: &str,
        content: ReactionContent,
    ) -> Result<()> {
        self.client()?
            .remove_review_comment_reaction(comment_id, content)
            .await
    }
}

#[async_trait]
impl GitHubPullRequestActionApi for RealGitHubApi {
    async fn approve_pull_request(
        &self,
        owner: &str,
        repo: &str,
        number: u64,
        body: Option<&str>,
    ) -> Result<()> {
        self.client()?
            .approve_pull_request(owner, repo, number, body)
            .await
    }

    async fn request_pull_request_changes(
        &self,
        owner: &str,
        repo: &str,
        number: u64,
        body: &str,
    ) -> Result<()> {
        self.client()?
            .request_pull_request_changes(owner, repo, number, body)
            .await
    }

    async fn merge_pull_request(
        &self,
        owner: &str,
        repo: &str,
        number: u64,
        head_sha: &str,
        method: MergeMethod,
    ) -> Result<()> {
        self.client()?
            .merge_pull_request(owner, repo, number, head_sha, method)
            .await
    }
}

#[cfg(test)]
#[path = "github_service_test_support.rs"]
pub(crate) mod test_support;
#[cfg(test)]
#[path = "github_service_tests.rs"]
mod tests;
