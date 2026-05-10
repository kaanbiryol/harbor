use async_trait::async_trait;
use harbor_domain::{
    CheckRun, DiffFile, PullRequest, PullRequestReview, ReactionContent, RepoId,
    ReviewCommentRange, ReviewThread, WorkflowJob, WorkflowRun,
};
use harbor_github::{
    GhCliTransport, GitHubClient, GitHubError, GitHubRateLimitStatus, PullRequestListFilter,
    Result, SubmitPullRequestReviewEvent,
};
use std::sync::Mutex;

#[async_trait]
pub(crate) trait GitHubApi: Send + Sync {
    fn latest_rate_limit(&self) -> Option<GitHubRateLimitStatus>;

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

#[async_trait]
impl GitHubApi for RealGitHubApi {
    fn latest_rate_limit(&self) -> Option<GitHubRateLimitStatus> {
        self.client.latest_rate_limit()
    }

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
        GitHubError, GitHubRateLimitStatus, PullRequestListFilter, Result,
        SubmitPullRequestReviewEvent,
    };

    use super::GitHubApi;

    #[derive(Clone, Default)]
    pub(crate) struct FakeGitHubApi {
        calls: Arc<Mutex<Vec<String>>>,
        repositories: Arc<Mutex<VecDeque<Result<Vec<RepoId>>>>>,
        pull_requests: Arc<Mutex<VecDeque<Result<Vec<PullRequest>>>>>,
        pull_request_details: Arc<Mutex<VecDeque<Result<PullRequest>>>>,
        files: Arc<Mutex<VecDeque<Result<Vec<DiffFile>>>>>,
        check_runs: Arc<Mutex<VecDeque<Result<Vec<CheckRun>>>>>,
        workflow_runs: Arc<Mutex<VecDeque<Result<Vec<WorkflowRun>>>>>,
        workflow_jobs: Arc<Mutex<VecDeque<Result<Vec<WorkflowJob>>>>>,
        workflow_logs: Arc<Mutex<VecDeque<Result<String>>>>,
        current_user: Arc<Mutex<VecDeque<Result<String>>>>,
        reviews: Arc<Mutex<VecDeque<Result<Vec<PullRequestReview>>>>>,
        review_comment_counts: Arc<Mutex<VecDeque<Result<usize>>>>,
        review_threads: Arc<Mutex<VecDeque<Result<Vec<ReviewThread>>>>>,
        dispatch_workflow_results: Arc<Mutex<VecDeque<Result<()>>>>,
        rerun_failed_jobs_results: Arc<Mutex<VecDeque<Result<()>>>>,
        approve_results: Arc<Mutex<VecDeque<Result<()>>>>,
        request_changes_results: Arc<Mutex<VecDeque<Result<()>>>>,
        merge_results: Arc<Mutex<VecDeque<Result<()>>>>,
        submit_review_results: Arc<Mutex<VecDeque<Result<()>>>>,
        create_comment_results: Arc<Mutex<VecDeque<Result<()>>>>,
        start_review_results: Arc<Mutex<VecDeque<Result<String>>>>,
        pending_thread_results: Arc<Mutex<VecDeque<Result<()>>>>,
        reply_results: Arc<Mutex<VecDeque<Result<()>>>>,
        resolve_thread_results: Arc<Mutex<VecDeque<Result<()>>>>,
        unresolve_thread_results: Arc<Mutex<VecDeque<Result<()>>>>,
        update_comment_results: Arc<Mutex<VecDeque<Result<()>>>>,
        delete_comment_results: Arc<Mutex<VecDeque<Result<()>>>>,
        add_reaction_results: Arc<Mutex<VecDeque<Result<()>>>>,
        remove_reaction_results: Arc<Mutex<VecDeque<Result<()>>>>,
    }

    impl FakeGitHubApi {
        pub(crate) fn push_pull_requests(&self, result: Result<Vec<PullRequest>>) {
            push_result(&self.pull_requests, result);
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

    fn push_result<T>(queue: &Arc<Mutex<VecDeque<Result<T>>>>, result: Result<T>) {
        queue
            .lock()
            .expect("fake GitHub API queue mutex should not be poisoned")
            .push_back(result);
    }

    fn pop_result<T>(queue: &Arc<Mutex<VecDeque<Result<T>>>>, name: &str) -> Result<T> {
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

    #[async_trait]
    impl GitHubApi for FakeGitHubApi {
        fn latest_rate_limit(&self) -> Option<GitHubRateLimitStatus> {
            None
        }

        async fn list_repositories(&self) -> Result<Vec<RepoId>> {
            self.record_call("list_repositories");
            pop_result(&self.repositories, "list_repositories")
        }

        async fn list_repository_pull_requests(
            &self,
            _repository: &RepoId,
            _filter: PullRequestListFilter,
        ) -> Result<Vec<PullRequest>> {
            self.record_call("list_repository_pull_requests");
            pop_result(&self.pull_requests, "list_repository_pull_requests")
        }

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

    use gpui::{
        AppContext, Context, Entity, IntoElement, Render, TestAppContext, VisualTestContext,
        Window, div,
    };
    use gpui_component::{Root, Theme, ThemeMode};
    use harbor_domain::{
        DiffFile, FileStatus, PullRequest, PullRequestReview, PullRequestReviewState,
        ReviewThreadState, WorkflowConclusion, WorkflowRun, WorkflowStatus,
    };
    use harbor_github::GitHubError;

    use crate::{
        actions::{PanelTab, PullRequestAction, WorkflowAction},
        test_fixtures::{diff_file, pull_request, review_thread, test_time},
        workspace::{AppView, github_service::test_support::FakeGitHubApi},
    };

    #[gpui::test]
    async fn loads_pull_request_inbox_success_from_service(cx: &mut TestAppContext) {
        let api = Arc::new(FakeGitHubApi::default());
        let pull_request = pull_request();
        api.push_pull_requests(Ok(vec![pull_request.clone()]));
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
            assert_eq!(view.load_error, None);
            assert!(!view.is_loading_prs);
        });
        assert_eq!(
            api.calls(),
            vec![
                "list_repository_pull_requests",
                "get_pull_request",
                "list_pull_request_files"
            ]
        );
    }

    #[gpui::test]
    async fn defers_panel_specific_pull_request_fetches_until_panel_opens(cx: &mut TestAppContext) {
        let api = Arc::new(FakeGitHubApi::default());
        let pull_request = pull_request();
        api.push_pull_request_detail(Ok(pull_request.clone()));
        api.push_files(Ok(vec![test_diff_file()]));
        api.push_check_runs(Ok(Vec::new()));
        let (view_entity, cx) = init_workspace_service_test(cx, api.clone());

        view_entity.update(cx, |view, cx| {
            view.pull_requests = vec![pull_request];
            view.selected_pr = 0;
            view.load_selected_pull_request(cx);
        });
        cx.run_until_parked();

        assert_eq!(
            api.calls(),
            vec!["get_pull_request", "list_pull_request_files"]
        );

        view_entity.update(cx, |view, cx| {
            view.select_panel_tab(PanelTab::Checks, cx);
        });
        cx.run_until_parked();

        assert_eq!(
            api.calls(),
            vec![
                "get_pull_request",
                "list_pull_request_files",
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
                "list_check_runs"
            ]
        );
    }

    #[gpui::test]
    async fn reports_pull_request_inbox_failure_from_service(cx: &mut TestAppContext) {
        let api = Arc::new(FakeGitHubApi::default());
        let pull_request = pull_request();
        api.push_pull_requests(Err(github_error("inbox failed")));
        let (view_entity, cx) = init_workspace_service_test(cx, api.clone());

        view_entity.update(cx, |view, cx| {
            view.load_pull_requests(pull_request.repo.clone(), cx);
        });
        cx.run_until_parked();

        view_entity.read_with(cx, |view, _| {
            assert!(view.pull_requests.is_empty());
            assert!(
                view.load_error
                    .as_deref()
                    .is_some_and(|error| error.contains("inbox failed"))
            );
            assert_eq!(
                view.status,
                "Failed to load open pull requests from acme/app"
            );
            assert!(!view.is_loading_prs);
        });
        assert_eq!(api.calls(), vec!["list_repository_pull_requests"]);
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
            view.selected_pr = 0;
            let generation_before = view.review_data_generation();
            view.refresh_selected_pull_request(cx);
            assert!(view.review_data_generation() > generation_before);
            view.selected_pr = 1;
        });
        cx.run_until_parked();

        view_entity.read_with(cx, |view, _| {
            assert_eq!(view.selected_pr, 1);
            assert_eq!(view.pull_requests[1].title, "Selected detail");
            assert!(view.files.is_empty());
            assert!(view.review_threads.is_empty());
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
            view.selected_pr = 0;
            view.load_selected_review_data(cx);
        });
        cx.run_until_parked();

        view_entity.read_with(cx, |view, _| {
            assert_eq!(view.pull_request_reviews, vec![review]);
            assert!(view.review_threads.is_empty());
            assert!(
                view.reviews_error
                    .as_deref()
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
            view.selected_pr = 0;
            view.load_selected_review_data(cx);
        });
        cx.run_until_parked();

        view_entity.read_with(cx, |view, _| {
            assert!(view.pull_request_reviews.is_empty());
            assert_eq!(view.review_threads, vec![thread]);
            assert!(
                view.reviews_error
                    .as_deref()
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
            view.selected_pr = 0;
            view.workflow_runs = vec![workflow_run()];
            view.run_workflow_action(WorkflowAction::DispatchBuild, cx);
            assert!(view.is_running_action);
            assert_eq!(view.status, "Dispatching CI on feature");
            view.pull_requests.clear();
        });
        cx.run_until_parked();

        success_view.read_with(cx, |view, _| {
            assert!(!view.is_running_action);
            assert_eq!(view.action_error, None);
            assert_eq!(view.status, "Dispatched CI on feature");
        });
        assert_eq!(success_api.calls(), vec!["dispatch_workflow"]);

        let failure_api = Arc::new(FakeGitHubApi::default());
        failure_api.push_dispatch_workflow(Err(github_error("dispatch failed")));
        let (failure_view, cx) = init_workspace_service_test(cx, failure_api);

        failure_view.update(cx, |view, cx| {
            view.pull_requests = vec![pull_request()];
            view.selected_pr = 0;
            view.workflow_runs = vec![workflow_run()];
            view.run_workflow_action(WorkflowAction::DispatchBuild, cx);
            assert_eq!(view.status, "Dispatching CI on feature");
        });
        cx.run_until_parked();

        failure_view.read_with(cx, |view, _| {
            assert!(!view.is_running_action);
            assert!(
                view.action_error
                    .as_deref()
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
            view.selected_pr = 0;
            view.run_pull_request_action(PullRequestAction::Approve, window, cx);
            assert!(view.is_running_pr_action);
            assert_eq!(view.status, "Approving PR #7");
        });
        cx.run_until_parked();

        success_view.read_with(cx, |view, _| {
            assert!(!view.is_running_pr_action);
            assert_eq!(view.pr_action_error, None);
            assert_eq!(view.status, "Approved PR #7");
        });
        assert_eq!(success_api.calls(), vec!["approve_pull_request"]);

        let failure_api = Arc::new(FakeGitHubApi::default());
        failure_api.push_approve_pull_request(Err(github_error("approval failed")));
        let (failure_view, cx) = init_workspace_service_test(cx, failure_api);

        failure_view.update_in(cx, |view, window, cx| {
            view.pull_requests = vec![pull_request()];
            view.selected_pr = 0;
            view.run_pull_request_action(PullRequestAction::Approve, window, cx);
            assert_eq!(view.status, "Approving PR #7");
        });
        cx.run_until_parked();

        failure_view.read_with(cx, |view, _| {
            assert!(!view.is_running_pr_action);
            assert!(
                view.pr_action_error
                    .as_deref()
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

    struct EmptyHarness;

    impl Render for EmptyHarness {
        fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
            div()
        }
    }
}
