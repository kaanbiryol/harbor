use async_trait::async_trait;
use harbor_domain::{
    CheckRun, DiffFile, MergeMethod, PullRequest, PullRequestComment, PullRequestReview,
    ReactionContent, RepoId, ReviewCommentRange, ReviewThread, WorkflowJob, WorkflowRun,
};
use harbor_github::{GitHubRateLimitStatus, RepositoryList, Result, SubmitPullRequestReviewEvent};
use harbor_sync::PullRequestInboxSource;

use crate::workspace::GitHubAuthSource;

pub(crate) trait GitHubApi:
    GitHubAuthApi
    + GitHubRateLimitApi
    + GitHubRepositoryApi
    + GitHubPullRequestDetailApi
    + GitHubWorkflowApi
    + GitHubWorkflowActionApi
    + GitHubReviewApi
    + GitHubReviewMutationApi
    + GitHubPullRequestActionApi
    + PullRequestInboxSource
{
}

impl<T> GitHubApi for T where
    T: GitHubAuthApi
        + GitHubRateLimitApi
        + GitHubRepositoryApi
        + GitHubPullRequestDetailApi
        + GitHubWorkflowApi
        + GitHubWorkflowActionApi
        + GitHubReviewApi
        + GitHubReviewMutationApi
        + GitHubPullRequestActionApi
        + PullRequestInboxSource
{
}

pub(crate) trait GitHubAuthApi: Send + Sync {
    fn configure_token(&self, token: String, source: GitHubAuthSource) -> Result<()>;
    fn configure_gh_cli(&self) -> Result<()>;
    fn clear_auth(&self) -> Result<()>;
    fn has_auth(&self) -> bool;
}

pub(crate) trait GitHubRateLimitApi: Send + Sync {
    fn latest_rate_limit(&self) -> Option<GitHubRateLimitStatus>;
}

#[async_trait]
pub(crate) trait GitHubRepositoryApi: Send + Sync {
    async fn list_repositories(&self) -> Result<RepositoryList>;

    async fn get_repository(&self, repository: &RepoId) -> Result<RepoId>;
}

#[async_trait]
pub(crate) trait GitHubPullRequestDetailApi: Send + Sync {
    async fn get_pull_request(&self, owner: &str, repo: &str, number: u64) -> Result<PullRequest>;

    async fn list_pull_request_files(
        &self,
        owner: &str,
        repo: &str,
        number: u64,
    ) -> Result<Vec<DiffFile>>;

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

    async fn list_check_runs(
        &self,
        owner: &str,
        repo: &str,
        head_sha: &str,
    ) -> Result<Vec<CheckRun>>;

    async fn list_workflow_runs_for_head(
        &self,
        owner: &str,
        repo: &str,
        head_sha: &str,
    ) -> Result<Vec<WorkflowRun>>;
}

#[async_trait]
pub(crate) trait GitHubWorkflowApi: Send + Sync {
    async fn list_workflow_jobs_for_run(
        &self,
        owner: &str,
        repo: &str,
        run_id: u64,
    ) -> Result<Vec<WorkflowJob>>;

    async fn workflow_run_log(&self, owner: &str, repo: &str, run_id: u64) -> Result<String>;
}

#[async_trait]
pub(crate) trait GitHubWorkflowActionApi: Send + Sync {
    async fn dispatch_workflow(
        &self,
        owner: &str,
        repo: &str,
        workflow_id: u64,
        git_ref: &str,
    ) -> Result<()>;

    async fn rerun_failed_jobs(&self, owner: &str, repo: &str, run_id: u64) -> Result<()>;
}

#[async_trait]
pub(crate) trait GitHubReviewApi: Send + Sync {
    async fn current_user(&self) -> Result<String>;

    async fn list_pull_request_reviews(
        &self,
        owner: &str,
        repo: &str,
        number: u64,
    ) -> Result<Vec<PullRequestReview>>;

    async fn list_pull_request_comments(
        &self,
        owner: &str,
        repo: &str,
        number: u64,
    ) -> Result<Vec<PullRequestComment>>;

    async fn pull_request_review_comment_count(
        &self,
        owner: &str,
        repo: &str,
        number: u64,
        review_id: &str,
    ) -> Result<usize>;

    async fn list_review_threads(
        &self,
        owner: &str,
        repo: &str,
        number: u64,
    ) -> Result<Vec<ReviewThread>>;
}

#[async_trait]
pub(crate) trait GitHubReviewMutationApi: Send + Sync {
    async fn submit_pull_request_review(
        &self,
        pull_request_review_node_id: &str,
        event: SubmitPullRequestReviewEvent,
        body: Option<&str>,
    ) -> Result<()>;

    async fn create_pull_request_review_comment(
        &self,
        owner: &str,
        repo: &str,
        number: u64,
        commit_id: &str,
        range: &ReviewCommentRange,
        body: &str,
    ) -> Result<()>;

    async fn start_pull_request_review(
        &self,
        pull_request_node_id: &str,
        commit_id: &str,
        range: &ReviewCommentRange,
        body: &str,
    ) -> Result<String>;

    async fn add_pending_review_thread(
        &self,
        pull_request_review_node_id: &str,
        range: &ReviewCommentRange,
        body: &str,
    ) -> Result<()>;

    async fn add_review_thread_reply(
        &self,
        thread_id: &str,
        pull_request_review_node_id: Option<&str>,
        body: &str,
    ) -> Result<()>;

    async fn resolve_review_thread(&self, thread_id: &str) -> Result<()>;

    async fn unresolve_review_thread(&self, thread_id: &str) -> Result<()>;

    async fn update_review_comment(&self, comment_id: &str, body: &str) -> Result<()>;

    async fn delete_review_comment(&self, comment_id: &str) -> Result<()>;

    async fn add_review_comment_reaction(
        &self,
        comment_id: &str,
        content: ReactionContent,
    ) -> Result<()>;

    async fn remove_review_comment_reaction(
        &self,
        comment_id: &str,
        content: ReactionContent,
    ) -> Result<()>;
}

#[async_trait]
pub(crate) trait GitHubPullRequestActionApi: Send + Sync {
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
