use async_trait::async_trait;
use harbor_domain::{
    CheckRun, DiffFile, PullRequest, PullRequestReview, ReactionContent, RepoId,
    ReviewCommentRange, ReviewThread, WorkflowJob, WorkflowRun,
};
use harbor_github::{
    GhCliTransport, GitHubClient, PullRequestListFilter, Result, SubmitPullRequestReviewEvent,
};

#[async_trait]
pub(crate) trait GitHubApi: Send + Sync {
    async fn list_repositories(&self) -> Result<Vec<RepoId>>;

    async fn list_repository_pull_requests(
        &self,
        repository: &RepoId,
        filter: PullRequestListFilter,
    ) -> Result<Vec<PullRequest>>;

    async fn get_pull_request(&self, owner: &str, repo: &str, number: u64) -> Result<PullRequest>;

    async fn list_pull_request_files(
        &self,
        owner: &str,
        repo: &str,
        number: u64,
    ) -> Result<Vec<DiffFile>>;

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

    async fn list_workflow_jobs_for_run(
        &self,
        owner: &str,
        repo: &str,
        run_id: u64,
    ) -> Result<Vec<WorkflowJob>>;

    async fn workflow_run_log(&self, owner: &str, repo: &str, run_id: u64) -> Result<String>;

    async fn current_user(&self) -> Result<String>;

    async fn list_pull_request_reviews(
        &self,
        owner: &str,
        repo: &str,
        number: u64,
    ) -> Result<Vec<PullRequestReview>>;

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

    async fn dispatch_workflow(
        &self,
        owner: &str,
        repo: &str,
        workflow_id: u64,
        git_ref: &str,
    ) -> Result<()>;

    async fn rerun_failed_jobs(&self, owner: &str, repo: &str, run_id: u64) -> Result<()>;

    async fn approve_pull_request(&self, owner: &str, repo: &str, number: u64) -> Result<()>;

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
    ) -> Result<()>;

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

#[derive(Clone, Debug)]
pub(crate) struct RealGitHubApi {
    client: GitHubClient<GhCliTransport>,
}

impl Default for RealGitHubApi {
    fn default() -> Self {
        let transport = GhCliTransport;
        Self {
            client: GitHubClient::new(transport),
        }
    }
}

#[async_trait]
impl GitHubApi for RealGitHubApi {
    async fn list_repositories(&self) -> Result<Vec<RepoId>> {
        self.client.list_repositories().await
    }

    async fn list_repository_pull_requests(
        &self,
        repository: &RepoId,
        filter: PullRequestListFilter,
    ) -> Result<Vec<PullRequest>> {
        self.client
            .list_repository_pull_requests(repository, filter)
            .await
    }

    async fn get_pull_request(&self, owner: &str, repo: &str, number: u64) -> Result<PullRequest> {
        self.client.get_pull_request(owner, repo, number).await
    }

    async fn list_pull_request_files(
        &self,
        owner: &str,
        repo: &str,
        number: u64,
    ) -> Result<Vec<DiffFile>> {
        self.client
            .list_pull_request_files(owner, repo, number)
            .await
    }

    async fn list_check_runs(
        &self,
        owner: &str,
        repo: &str,
        head_sha: &str,
    ) -> Result<Vec<CheckRun>> {
        self.client.list_check_runs(owner, repo, head_sha).await
    }

    async fn list_workflow_runs_for_head(
        &self,
        owner: &str,
        repo: &str,
        head_sha: &str,
    ) -> Result<Vec<WorkflowRun>> {
        self.client
            .list_workflow_runs_for_head(owner, repo, head_sha)
            .await
    }

    async fn list_workflow_jobs_for_run(
        &self,
        owner: &str,
        repo: &str,
        run_id: u64,
    ) -> Result<Vec<WorkflowJob>> {
        self.client
            .list_workflow_jobs_for_run(owner, repo, run_id)
            .await
    }

    async fn workflow_run_log(&self, owner: &str, repo: &str, run_id: u64) -> Result<String> {
        self.client.workflow_run_log(owner, repo, run_id).await
    }

    async fn current_user(&self) -> Result<String> {
        self.client.current_user().await
    }

    async fn list_pull_request_reviews(
        &self,
        owner: &str,
        repo: &str,
        number: u64,
    ) -> Result<Vec<PullRequestReview>> {
        self.client
            .list_pull_request_reviews(owner, repo, number)
            .await
    }

    async fn pull_request_review_comment_count(
        &self,
        owner: &str,
        repo: &str,
        number: u64,
        review_id: &str,
    ) -> Result<usize> {
        self.client
            .pull_request_review_comment_count(owner, repo, number, review_id)
            .await
    }

    async fn list_review_threads(
        &self,
        owner: &str,
        repo: &str,
        number: u64,
    ) -> Result<Vec<ReviewThread>> {
        self.client.list_review_threads(owner, repo, number).await
    }

    async fn dispatch_workflow(
        &self,
        owner: &str,
        repo: &str,
        workflow_id: u64,
        git_ref: &str,
    ) -> Result<()> {
        self.client
            .dispatch_workflow(owner, repo, workflow_id, git_ref)
            .await
    }

    async fn rerun_failed_jobs(&self, owner: &str, repo: &str, run_id: u64) -> Result<()> {
        self.client.rerun_failed_jobs(owner, repo, run_id).await
    }

    async fn approve_pull_request(&self, owner: &str, repo: &str, number: u64) -> Result<()> {
        self.client.approve_pull_request(owner, repo, number).await
    }

    async fn request_pull_request_changes(
        &self,
        owner: &str,
        repo: &str,
        number: u64,
        body: &str,
    ) -> Result<()> {
        self.client
            .request_pull_request_changes(owner, repo, number, body)
            .await
    }

    async fn merge_pull_request(
        &self,
        owner: &str,
        repo: &str,
        number: u64,
        head_sha: &str,
    ) -> Result<()> {
        self.client
            .merge_pull_request(owner, repo, number, head_sha)
            .await
    }

    async fn submit_pull_request_review(
        &self,
        pull_request_review_node_id: &str,
        event: SubmitPullRequestReviewEvent,
        body: Option<&str>,
    ) -> Result<()> {
        self.client
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
        self.client
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
        self.client
            .start_pull_request_review(pull_request_node_id, commit_id, range, body)
            .await
    }

    async fn add_pending_review_thread(
        &self,
        pull_request_review_node_id: &str,
        range: &ReviewCommentRange,
        body: &str,
    ) -> Result<()> {
        self.client
            .add_pending_review_thread(pull_request_review_node_id, range, body)
            .await
    }

    async fn add_review_thread_reply(
        &self,
        thread_id: &str,
        pull_request_review_node_id: Option<&str>,
        body: &str,
    ) -> Result<()> {
        self.client
            .add_review_thread_reply(thread_id, pull_request_review_node_id, body)
            .await
    }

    async fn resolve_review_thread(&self, thread_id: &str) -> Result<()> {
        self.client.resolve_review_thread(thread_id).await
    }

    async fn unresolve_review_thread(&self, thread_id: &str) -> Result<()> {
        self.client.unresolve_review_thread(thread_id).await
    }

    async fn update_review_comment(&self, comment_id: &str, body: &str) -> Result<()> {
        self.client.update_review_comment(comment_id, body).await
    }

    async fn delete_review_comment(&self, comment_id: &str) -> Result<()> {
        self.client.delete_review_comment(comment_id).await
    }

    async fn add_review_comment_reaction(
        &self,
        comment_id: &str,
        content: ReactionContent,
    ) -> Result<()> {
        self.client
            .add_review_comment_reaction(comment_id, content)
            .await
    }

    async fn remove_review_comment_reaction(
        &self,
        comment_id: &str,
        content: ReactionContent,
    ) -> Result<()> {
        self.client
            .remove_review_comment_reaction(comment_id, content)
            .await
    }
}
