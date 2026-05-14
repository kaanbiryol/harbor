use async_trait::async_trait;
use harbor_domain::{
    CheckRun, DiffFile, PullRequest, PullRequestReview, ReactionContent, RepoId,
    ReviewCommentRange, ReviewThread, WorkflowJob, WorkflowRun,
};
use harbor_github::{
    ConditionalFetch, GhCliTransport, GitHubClient, GitHubError, GitHubRateLimitStatus,
    HttpCacheValidator, PullRequestEnrichment, PullRequestListFilter, RepositoryList, Result,
    SubmitPullRequestReviewEvent,
};
use harbor_sync::PullRequestInboxSource;
use std::sync::Mutex;

pub(crate) trait GitHubApi:
    GitHubRateLimitApi
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
    T: GitHubRateLimitApi
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
}

#[derive(Clone, Debug)]
pub(crate) struct RealGitHubApi {
    client: GitHubClient<GhCliTransport>,
    current_user_login: std::sync::Arc<Mutex<Option<String>>>,
}

impl Default for RealGitHubApi {
    fn default() -> Self {
        Self {
            client: GitHubClient::new(GhCliTransport::default()),
            current_user_login: std::sync::Arc::new(Mutex::new(None)),
        }
    }
}

impl RealGitHubApi {
    fn cached_current_user_login(&self) -> Result<Option<String>> {
        self.current_user_login
            .lock()
            .map(|login| login.clone())
            .map_err(|error| GitHubError::Transport(error.to_string()))
    }

    fn cache_current_user_login(&self, login: String) -> Result<()> {
        self.current_user_login
            .lock()
            .map(|mut cached_login| {
                *cached_login = Some(login);
            })
            .map_err(|error| GitHubError::Transport(error.to_string()))
    }
}

impl GitHubRateLimitApi for RealGitHubApi {
    fn latest_rate_limit(&self) -> Option<GitHubRateLimitStatus> {
        self.client.latest_rate_limit()
    }
}

#[async_trait]
impl PullRequestInboxSource for RealGitHubApi {
    fn latest_rate_limits(&self) -> Vec<GitHubRateLimitStatus> {
        self.client.latest_rate_limits()
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

    async fn count_repository_pull_requests(
        &self,
        repository: &RepoId,
        filter: PullRequestListFilter,
    ) -> Result<usize> {
        self.client
            .count_repository_pull_requests(repository, filter)
            .await
    }

    async fn list_repository_pull_requests_light(
        &self,
        repository: &RepoId,
        filter: PullRequestListFilter,
        validator: Option<HttpCacheValidator>,
    ) -> Result<ConditionalFetch<Vec<PullRequest>>> {
        self.client
            .list_repository_pull_requests_light(repository, filter, validator.as_ref())
            .await
    }

    async fn enrich_pull_requests_by_node_ids(
        &self,
        node_ids: &[String],
    ) -> Result<Vec<PullRequestEnrichment>> {
        self.client.enrich_pull_requests_by_node_ids(node_ids).await
    }
}

#[async_trait]
impl GitHubRepositoryApi for RealGitHubApi {
    async fn list_repositories(&self) -> Result<RepositoryList> {
        self.client.list_repositories().await
    }

    async fn get_repository(&self, repository: &RepoId) -> Result<RepoId> {
        self.client.get_repository(repository).await
    }
}

#[async_trait]
impl GitHubPullRequestDetailApi for RealGitHubApi {
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
}

#[async_trait]
impl GitHubWorkflowApi for RealGitHubApi {
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
        self.client
            .dispatch_workflow(owner, repo, workflow_id, git_ref)
            .await
    }

    async fn rerun_failed_jobs(&self, owner: &str, repo: &str, run_id: u64) -> Result<()> {
        self.client.rerun_failed_jobs(owner, repo, run_id).await
    }
}

#[async_trait]
impl GitHubReviewApi for RealGitHubApi {
    async fn current_user(&self) -> Result<String> {
        if let Some(login) = self.cached_current_user_login()? {
            return Ok(login);
        }

        let login = self.client.current_user().await?;
        self.cache_current_user_login(login.clone())?;

        Ok(login)
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
}

#[async_trait]
impl GitHubReviewMutationApi for RealGitHubApi {
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

#[async_trait]
impl GitHubPullRequestActionApi for RealGitHubApi {
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
}

#[cfg(test)]
pub(crate) mod test_support {
    use std::{
        collections::VecDeque,
        sync::{Arc, Mutex},
    };

    use async_trait::async_trait;
    use harbor_domain::{
        CheckRun, DiffFile, PullRequest, PullRequestReview, ReactionContent, RepoId,
        ReviewCommentRange, ReviewThread, WorkflowJob, WorkflowRun,
    };
    use harbor_github::{
        ConditionalFetch, GitHubError, GitHubRateLimitStatus, HttpCacheValidator,
        PullRequestEnrichment, PullRequestListFilter, RepositoryList, Result,
        SubmitPullRequestReviewEvent,
    };

    use harbor_sync::PullRequestInboxSource;

    use super::{
        GitHubPullRequestActionApi, GitHubPullRequestDetailApi, GitHubRateLimitApi,
        GitHubRepositoryApi, GitHubReviewApi, GitHubReviewMutationApi, GitHubWorkflowActionApi,
        GitHubWorkflowApi,
    };

    type FakeQueue<T> = Arc<Mutex<VecDeque<Result<T>>>>;

    #[derive(Clone, Default)]
    pub(crate) struct FakeGitHubApi {
        calls: Arc<Mutex<Vec<String>>>,
        repositories: FakeQueue<RepositoryList>,
        repository_lookups: FakeQueue<RepoId>,
        pull_requests: FakeQueue<Vec<PullRequest>>,
        pull_request_counts: FakeQueue<usize>,
        light_pull_requests: FakeQueue<ConditionalFetch<Vec<PullRequest>>>,
        pull_request_enrichments: FakeQueue<Vec<PullRequestEnrichment>>,
        pull_request_details: FakeQueue<PullRequest>,
        files: FakeQueue<Vec<DiffFile>>,
        check_runs: FakeQueue<Vec<CheckRun>>,
        workflow_runs: FakeQueue<Vec<WorkflowRun>>,
        workflow_jobs: FakeQueue<Vec<WorkflowJob>>,
        workflow_logs: FakeQueue<String>,
        current_user: FakeQueue<String>,
        reviews: FakeQueue<Vec<PullRequestReview>>,
        review_comment_counts: FakeQueue<usize>,
        review_threads: FakeQueue<Vec<ReviewThread>>,
        dispatch_workflow_results: FakeQueue<()>,
        rerun_failed_jobs_results: FakeQueue<()>,
        approve_results: FakeQueue<()>,
        request_changes_results: FakeQueue<()>,
        merge_results: FakeQueue<()>,
        submit_review_results: FakeQueue<()>,
        create_comment_results: FakeQueue<()>,
        start_review_results: FakeQueue<String>,
        pending_thread_results: FakeQueue<()>,
        reply_results: FakeQueue<()>,
        resolve_thread_results: FakeQueue<()>,
        unresolve_thread_results: FakeQueue<()>,
        update_comment_results: FakeQueue<()>,
        delete_comment_results: FakeQueue<()>,
        add_reaction_results: FakeQueue<()>,
        remove_reaction_results: FakeQueue<()>,
    }

    impl FakeGitHubApi {
        pub(crate) fn push_repository_lookup(&self, result: Result<RepoId>) {
            push_result(&self.repository_lookups, result);
        }

        pub(crate) fn push_light_pull_requests(
            &self,
            result: Result<ConditionalFetch<Vec<PullRequest>>>,
        ) {
            push_result(&self.light_pull_requests, result);
        }

        pub(crate) fn push_pull_request_count(&self, result: Result<usize>) {
            push_result(&self.pull_request_counts, result);
        }

        pub(crate) fn push_pull_request_enrichments(
            &self,
            result: Result<Vec<PullRequestEnrichment>>,
        ) {
            push_result(&self.pull_request_enrichments, result);
        }

        pub(crate) fn push_pull_request_detail(&self, result: Result<PullRequest>) {
            push_result(&self.pull_request_details, result);
        }

        pub(crate) fn push_files(&self, result: Result<Vec<DiffFile>>) {
            push_result(&self.files, result);
        }

        pub(crate) fn push_check_runs(&self, result: Result<Vec<CheckRun>>) {
            push_result(&self.check_runs, result);
        }

        pub(crate) fn push_workflow_runs(&self, result: Result<Vec<WorkflowRun>>) {
            push_result(&self.workflow_runs, result);
        }

        pub(crate) fn push_current_user(&self, result: Result<String>) {
            push_result(&self.current_user, result);
        }

        pub(crate) fn push_reviews(&self, result: Result<Vec<PullRequestReview>>) {
            push_result(&self.reviews, result);
        }

        pub(crate) fn push_review_threads(&self, result: Result<Vec<ReviewThread>>) {
            push_result(&self.review_threads, result);
        }

        pub(crate) fn push_dispatch_workflow(&self, result: Result<()>) {
            push_result(&self.dispatch_workflow_results, result);
        }

        pub(crate) fn push_approve_pull_request(&self, result: Result<()>) {
            push_result(&self.approve_results, result);
        }

        pub(crate) fn calls(&self) -> Vec<String> {
            self.calls
                .lock()
                .expect("fake GitHub API calls mutex should not be poisoned")
                .clone()
        }

        fn record_call(&self, name: &str) {
            self.calls
                .lock()
                .expect("fake GitHub API calls mutex should not be poisoned")
                .push(name.to_string());
        }
    }

    fn push_result<T>(queue: &FakeQueue<T>, result: Result<T>) {
        queue
            .lock()
            .expect("fake GitHub API queue mutex should not be poisoned")
            .push_back(result);
    }

    fn pop_result<T>(queue: &FakeQueue<T>, name: &str) -> Result<T> {
        queue
            .lock()
            .expect("fake GitHub API queue mutex should not be poisoned")
            .pop_front()
            .unwrap_or_else(|| {
                Err(GitHubError::Transport(format!(
                    "missing fake {name} result"
                )))
            })
    }

    impl GitHubRateLimitApi for FakeGitHubApi {
        fn latest_rate_limit(&self) -> Option<GitHubRateLimitStatus> {
            None
        }
    }

    #[async_trait]
    impl PullRequestInboxSource for FakeGitHubApi {
        fn latest_rate_limits(&self) -> Vec<GitHubRateLimitStatus> {
            Vec::new()
        }

        async fn list_repository_pull_requests(
            &self,
            _repository: &RepoId,
            _filter: PullRequestListFilter,
        ) -> Result<Vec<PullRequest>> {
            self.record_call("list_repository_pull_requests");
            pop_result(&self.pull_requests, "list_repository_pull_requests")
        }

        async fn count_repository_pull_requests(
            &self,
            _repository: &RepoId,
            _filter: PullRequestListFilter,
        ) -> Result<usize> {
            self.record_call("count_repository_pull_requests");
            pop_result(&self.pull_request_counts, "count_repository_pull_requests")
        }

        async fn list_repository_pull_requests_light(
            &self,
            _repository: &RepoId,
            _filter: PullRequestListFilter,
            _validator: Option<HttpCacheValidator>,
        ) -> Result<ConditionalFetch<Vec<PullRequest>>> {
            self.record_call("list_repository_pull_requests_light");
            pop_result(
                &self.light_pull_requests,
                "list_repository_pull_requests_light",
            )
        }

        async fn enrich_pull_requests_by_node_ids(
            &self,
            _node_ids: &[String],
        ) -> Result<Vec<PullRequestEnrichment>> {
            self.record_call("enrich_pull_requests_by_node_ids");
            pop_result(
                &self.pull_request_enrichments,
                "enrich_pull_requests_by_node_ids",
            )
        }
    }

    #[async_trait]
    impl GitHubRepositoryApi for FakeGitHubApi {
        async fn list_repositories(&self) -> Result<RepositoryList> {
            self.record_call("list_repositories");
            pop_result(&self.repositories, "list_repositories")
        }

        async fn get_repository(&self, _repository: &RepoId) -> Result<RepoId> {
            self.record_call("get_repository");
            pop_result(&self.repository_lookups, "get_repository")
        }
    }

    #[async_trait]
    impl GitHubPullRequestDetailApi for FakeGitHubApi {
        async fn get_pull_request(
            &self,
            _owner: &str,
            _repo: &str,
            _number: u64,
        ) -> Result<PullRequest> {
            self.record_call("get_pull_request");
            pop_result(&self.pull_request_details, "get_pull_request")
        }

        async fn list_pull_request_files(
            &self,
            _owner: &str,
            _repo: &str,
            _number: u64,
        ) -> Result<Vec<DiffFile>> {
            self.record_call("list_pull_request_files");
            pop_result(&self.files, "list_pull_request_files")
        }

        async fn list_check_runs(
            &self,
            _owner: &str,
            _repo: &str,
            _head_sha: &str,
        ) -> Result<Vec<CheckRun>> {
            self.record_call("list_check_runs");
            pop_result(&self.check_runs, "list_check_runs")
        }

        async fn list_workflow_runs_for_head(
            &self,
            _owner: &str,
            _repo: &str,
            _head_sha: &str,
        ) -> Result<Vec<WorkflowRun>> {
            self.record_call("list_workflow_runs_for_head");
            pop_result(&self.workflow_runs, "list_workflow_runs_for_head")
        }
    }

    #[async_trait]
    impl GitHubWorkflowApi for FakeGitHubApi {
        async fn list_workflow_jobs_for_run(
            &self,
            _owner: &str,
            _repo: &str,
            _run_id: u64,
        ) -> Result<Vec<WorkflowJob>> {
            self.record_call("list_workflow_jobs_for_run");
            pop_result(&self.workflow_jobs, "list_workflow_jobs_for_run")
        }

        async fn workflow_run_log(
            &self,
            _owner: &str,
            _repo: &str,
            _run_id: u64,
        ) -> Result<String> {
            self.record_call("workflow_run_log");
            pop_result(&self.workflow_logs, "workflow_run_log")
        }
    }

    #[async_trait]
    impl GitHubReviewApi for FakeGitHubApi {
        async fn current_user(&self) -> Result<String> {
            self.record_call("current_user");
            pop_result(&self.current_user, "current_user")
        }

        async fn list_pull_request_reviews(
            &self,
            _owner: &str,
            _repo: &str,
            _number: u64,
        ) -> Result<Vec<PullRequestReview>> {
            self.record_call("list_pull_request_reviews");
            pop_result(&self.reviews, "list_pull_request_reviews")
        }

        async fn pull_request_review_comment_count(
            &self,
            _owner: &str,
            _repo: &str,
            _number: u64,
            _review_id: &str,
        ) -> Result<usize> {
            self.record_call("pull_request_review_comment_count");
            pop_result(
                &self.review_comment_counts,
                "pull_request_review_comment_count",
            )
        }

        async fn list_review_threads(
            &self,
            _owner: &str,
            _repo: &str,
            _number: u64,
        ) -> Result<Vec<ReviewThread>> {
            self.record_call("list_review_threads");
            pop_result(&self.review_threads, "list_review_threads")
        }
    }

    #[async_trait]
    impl GitHubWorkflowActionApi for FakeGitHubApi {
        async fn dispatch_workflow(
            &self,
            _owner: &str,
            _repo: &str,
            _workflow_id: u64,
            _git_ref: &str,
        ) -> Result<()> {
            self.record_call("dispatch_workflow");
            pop_result(&self.dispatch_workflow_results, "dispatch_workflow")
        }

        async fn rerun_failed_jobs(&self, _owner: &str, _repo: &str, _run_id: u64) -> Result<()> {
            self.record_call("rerun_failed_jobs");
            pop_result(&self.rerun_failed_jobs_results, "rerun_failed_jobs")
        }
    }

    #[async_trait]
    impl GitHubPullRequestActionApi for FakeGitHubApi {
        async fn approve_pull_request(
            &self,
            _owner: &str,
            _repo: &str,
            _number: u64,
        ) -> Result<()> {
            self.record_call("approve_pull_request");
            pop_result(&self.approve_results, "approve_pull_request")
        }

        async fn request_pull_request_changes(
            &self,
            _owner: &str,
            _repo: &str,
            _number: u64,
            _body: &str,
        ) -> Result<()> {
            self.record_call("request_pull_request_changes");
            pop_result(
                &self.request_changes_results,
                "request_pull_request_changes",
            )
        }

        async fn merge_pull_request(
            &self,
            _owner: &str,
            _repo: &str,
            _number: u64,
            _head_sha: &str,
        ) -> Result<()> {
            self.record_call("merge_pull_request");
            pop_result(&self.merge_results, "merge_pull_request")
        }
    }

    #[async_trait]
    impl GitHubReviewMutationApi for FakeGitHubApi {
        async fn submit_pull_request_review(
            &self,
            _pull_request_review_node_id: &str,
            _event: SubmitPullRequestReviewEvent,
            _body: Option<&str>,
        ) -> Result<()> {
            self.record_call("submit_pull_request_review");
            pop_result(&self.submit_review_results, "submit_pull_request_review")
        }

        async fn create_pull_request_review_comment(
            &self,
            _owner: &str,
            _repo: &str,
            _number: u64,
            _commit_id: &str,
            _range: &ReviewCommentRange,
            _body: &str,
        ) -> Result<()> {
            self.record_call("create_pull_request_review_comment");
            pop_result(
                &self.create_comment_results,
                "create_pull_request_review_comment",
            )
        }

        async fn start_pull_request_review(
            &self,
            _pull_request_node_id: &str,
            _commit_id: &str,
            _range: &ReviewCommentRange,
            _body: &str,
        ) -> Result<String> {
            self.record_call("start_pull_request_review");
            pop_result(&self.start_review_results, "start_pull_request_review")
        }

        async fn add_pending_review_thread(
            &self,
            _pull_request_review_node_id: &str,
            _range: &ReviewCommentRange,
            _body: &str,
        ) -> Result<()> {
            self.record_call("add_pending_review_thread");
            pop_result(&self.pending_thread_results, "add_pending_review_thread")
        }

        async fn add_review_thread_reply(
            &self,
            _thread_id: &str,
            _pull_request_review_node_id: Option<&str>,
            _body: &str,
        ) -> Result<()> {
            self.record_call("add_review_thread_reply");
            pop_result(&self.reply_results, "add_review_thread_reply")
        }

        async fn resolve_review_thread(&self, _thread_id: &str) -> Result<()> {
            self.record_call("resolve_review_thread");
            pop_result(&self.resolve_thread_results, "resolve_review_thread")
        }

        async fn unresolve_review_thread(&self, _thread_id: &str) -> Result<()> {
            self.record_call("unresolve_review_thread");
            pop_result(&self.unresolve_thread_results, "unresolve_review_thread")
        }

        async fn update_review_comment(&self, _comment_id: &str, _body: &str) -> Result<()> {
            self.record_call("update_review_comment");
            pop_result(&self.update_comment_results, "update_review_comment")
        }

        async fn delete_review_comment(&self, _comment_id: &str) -> Result<()> {
            self.record_call("delete_review_comment");
            pop_result(&self.delete_comment_results, "delete_review_comment")
        }

        async fn add_review_comment_reaction(
            &self,
            _comment_id: &str,
            _content: ReactionContent,
        ) -> Result<()> {
            self.record_call("add_review_comment_reaction");
            pop_result(&self.add_reaction_results, "add_review_comment_reaction")
        }

        async fn remove_review_comment_reaction(
            &self,
            _comment_id: &str,
            _content: ReactionContent,
        ) -> Result<()> {
            self.record_call("remove_review_comment_reaction");
            pop_result(
                &self.remove_reaction_results,
                "remove_review_comment_reaction",
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use chrono::{Duration, Utc};
    use gpui::{
        AppContext, Context, Entity, IntoElement, Render, TestAppContext, VisualTestContext,
        Window, div,
    };
    use gpui_component::{Root, Theme, ThemeMode};
    use harbor_domain::{
        ChecksSummary, DiffFile, FileStatus, MergeState, PullRequest, PullRequestReview,
        PullRequestReviewState, RepoId, ReviewDecision, ReviewThreadState, WorkflowConclusion,
        WorkflowRun, WorkflowStatus,
    };
    use harbor_github::{ConditionalFetch, GitHubError, PullRequestEnrichment};
    use harbor_sync::{SyncState, SyncTarget};

    use crate::{
        actions::{PanelTab, PullRequestAction, WorkflowAction},
        test_fixtures::{diff_file, pull_request, review_thread, test_time},
        workspace::{
            AppView, PullRequestInboxCacheKey, PullRequestInboxMode,
            github_service::test_support::FakeGitHubApi,
        },
    };

    #[gpui::test]
    async fn loads_pull_request_inbox_success_from_service(cx: &mut TestAppContext) {
        let api = Arc::new(FakeGitHubApi::default());
        let pull_request = pull_request();
        api.push_light_pull_requests(Ok(ConditionalFetch::Modified {
            value: vec![pull_request.clone()],
            validator: None,
        }));
        enqueue_successful_detail_load(&api, &pull_request);
        let (view_entity, cx) = init_workspace_service_test(cx, api.clone());

        view_entity.update(cx, |view, cx| {
            view.load_pull_requests(pull_request.repo.clone(), cx);
        });
        cx.run_until_parked();

        view_entity.read_with(cx, |view, _| {
            assert_eq!(view.pull_requests.len(), 1);
            assert_eq!(view.pull_requests[0].number, pull_request.number);
            assert_eq!(view.pull_requests[0].title, pull_request.title);
            assert_eq!(view.pull_request_inbox.load_error(), None);
            assert!(!view.pull_request_inbox.is_loading());
        });
        assert_eq!(
            api.calls(),
            vec![
                "list_repository_pull_requests_light",
                "get_pull_request",
                "list_pull_request_files",
                "current_user",
                "list_pull_request_reviews",
                "list_review_threads"
            ]
        );
    }

    #[gpui::test]
    async fn prefetches_inactive_inbox_counts_without_loading_items(cx: &mut TestAppContext) {
        let api = Arc::new(FakeGitHubApi::default());
        let pull_request = pull_request();
        api.push_pull_request_count(Ok(4));
        api.push_pull_request_count(Ok(2));
        api.push_light_pull_requests(Ok(ConditionalFetch::Modified {
            value: vec![pull_request.clone()],
            validator: None,
        }));
        enqueue_successful_detail_load(&api, &pull_request);
        let (view_entity, cx) = init_workspace_service_test(cx, api.clone());

        view_entity.update(cx, |view, cx| {
            view.prefetch_inbox_counts = true;
            view.load_pull_requests(pull_request.repo.clone(), cx);
        });
        cx.run_until_parked();

        view_entity.read_with(cx, |view, _| {
            let closed_key = PullRequestInboxCacheKey::new(
                pull_request.repo.clone(),
                PullRequestInboxMode::Closed,
            );
            let needs_review_key = PullRequestInboxCacheKey::new(
                pull_request.repo.clone(),
                PullRequestInboxMode::NeedsReview,
            );

            assert_eq!(view.pull_request_inbox.snapshot_count(&closed_key), Some(4));
            assert_eq!(
                view.pull_request_inbox.snapshot_count(&needs_review_key),
                Some(2)
            );
            assert!(view.pull_request_inbox.snapshot(&closed_key).is_none());
            assert!(
                view.pull_request_inbox
                    .snapshot(&needs_review_key)
                    .is_none()
            );
        });

        let calls = api.calls();
        assert_eq!(
            calls
                .iter()
                .filter(|call| call.as_str() == "count_repository_pull_requests")
                .count(),
            2
        );
        assert!(
            !calls
                .iter()
                .any(|call| call.as_str() == "list_repository_pull_requests")
        );
    }

    #[gpui::test]
    async fn loads_diff_review_threads_and_defers_other_panel_fetches(cx: &mut TestAppContext) {
        let api = Arc::new(FakeGitHubApi::default());
        let pull_request = pull_request();
        let thread = review_thread(ReviewThreadState::Unresolved);
        api.push_pull_request_detail(Ok(pull_request.clone()));
        api.push_files(Ok(vec![test_diff_file()]));
        api.push_current_user(Ok("octocat".to_string()));
        api.push_reviews(Ok(Vec::new()));
        api.push_review_threads(Ok(vec![thread.clone()]));
        api.push_check_runs(Ok(Vec::new()));
        let (view_entity, cx) = init_workspace_service_test(cx, api.clone());

        view_entity.update(cx, |view, cx| {
            view.pull_requests = vec![pull_request];
            view.selection_state.reset_pull_request_index();
            view.load_selected_pull_request(cx);
        });
        cx.run_until_parked();

        assert_eq!(
            api.calls(),
            vec![
                "get_pull_request",
                "list_pull_request_files",
                "current_user",
                "list_pull_request_reviews",
                "list_review_threads"
            ]
        );
        view_entity.read_with(cx, |view, _| {
            assert_eq!(view.review_state.review_threads, vec![thread]);
        });

        view_entity.update(cx, |view, cx| {
            view.select_panel_tab(PanelTab::Checks, cx);
        });
        cx.run_until_parked();

        assert_eq!(
            api.calls(),
            vec![
                "get_pull_request",
                "list_pull_request_files",
                "current_user",
                "list_pull_request_reviews",
                "list_review_threads",
                "list_check_runs"
            ]
        );

        view_entity.update(cx, |view, cx| {
            view.select_panel_tab(PanelTab::Diff, cx);
            view.select_panel_tab(PanelTab::Checks, cx);
        });
        cx.run_until_parked();

        assert_eq!(
            api.calls(),
            vec![
                "get_pull_request",
                "list_pull_request_files",
                "current_user",
                "list_pull_request_reviews",
                "list_review_threads",
                "list_check_runs"
            ]
        );
    }

    #[gpui::test]
    async fn typed_repository_lookup_loads_pull_requests_after_validation(cx: &mut TestAppContext) {
        let api = Arc::new(FakeGitHubApi::default());
        let repository = RepoId::new("acme", "app");
        let pull_request = pull_request();
        api.push_repository_lookup(Ok(repository.clone()));
        api.push_light_pull_requests(Ok(ConditionalFetch::Modified {
            value: vec![pull_request.clone()],
            validator: None,
        }));
        enqueue_successful_detail_load(&api, &pull_request);
        let (view_entity, cx) = init_workspace_service_test(cx, api.clone());

        view_entity.update(cx, |view, cx| {
            view.open_typed_repository_from_switcher(repository.clone(), cx);
        });
        cx.run_until_parked();

        view_entity.read_with(cx, |view, _| {
            assert_eq!(view.current_repository(), Some(&repository));
            assert_eq!(view.pull_requests.len(), 1);
            assert!(!view.repository_state.is_loading());
        });
        assert_eq!(
            api.calls(),
            vec![
                "get_repository",
                "list_repository_pull_requests_light",
                "get_pull_request",
                "list_pull_request_files",
                "current_user",
                "list_pull_request_reviews",
                "list_review_threads"
            ]
        );
    }

    #[gpui::test]
    async fn reports_pull_request_inbox_failure_from_service(cx: &mut TestAppContext) {
        let api = Arc::new(FakeGitHubApi::default());
        let pull_request = pull_request();
        api.push_light_pull_requests(Err(github_error("inbox failed")));
        let (view_entity, cx) = init_workspace_service_test(cx, api.clone());

        view_entity.update(cx, |view, cx| {
            view.load_pull_requests(pull_request.repo.clone(), cx);
        });
        cx.run_until_parked();

        view_entity.read_with(cx, |view, _| {
            assert!(view.pull_requests.is_empty());
            assert!(
                view.pull_request_inbox
                    .load_error()
                    .is_some_and(|error| error.contains("inbox failed"))
            );
            assert_eq!(
                view.status,
                "Failed to load open pull requests from acme/app"
            );
            assert!(!view.pull_request_inbox.is_loading());
        });
        assert_eq!(api.calls(), vec!["list_repository_pull_requests_light"]);
    }

    #[gpui::test]
    async fn focus_catch_up_uses_light_inbox_refresh_only(cx: &mut TestAppContext) {
        let api = Arc::new(FakeGitHubApi::default());
        let pull_request = pull_request();
        api.push_light_pull_requests(Ok(ConditionalFetch::Modified {
            value: vec![pull_request.clone()],
            validator: None,
        }));
        let (view_entity, cx) = init_workspace_service_test(cx, api.clone());

        view_entity.update(cx, |view, cx| {
            view.repository_state
                .select_repository(pull_request.repo.clone());
            view.pull_requests = vec![pull_request];
            view.selection_state.reset_pull_request_index();
            view.sync_runtime.set_sync_state(
                SyncTarget::ActiveInboxLight,
                SyncState {
                    last_successful_fetch_at: Some(Utc::now() - Duration::seconds(31)),
                    ..Default::default()
                },
            );
            view.catch_up_active_inbox_after_focus(cx);
        });
        cx.run_until_parked();

        assert_eq!(api.calls(), vec!["list_repository_pull_requests_light"]);
    }

    #[gpui::test]
    async fn focus_catch_up_before_threshold_does_not_refresh(cx: &mut TestAppContext) {
        let api = Arc::new(FakeGitHubApi::default());
        let pull_request = pull_request();
        let (view_entity, cx) = init_workspace_service_test(cx, api.clone());

        view_entity.update(cx, |view, cx| {
            view.repository_state
                .select_repository(pull_request.repo.clone());
            view.sync_runtime.set_sync_state(
                SyncTarget::ActiveInboxLight,
                SyncState {
                    last_successful_fetch_at: Some(Utc::now()),
                    ..Default::default()
                },
            );
            view.catch_up_active_inbox_after_focus(cx);
        });
        cx.run_until_parked();

        assert!(api.calls().is_empty());
    }

    #[gpui::test]
    async fn focus_catch_up_does_not_run_needs_review_graphql_after_thirty_seconds(
        cx: &mut TestAppContext,
    ) {
        let api = Arc::new(FakeGitHubApi::default());
        let pull_request = pull_request();
        let (view_entity, cx) = init_workspace_service_test(cx, api.clone());

        view_entity.update(cx, |view, cx| {
            view.repository_state
                .select_repository(pull_request.repo.clone());
            view.pull_request_inbox
                .set_mode(PullRequestInboxMode::NeedsReview);
            view.sync_runtime.set_sync_state(
                SyncTarget::ActiveInbox,
                SyncState {
                    last_successful_fetch_at: Some(Utc::now() - Duration::seconds(31)),
                    ..Default::default()
                },
            );
            view.catch_up_active_inbox_after_focus(cx);
        });
        cx.run_until_parked();

        assert!(api.calls().is_empty());
    }

    #[gpui::test]
    async fn active_inbox_stale_marks_current_mode_target(cx: &mut TestAppContext) {
        let api = Arc::new(FakeGitHubApi::default());
        let (view_entity, cx) = init_workspace_service_test(cx, api);

        view_entity.update(cx, |view, _| {
            view.pull_request_inbox.set_mode(PullRequestInboxMode::Open);
            view.mark_active_inbox_stale();
            assert!(
                view.sync_runtime
                    .sync_state(SyncTarget::ActiveInboxLight)
                    .is_some_and(|state| state.stale)
            );
            assert!(
                !view
                    .sync_runtime
                    .sync_state(SyncTarget::ActiveInbox)
                    .is_some_and(|state| state.stale)
            );

            view.pull_request_inbox
                .set_mode(PullRequestInboxMode::NeedsReview);
            view.mark_active_inbox_stale();
            assert!(
                view.sync_runtime
                    .sync_state(SyncTarget::ActiveInbox)
                    .is_some_and(|state| state.stale)
            );
        });
    }

    #[gpui::test]
    async fn cached_detail_restore_preserves_diff_position_without_refetch(
        cx: &mut TestAppContext,
    ) {
        let api = Arc::new(FakeGitHubApi::default());
        let (view_entity, cx) = init_workspace_service_test(cx, api.clone());

        view_entity.update(cx, |view, cx| {
            view.pull_requests = vec![pull_request()];
            view.selection_state.reset_pull_request_index();
            view.detail_state.files = vec![
                diff_file("src/a.rs", FileStatus::Modified),
                diff_file("src/b.rs", FileStatus::Modified),
            ];
            view.detail_state.diffs = vec![None, None];
            mark_detail_sections_loaded(view);
            view.selection_state.set_diff_position(1, 4);
            view.active_tab = PanelTab::Diff;
            view.cache_current_pull_request_detail_snapshot();

            view.detail_state.files = vec![diff_file("src/other.rs", FileStatus::Modified)];
            view.detail_state.diffs = vec![None];
            view.selection_state.set_diff_position(0, 0);
            view.active_tab = PanelTab::Review;

            assert!(view.restore_selected_pull_request_detail_snapshot(cx));
            assert_eq!(
                view.detail_state
                    .files
                    .iter()
                    .map(|file| file.path.as_str())
                    .collect::<Vec<_>>(),
                vec!["src/a.rs", "src/b.rs"]
            );
            assert_eq!(view.active_file_index(), 1);
            assert_eq!(view.active_hunk_index(), 4);
            assert_eq!(view.active_tab, PanelTab::Diff);
            assert_eq!(view.status, "Showing cached PR #7 details");
        });
        cx.run_until_parked();

        assert!(api.calls().is_empty());
    }

    #[gpui::test]
    async fn cached_inbox_restore_bounds_stale_selection_without_refetch(cx: &mut TestAppContext) {
        let api = Arc::new(FakeGitHubApi::default());
        let pull_request = pull_request();
        let (view_entity, cx) = init_workspace_service_test(cx, api.clone());

        view_entity.update(cx, |view, cx| {
            view.repository_state
                .select_repository(pull_request.repo.clone());
            view.pull_request_inbox.set_mode(PullRequestInboxMode::Open);
            view.pull_requests = vec![pull_request.clone()];
            view.detail_state.files = vec![test_diff_file()];
            view.detail_state.diffs = vec![None];
            mark_detail_sections_loaded(view);
            view.selection_state.set_pull_request_index(9);
            view.selection_state.set_diff_position(7, 2);

            let key = view
                .current_pull_request_inbox_key()
                .expect("configured repository should produce inbox cache key");
            view.cache_current_pull_request_inbox_snapshot();
            assert_eq!(view.pull_request_inbox.snapshot_count(&key), Some(1));

            view.pull_requests.clear();
            view.detail_state.files.clear();
            view.detail_state.diffs.clear();
            view.selection_state.set_pull_request_index(3);
            view.selection_state.set_diff_position(3, 0);

            assert!(view.restore_pull_request_inbox_snapshot(key, cx));
            assert_eq!(view.pull_requests.len(), 1);
            assert_eq!(view.selected_pull_request_index(), 0);
            assert_eq!(view.selected_pull_request_number(), Some(7));
            assert_eq!(view.active_file_index(), 0);
            assert_eq!(view.active_hunk_index(), 2);
            assert_eq!(
                view.status,
                "Showing cached open pull requests from acme/app"
            );
        });
        cx.run_until_parked();

        assert!(api.calls().is_empty());
    }

    #[gpui::test]
    async fn selected_metadata_refresh_does_not_refetch_files(cx: &mut TestAppContext) {
        let api = Arc::new(FakeGitHubApi::default());
        let mut updated_pull_request = pull_request();
        updated_pull_request.title = "Updated title".to_string();
        api.push_pull_request_detail(Ok(updated_pull_request.clone()));
        let (view_entity, cx) = init_workspace_service_test(cx, api.clone());

        view_entity.update(cx, |view, cx| {
            view.pull_requests = vec![pull_request()];
            view.selection_state.reset_pull_request_index();
            view.refresh_selected_pull_request_metadata_only(cx);
        });
        cx.run_until_parked();

        assert_eq!(api.calls(), vec!["get_pull_request"]);
        view_entity.read_with(cx, |view, _| {
            assert_eq!(view.pull_requests[0].title, "Updated title");
            assert!(view.detail_state.files.is_empty());
        });
    }

    #[gpui::test]
    async fn manual_inbox_refresh_can_force_enrichment(cx: &mut TestAppContext) {
        let api = Arc::new(FakeGitHubApi::default());
        let pull_request = pull_request();
        api.push_light_pull_requests(Ok(ConditionalFetch::Modified {
            value: vec![pull_request.clone()],
            validator: None,
        }));
        api.push_pull_request_enrichments(Ok(vec![enrichment(&pull_request)]));
        let (view_entity, cx) = init_workspace_service_test(cx, api.clone());

        view_entity.update(cx, |view, cx| {
            view.repository_state
                .select_repository(pull_request.repo.clone());
            view.pull_requests = vec![pull_request.clone()];
            view.selection_state.reset_pull_request_index();
            view.refresh_pull_requests(pull_request.repo, cx);
        });
        cx.run_until_parked();

        assert_eq!(
            api.calls(),
            vec![
                "list_repository_pull_requests_light",
                "enrich_pull_requests_by_node_ids"
            ]
        );
    }

    #[gpui::test]
    async fn ignores_stale_pull_request_detail_results_after_selection_changes(
        cx: &mut TestAppContext,
    ) {
        let api = Arc::new(FakeGitHubApi::default());
        let first_pull_request = pull_request();
        let mut stale_detail = first_pull_request.clone();
        stale_detail.title = "Stale detail".to_string();
        let mut second_pull_request = pull_request();
        second_pull_request.number = 8;
        second_pull_request.title = "Selected detail".to_string();
        second_pull_request.head_sha = "def456".to_string();
        api.push_pull_request_detail(Ok(stale_detail));
        let (view_entity, cx) = init_workspace_service_test(cx, api);

        view_entity.update(cx, |view, cx| {
            view.pull_requests = vec![first_pull_request, second_pull_request.clone()];
            view.selection_state.reset_pull_request_index();
            let generation_before = view.review_data_generation();
            view.refresh_selected_pull_request(cx);
            assert!(view.review_data_generation() > generation_before);
            view.selection_state.set_pull_request_index(1);
        });
        cx.run_until_parked();

        view_entity.read_with(cx, |view, _| {
            assert_eq!(view.selected_pull_request_index(), 1);
            assert_eq!(view.pull_requests[1].title, "Selected detail");
            assert!(view.detail_state.files.is_empty());
            assert!(view.review_state.review_threads.is_empty());
        });
    }

    #[gpui::test]
    async fn selected_metadata_replace_preserves_cached_row_signals(cx: &mut TestAppContext) {
        let api = Arc::new(FakeGitHubApi::default());
        let mut row_pull_request = pull_request();
        row_pull_request.review_decision = Some(ReviewDecision::Approved);
        row_pull_request.merge_state = Some(MergeState::Clean);
        row_pull_request.checks_summary = ChecksSummary {
            total: 3,
            passed: 3,
            failed: 0,
            pending: 0,
            skipped: 0,
        };
        row_pull_request.unresolved_threads = 2;
        let mut metadata = row_pull_request.clone();
        metadata.title = "REST detail".to_string();
        metadata.review_decision = None;
        metadata.merge_state = Some(MergeState::Unknown);
        metadata.checks_summary = ChecksSummary::default();
        metadata.unresolved_threads = 0;
        let (view_entity, cx) = init_workspace_service_test(cx, api);

        view_entity.update(cx, |view, _| {
            view.pull_requests = vec![row_pull_request.clone()];
            view.selection_state.reset_pull_request_index();
            view.replace_selected_pull_request_preserving_row_fields(metadata);
        });

        view_entity.read_with(cx, |view, _| {
            let selected = &view.pull_requests[0];
            assert_eq!(selected.title, "REST detail");
            assert_eq!(selected.review_decision, Some(ReviewDecision::Approved));
            assert_eq!(selected.merge_state, Some(MergeState::Clean));
            assert_eq!(selected.checks_summary, row_pull_request.checks_summary);
            assert_eq!(selected.unresolved_threads, 2);
        });
    }

    #[gpui::test]
    async fn refresh_review_data_keeps_reviews_when_threads_fail(cx: &mut TestAppContext) {
        let api = Arc::new(FakeGitHubApi::default());
        let pull_request = pull_request();
        let review = pull_request_review("review-1", PullRequestReviewState::Approved);
        api.push_current_user(Ok("octocat".to_string()));
        api.push_reviews(Ok(vec![review.clone()]));
        api.push_review_threads(Err(github_error("threads failed")));
        let (view_entity, cx) = init_workspace_service_test(cx, api);

        view_entity.update(cx, |view, cx| {
            view.pull_requests = vec![pull_request];
            view.selection_state.reset_pull_request_index();
            view.load_selected_review_data(cx);
        });
        cx.run_until_parked();

        view_entity.read_with(cx, |view, _| {
            assert_eq!(view.review_state.pull_request_reviews, vec![review]);
            assert!(view.review_state.review_threads.is_empty());
            assert!(
                view.review_state
                    .reviews_error()
                    .is_some_and(|error| error.contains("Failed to load review threads"))
            );
            assert_eq!(
                view.status,
                "Refreshed review history for PR #7, but threads failed"
            );
        });
    }

    #[gpui::test]
    async fn refresh_review_data_keeps_threads_when_reviews_fail(cx: &mut TestAppContext) {
        let api = Arc::new(FakeGitHubApi::default());
        let pull_request = pull_request();
        let thread = review_thread(ReviewThreadState::Unresolved);
        api.push_current_user(Ok("octocat".to_string()));
        api.push_reviews(Err(github_error("reviews failed")));
        api.push_review_threads(Ok(vec![thread.clone()]));
        let (view_entity, cx) = init_workspace_service_test(cx, api);

        view_entity.update(cx, |view, cx| {
            view.pull_requests = vec![pull_request];
            view.selection_state.reset_pull_request_index();
            view.load_selected_review_data(cx);
        });
        cx.run_until_parked();

        view_entity.read_with(cx, |view, _| {
            assert!(view.review_state.pull_request_reviews.is_empty());
            assert_eq!(view.review_state.review_threads, vec![thread]);
            assert!(
                view.review_state
                    .reviews_error()
                    .is_some_and(|error| error.contains("Failed to load review history"))
            );
            assert_eq!(
                view.status,
                "Refreshed 1 review threads for PR #7, but review history failed"
            );
        });
    }

    #[gpui::test]
    async fn workflow_action_reports_success_and_failure_from_service(cx: &mut TestAppContext) {
        let success_api = Arc::new(FakeGitHubApi::default());
        success_api.push_dispatch_workflow(Ok(()));
        let (success_view, cx) = init_workspace_service_test(cx, success_api.clone());

        success_view.update(cx, |view, cx| {
            view.pull_requests = vec![pull_request()];
            view.selection_state.reset_pull_request_index();
            view.detail_state.workflow_runs = vec![workflow_run()];
            view.run_workflow_action(WorkflowAction::DispatchBuild, cx);
            assert!(view.action_runtime.workflow_action_running());
            assert_eq!(view.status, "Dispatching CI on feature");
            view.pull_requests.clear();
        });
        cx.run_until_parked();

        success_view.read_with(cx, |view, _| {
            assert!(!view.action_runtime.workflow_action_running());
            assert_eq!(view.action_runtime.workflow_action_error(), None);
            assert_eq!(view.status, "Dispatched CI on feature");
        });
        assert_eq!(success_api.calls(), vec!["dispatch_workflow"]);

        let failure_api = Arc::new(FakeGitHubApi::default());
        failure_api.push_dispatch_workflow(Err(github_error("dispatch failed")));
        let (failure_view, cx) = init_workspace_service_test(cx, failure_api);

        failure_view.update(cx, |view, cx| {
            view.pull_requests = vec![pull_request()];
            view.selection_state.reset_pull_request_index();
            view.detail_state.workflow_runs = vec![workflow_run()];
            view.run_workflow_action(WorkflowAction::DispatchBuild, cx);
            assert_eq!(view.status, "Dispatching CI on feature");
        });
        cx.run_until_parked();

        failure_view.read_with(cx, |view, _| {
            assert!(!view.action_runtime.workflow_action_running());
            assert!(
                view.action_runtime
                    .workflow_action_error()
                    .is_some_and(|error| error.contains("Failed to dispatch workflow"))
            );
            assert!(view.status.contains("dispatch failed"));
        });
    }

    #[gpui::test]
    async fn pull_request_action_reports_success_and_failure_from_service(cx: &mut TestAppContext) {
        let success_api = Arc::new(FakeGitHubApi::default());
        success_api.push_approve_pull_request(Ok(()));
        let (success_view, cx) = init_workspace_service_test(cx, success_api.clone());

        success_view.update_in(cx, |view, window, cx| {
            view.pull_requests = vec![pull_request()];
            view.selection_state.reset_pull_request_index();
            view.run_pull_request_action(PullRequestAction::Approve, window, cx);
            assert!(view.action_runtime.pull_request_action_running());
            assert_eq!(view.status, "Approving PR #7");
        });
        cx.run_until_parked();

        success_view.read_with(cx, |view, _| {
            assert!(!view.action_runtime.pull_request_action_running());
            assert_eq!(view.action_runtime.pull_request_action_error(), None);
            assert_eq!(view.status, "Approved PR #7");
        });
        assert_eq!(success_api.calls(), vec!["approve_pull_request"]);

        let failure_api = Arc::new(FakeGitHubApi::default());
        failure_api.push_approve_pull_request(Err(github_error("approval failed")));
        let (failure_view, cx) = init_workspace_service_test(cx, failure_api);

        failure_view.update_in(cx, |view, window, cx| {
            view.pull_requests = vec![pull_request()];
            view.selection_state.reset_pull_request_index();
            view.run_pull_request_action(PullRequestAction::Approve, window, cx);
            assert_eq!(view.status, "Approving PR #7");
        });
        cx.run_until_parked();

        failure_view.read_with(cx, |view, _| {
            assert!(!view.action_runtime.pull_request_action_running());
            assert!(
                view.action_runtime
                    .pull_request_action_error()
                    .is_some_and(|error| error.contains("Failed to approve pull request"))
            );
            assert!(view.status.contains("approval failed"));
        });
    }

    fn init_workspace_service_test(
        cx: &mut TestAppContext,
        api: Arc<FakeGitHubApi>,
    ) -> (Entity<AppView>, &mut VisualTestContext) {
        cx.update(|cx| {
            gpui_component::init(cx);
            Theme::change(ThemeMode::Dark, None, cx);
        });

        let mut view_entity = None;
        let (_, cx) = cx.add_window_view(|window, cx| {
            let view = cx.new(|cx| AppView::new_with_github_api(api, window, cx));
            view_entity = Some(view);
            Root::new(cx.new(|_| EmptyHarness), window, cx)
        });

        (
            view_entity.expect("workspace service test AppView should be created"),
            cx,
        )
    }

    fn enqueue_successful_detail_load(api: &FakeGitHubApi, pull_request: &PullRequest) {
        api.push_pull_request_detail(Ok(pull_request.clone()));
        api.push_files(Ok(vec![test_diff_file()]));
        api.push_check_runs(Ok(Vec::new()));
        api.push_workflow_runs(Ok(Vec::new()));
        api.push_current_user(Ok("octocat".to_string()));
        api.push_reviews(Ok(Vec::new()));
        api.push_review_threads(Ok(Vec::new()));
    }

    fn mark_detail_sections_loaded(view: &mut AppView) {
        view.detail_state.apply_details_success();
        view.detail_state.apply_files_success();
        view.detail_state.apply_checks_success();
        view.detail_state.apply_workflows_success();
        view.review_state.apply_reviews_success();
    }

    fn test_diff_file() -> DiffFile {
        let mut file = diff_file("src/lib.rs", FileStatus::Modified);
        file.patch = Some("@@ -1 +1 @@\n-old\n+new\n".to_string());
        file
    }

    fn pull_request_review(id: &str, state: PullRequestReviewState) -> PullRequestReview {
        PullRequestReview {
            id: id.to_string(),
            node_id: Some(format!("{id}-node")),
            author: "octocat".to_string(),
            state,
            body: None,
            submitted_at: Some(test_time()),
        }
    }

    fn workflow_run() -> WorkflowRun {
        WorkflowRun {
            id: 42,
            workflow_id: Some(9),
            name: "build".to_string(),
            workflow_name: Some("CI".to_string()),
            status: WorkflowStatus::Completed,
            conclusion: Some(WorkflowConclusion::Failure),
            head_branch: "feature".to_string(),
            head_sha: "abc123".to_string(),
            event: "pull_request".to_string(),
            url: "https://api.github.com/repos/acme/app/actions/runs/42".to_string(),
            html_url: "https://github.com/acme/app/actions/runs/42".to_string(),
            created_at: test_time(),
            updated_at: test_time(),
        }
    }

    fn github_error(message: &str) -> GitHubError {
        GitHubError::Transport(message.to_string())
    }

    fn enrichment(pull_request: &PullRequest) -> PullRequestEnrichment {
        PullRequestEnrichment {
            node_id: pull_request.node_id.clone(),
            review_decision: pull_request.review_decision,
            merge_state: pull_request.merge_state,
            checks_summary: Default::default(),
        }
    }

    struct EmptyHarness;

    impl Render for EmptyHarness {
        fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
            div()
        }
    }
}
