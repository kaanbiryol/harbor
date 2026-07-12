#[path = "github_service_test_support/actions.rs"]
mod actions;
#[path = "github_service_test_support/queues.rs"]
mod queues;

use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use harbor_domain::{
    CheckRun, DiffFile, PullRequest, PullRequestComment, PullRequestCommit, PullRequestReview,
    RepoId, ReviewThread, Workflow, WorkflowJob, WorkflowRun,
};
use harbor_github::{
    ConditionalFetch, GitHubRateLimitStatus, HttpCacheValidator, PullRequestEnrichment,
    PullRequestListFilter, PullRequestMetadataOptions, PullRequestPage, PullRequestPageCursor,
    RepositoryList, Result,
};

use harbor_sync::PullRequestInboxSource;

use super::{
    GitHubAuthApi, GitHubPullRequestDetailApi, GitHubRateLimitApi, GitHubRepositoryApi,
    GitHubReviewApi, GitHubWorkflowApi,
};
use crate::workspace::GitHubAuthSource;
use queues::{FakeQueue, pop_result, push_result};

type FakeLightPullRequestRequest = (Option<PullRequestPageCursor>, usize, bool);
type FakeLightPullRequestRequests = Arc<Mutex<Vec<FakeLightPullRequestRequest>>>;

#[derive(Clone, Default)]
pub(crate) struct FakeGitHubApi {
    calls: Arc<Mutex<Vec<String>>>,
    light_pull_request_requests: FakeLightPullRequestRequests,
    repositories: FakeQueue<RepositoryList>,
    repository_lookups: FakeQueue<RepoId>,
    metadata_options: FakeQueue<PullRequestMetadataOptions>,
    pull_request_pages: FakeQueue<PullRequestPage>,
    pull_request_counts: FakeQueue<usize>,
    light_pull_request_pages: FakeQueue<ConditionalFetch<PullRequestPage>>,
    pull_request_enrichments: FakeQueue<Vec<PullRequestEnrichment>>,
    pull_request_details: FakeQueue<PullRequest>,
    files: FakeQueue<Vec<DiffFile>>,
    commits: FakeQueue<Vec<PullRequestCommit>>,
    mark_file_viewed_results: FakeQueue<()>,
    unmark_file_viewed_results: FakeQueue<()>,
    check_runs: FakeQueue<Vec<CheckRun>>,
    workflows: FakeQueue<Vec<Workflow>>,
    repository_workflow_runs: FakeQueue<Vec<WorkflowRun>>,
    workflow_runs_for_workflow: FakeQueue<Vec<WorkflowRun>>,
    workflow_runs: FakeQueue<Vec<WorkflowRun>>,
    workflow_jobs: FakeQueue<Vec<WorkflowJob>>,
    workflow_logs: FakeQueue<String>,
    current_user: FakeQueue<String>,
    reviews: FakeQueue<Vec<PullRequestReview>>,
    pull_request_comments: FakeQueue<Vec<PullRequestComment>>,
    review_comment_counts: FakeQueue<usize>,
    review_threads: FakeQueue<Vec<ReviewThread>>,
    dispatch_workflow_results: FakeQueue<()>,
    rerun_failed_jobs_results: FakeQueue<()>,
    update_pull_request_body_results: FakeQueue<()>,
    request_reviewer_results: FakeQueue<()>,
    add_assignee_results: FakeQueue<()>,
    add_label_results: FakeQueue<()>,
    create_pull_request_comment_results: FakeQueue<()>,
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
        let result = result.map(|fetch| match fetch {
            ConditionalFetch::Modified { value, validator } => ConditionalFetch::Modified {
                value: page_from_pull_requests(value),
                validator,
            },
            ConditionalFetch::NotModified { validator } => {
                ConditionalFetch::NotModified { validator }
            }
        });
        push_result(&self.light_pull_request_pages, result);
    }

    pub(crate) fn push_light_pull_request_page(
        &self,
        result: Result<ConditionalFetch<PullRequestPage>>,
    ) {
        push_result(&self.light_pull_request_pages, result);
    }

    pub(crate) fn push_pull_request_count(&self, result: Result<usize>) {
        push_result(&self.pull_request_counts, result);
    }

    pub(crate) fn push_pull_request_enrichments(&self, result: Result<Vec<PullRequestEnrichment>>) {
        push_result(&self.pull_request_enrichments, result);
    }

    pub(crate) fn push_pull_request_detail(&self, result: Result<PullRequest>) {
        push_result(&self.pull_request_details, result);
    }

    pub(crate) fn push_files(&self, result: Result<Vec<DiffFile>>) {
        push_result(&self.files, result);
    }

    pub(crate) fn push_mark_file_viewed(&self, result: Result<()>) {
        push_result(&self.mark_file_viewed_results, result);
    }

    pub(crate) fn push_unmark_file_viewed(&self, result: Result<()>) {
        push_result(&self.unmark_file_viewed_results, result);
    }

    pub(crate) fn push_check_runs(&self, result: Result<Vec<CheckRun>>) {
        push_result(&self.check_runs, result);
    }

    pub(crate) fn push_workflow_runs(&self, result: Result<Vec<WorkflowRun>>) {
        push_result(&self.workflow_runs, result);
    }

    pub(crate) fn push_workflows(&self, result: Result<Vec<Workflow>>) {
        push_result(&self.workflows, result);
    }

    pub(crate) fn push_repository_workflow_runs(&self, result: Result<Vec<WorkflowRun>>) {
        push_result(&self.repository_workflow_runs, result);
    }

    pub(crate) fn push_workflow_runs_for_workflow(&self, result: Result<Vec<WorkflowRun>>) {
        push_result(&self.workflow_runs_for_workflow, result);
    }

    pub(crate) fn push_current_user(&self, result: Result<String>) {
        push_result(&self.current_user, result);
    }

    pub(crate) fn push_reviews(&self, result: Result<Vec<PullRequestReview>>) {
        push_result(&self.reviews, result);
    }

    pub(crate) fn push_pull_request_comments(&self, result: Result<Vec<PullRequestComment>>) {
        push_result(&self.pull_request_comments, result);
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

    pub(crate) fn push_request_pull_request_reviewer(&self, result: Result<()>) {
        push_result(&self.request_reviewer_results, result);
    }

    pub(crate) fn push_update_pull_request_body(&self, result: Result<()>) {
        push_result(&self.update_pull_request_body_results, result);
    }

    pub(crate) fn push_add_pull_request_assignee(&self, result: Result<()>) {
        push_result(&self.add_assignee_results, result);
    }

    pub(crate) fn push_add_pull_request_label(&self, result: Result<()>) {
        push_result(&self.add_label_results, result);
    }

    pub(crate) fn push_create_pull_request_comment(&self, result: Result<()>) {
        push_result(&self.create_pull_request_comment_results, result);
    }

    pub(crate) fn calls(&self) -> Vec<String> {
        self.calls
            .lock()
            .expect("fake GitHub API calls mutex should not be poisoned")
            .clone()
    }

    pub(crate) fn light_pull_request_requests(&self) -> Vec<FakeLightPullRequestRequest> {
        self.light_pull_request_requests
            .lock()
            .expect("fake GitHub API request mutex should not be poisoned")
            .clone()
    }

    fn record_call(&self, name: &str) {
        self.calls
            .lock()
            .expect("fake GitHub API calls mutex should not be poisoned")
            .push(name.to_string());
    }
}

fn page_from_pull_requests(pull_requests: Vec<PullRequest>) -> PullRequestPage {
    PullRequestPage {
        total_count: Some(pull_requests.len()),
        next_cursor: None,
        pull_requests,
    }
}

impl GitHubRateLimitApi for FakeGitHubApi {
    fn latest_rate_limit(&self) -> Option<GitHubRateLimitStatus> {
        None
    }
}

impl GitHubAuthApi for FakeGitHubApi {
    fn configure_token(&self, _token: String, source: GitHubAuthSource) -> Result<()> {
        self.record_call(match source {
            GitHubAuthSource::OAuth => "configure_oauth_token",
            GitHubAuthSource::GhCli => "configure_gh_cli_token",
        });
        Ok(())
    }

    fn configure_gh_cli(&self) -> Result<()> {
        self.record_call("configure_gh_cli");
        Ok(())
    }

    fn clear_auth(&self) -> Result<()> {
        self.record_call("clear_auth");
        Ok(())
    }

    fn has_auth(&self) -> bool {
        true
    }
}

#[async_trait]
impl PullRequestInboxSource for FakeGitHubApi {
    fn latest_rate_limits(&self) -> Vec<GitHubRateLimitStatus> {
        Vec::new()
    }

    async fn list_repository_pull_request_page(
        &self,
        _repository: &RepoId,
        _filter: PullRequestListFilter,
        _cursor: Option<PullRequestPageCursor>,
        _page_size: usize,
    ) -> Result<PullRequestPage> {
        self.record_call("list_repository_pull_requests");
        pop_result(&self.pull_request_pages, "list_repository_pull_requests")
    }

    async fn count_repository_pull_requests(
        &self,
        _repository: &RepoId,
        _filter: PullRequestListFilter,
    ) -> Result<usize> {
        self.record_call("count_repository_pull_requests");
        pop_result(&self.pull_request_counts, "count_repository_pull_requests")
    }

    async fn list_repository_pull_requests_light_page(
        &self,
        _repository: &RepoId,
        _filter: PullRequestListFilter,
        cursor: Option<PullRequestPageCursor>,
        page_size: usize,
        validator: Option<HttpCacheValidator>,
    ) -> Result<ConditionalFetch<PullRequestPage>> {
        self.record_call("list_repository_pull_requests_light");
        self.light_pull_request_requests
            .lock()
            .expect("fake GitHub API request mutex should not be poisoned")
            .push((cursor, page_size, validator.is_some()));
        pop_result(
            &self.light_pull_request_pages,
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

    async fn list_pull_request_metadata_options(
        &self,
        _owner: &str,
        _repo: &str,
    ) -> Result<PullRequestMetadataOptions> {
        self.record_call("list_pull_request_metadata_options");
        pop_result(&self.metadata_options, "list_pull_request_metadata_options")
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

    async fn list_pull_request_commits(
        &self,
        _owner: &str,
        _repo: &str,
        _number: u64,
    ) -> Result<Vec<PullRequestCommit>> {
        self.record_call("list_pull_request_commits");
        pop_result(&self.commits, "list_pull_request_commits")
    }

    async fn mark_pull_request_file_viewed(
        &self,
        _pull_request_node_id: &str,
        _path: &str,
    ) -> Result<()> {
        self.record_call("mark_pull_request_file_viewed");
        pop_result(
            &self.mark_file_viewed_results,
            "mark_pull_request_file_viewed",
        )
    }

    async fn unmark_pull_request_file_viewed(
        &self,
        _pull_request_node_id: &str,
        _path: &str,
    ) -> Result<()> {
        self.record_call("unmark_pull_request_file_viewed");
        pop_result(
            &self.unmark_file_viewed_results,
            "unmark_pull_request_file_viewed",
        )
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
    async fn list_workflows(&self, _owner: &str, _repo: &str) -> Result<Vec<Workflow>> {
        self.record_call("list_workflows");
        pop_result(&self.workflows, "list_workflows")
    }

    async fn list_repository_workflow_runs(
        &self,
        _owner: &str,
        _repo: &str,
    ) -> Result<Vec<WorkflowRun>> {
        self.record_call("list_repository_workflow_runs");
        pop_result(
            &self.repository_workflow_runs,
            "list_repository_workflow_runs",
        )
    }

    async fn list_workflow_runs_for_workflow(
        &self,
        _owner: &str,
        _repo: &str,
        _workflow_id: u64,
    ) -> Result<Vec<WorkflowRun>> {
        self.record_call("list_workflow_runs_for_workflow");
        pop_result(
            &self.workflow_runs_for_workflow,
            "list_workflow_runs_for_workflow",
        )
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

    async fn workflow_run_log(&self, _owner: &str, _repo: &str, _run_id: u64) -> Result<String> {
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

    async fn list_pull_request_comments(
        &self,
        _owner: &str,
        _repo: &str,
        _number: u64,
    ) -> Result<Vec<PullRequestComment>> {
        self.record_call("list_pull_request_comments");
        pop_result(&self.pull_request_comments, "list_pull_request_comments")
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
