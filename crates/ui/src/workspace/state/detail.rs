use std::{
    collections::{HashMap, VecDeque},
    sync::Arc,
};

use harbor_domain::{
    CheckRun, DiffFile, FileViewedState, PullRequestCommit, WorkflowJob, WorkflowRun,
};

use crate::{
    diff::ParsedDiff,
    workspace::{
        PendingReviewSession, PullRequestDetailCacheKey, PullRequestDetailSnapshot,
        WorkflowLogState,
        reviews::{
            remove_review_comment_from_threads, rollback_pending_review_comment_count,
            unresolved_review_thread_count,
        },
        status::LoadStatus,
    },
};

pub(crate) struct PullRequestDetailUiState {
    files: Vec<DiffFile>,
    diffs: Vec<Option<ParsedDiff>>,
    check_runs: Vec<CheckRun>,
    commits: Vec<PullRequestCommit>,
    workflow_runs: Vec<WorkflowRun>,
    workflow_jobs: Vec<WorkflowJob>,
    pull_request_detail_cache: HashMap<PullRequestDetailCacheKey, Arc<PullRequestDetailSnapshot>>,
    pull_request_detail_cache_order: VecDeque<PullRequestDetailCacheKey>,
    details_load: LoadStatus,
    files_load: LoadStatus,
    checks_load: LoadStatus,
    commits_load: LoadStatus,
    workflows_load: LoadStatus,
    pub(crate) log_state: WorkflowLogState,
}

const MAX_PULL_REQUEST_DETAIL_SNAPSHOTS: usize = 8;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct PullRequestDetailLoadedState {
    pub(crate) details: bool,
    pub(crate) files: bool,
    pub(crate) checks: bool,
    pub(crate) commits: bool,
    pub(crate) workflows: bool,
    pub(crate) reviews: bool,
}

impl PullRequestDetailUiState {
    pub(crate) fn new(
        files: Vec<DiffFile>,
        diffs: Vec<Option<ParsedDiff>>,
        log_state: WorkflowLogState,
    ) -> Self {
        Self {
            files,
            diffs,
            check_runs: Vec::new(),
            commits: Vec::new(),
            workflow_runs: Vec::new(),
            workflow_jobs: Vec::new(),
            pull_request_detail_cache: HashMap::new(),
            pull_request_detail_cache_order: VecDeque::new(),
            details_load: LoadStatus::Idle,
            files_load: LoadStatus::Idle,
            checks_load: LoadStatus::Idle,
            commits_load: LoadStatus::Idle,
            workflows_load: LoadStatus::Idle,
            log_state,
        }
    }

    pub(crate) fn reset_for_selection(&mut self) {
        self.details_load.reset();
        self.files_load.reset();
        self.checks_load.reset();
        self.commits_load.reset();
        self.workflows_load.reset();
    }

    pub(crate) fn cache_snapshot(
        &mut self,
        key: PullRequestDetailCacheKey,
        snapshot: Arc<PullRequestDetailSnapshot>,
    ) {
        self.pull_request_detail_cache_order
            .retain(|existing| existing != &key);
        self.pull_request_detail_cache_order.push_back(key.clone());
        self.pull_request_detail_cache.insert(key, snapshot);

        while self.pull_request_detail_cache_order.len() > MAX_PULL_REQUEST_DETAIL_SNAPSHOTS {
            if let Some(expired) = self.pull_request_detail_cache_order.pop_front() {
                self.pull_request_detail_cache.remove(&expired);
            }
        }
    }

    pub(crate) fn snapshot(
        &self,
        key: &PullRequestDetailCacheKey,
    ) -> Option<&Arc<PullRequestDetailSnapshot>> {
        self.pull_request_detail_cache.get(key)
    }

    pub(crate) fn remove_optimistic_comment_from_snapshot(
        &mut self,
        key: &PullRequestDetailCacheKey,
        comment_id: &str,
    ) {
        if let Some(snapshot) = self.pull_request_detail_cache.get_mut(key) {
            let snapshot = Arc::make_mut(snapshot);
            remove_review_comment_from_threads(&mut snapshot.review_threads, comment_id);
            snapshot.pull_request.unresolved_threads =
                unresolved_review_thread_count(&snapshot.review_threads);
        }
    }

    pub(crate) fn rollback_pending_review_comment_count_in_snapshot(
        &mut self,
        key: &PullRequestDetailCacheKey,
        previous_pending_review: Option<&PendingReviewSession>,
    ) {
        if let Some(snapshot) = self.pull_request_detail_cache.get_mut(key) {
            let snapshot = Arc::make_mut(snapshot);
            rollback_pending_review_comment_count(
                &mut snapshot.pending_review,
                previous_pending_review,
            );
        }
    }

    pub(crate) fn set_pending_review_in_snapshot(
        &mut self,
        key: &PullRequestDetailCacheKey,
        pending_review: PendingReviewSession,
    ) {
        if let Some(snapshot) = self.pull_request_detail_cache.get_mut(key) {
            let snapshot = Arc::make_mut(snapshot);
            snapshot.pending_review = Some(pending_review);
        }
    }

    pub(crate) fn restore_loaded_sections(&mut self, sections: PullRequestDetailLoadedState) {
        self.details_load = load_status_from_loaded(sections.details);
        self.files_load = load_status_from_loaded(sections.files);
        self.checks_load = load_status_from_loaded(sections.checks);
        self.commits_load = load_status_from_loaded(sections.commits);
        self.workflows_load = load_status_from_loaded(sections.workflows);
    }

    pub(crate) fn replace_diff_files(
        &mut self,
        files: Vec<DiffFile>,
        diffs: Vec<Option<ParsedDiff>>,
    ) {
        self.files = files;
        self.diffs = diffs;
    }

    pub(crate) fn files(&self) -> &[DiffFile] {
        &self.files
    }

    pub(crate) fn set_file_viewed_state(&mut self, path: &str, viewed_state: FileViewedState) {
        if let Some(file) = self.files.iter_mut().find(|file| file.path == path) {
            file.viewed_state = viewed_state;
        }
    }

    pub(crate) fn diffs(&self) -> &[Option<ParsedDiff>] {
        &self.diffs
    }

    pub(crate) fn clear_diff_files(&mut self) {
        self.files.clear();
        self.diffs.clear();
    }

    pub(crate) fn replace_parsed_diff(&mut self, file_index: usize, diff: ParsedDiff) -> bool {
        let Some(slot) = self.diffs.get_mut(file_index) else {
            return false;
        };

        *slot = Some(diff);
        true
    }

    pub(crate) fn replace_check_runs(&mut self, check_runs: Vec<CheckRun>) {
        self.check_runs = check_runs;
    }

    pub(crate) fn check_runs(&self) -> &[CheckRun] {
        &self.check_runs
    }

    pub(crate) fn clear_check_runs(&mut self) {
        self.check_runs.clear();
    }

    pub(crate) fn replace_commits(&mut self, commits: Vec<PullRequestCommit>) {
        self.commits = commits;
    }
    pub(crate) fn commits(&self) -> &[PullRequestCommit] {
        &self.commits
    }
    pub(crate) fn clear_commits(&mut self) {
        self.commits.clear();
    }

    pub(crate) fn replace_workflow_runs(&mut self, workflow_runs: Vec<WorkflowRun>) {
        self.workflow_runs = workflow_runs;
    }

    pub(crate) fn workflow_runs(&self) -> &[WorkflowRun] {
        &self.workflow_runs
    }

    pub(crate) fn clear_workflow_runs(&mut self) {
        self.workflow_runs.clear();
    }

    pub(crate) fn replace_workflow_jobs(&mut self, workflow_jobs: Vec<WorkflowJob>) {
        self.workflow_jobs = workflow_jobs;
    }

    pub(crate) fn workflow_jobs(&self) -> &[WorkflowJob] {
        &self.workflow_jobs
    }

    pub(crate) fn clear_workflow_jobs(&mut self) {
        self.workflow_jobs.clear();
    }

    pub(crate) fn loaded_sections(&self, reviews_loaded: bool) -> PullRequestDetailLoadedState {
        PullRequestDetailLoadedState {
            details: self.details_load.is_finished(),
            files: self.files_load.is_finished(),
            checks: self.checks_load.is_finished(),
            commits: self.commits_load.is_finished(),
            workflows: self.workflows_load.is_finished(),
            reviews: reviews_loaded,
        }
    }

    pub(crate) fn is_any_loading(&self) -> bool {
        self.details_load.is_loading()
            || self.files_load.is_loading()
            || self.checks_load.is_loading()
            || self.commits_load.is_loading()
            || self.workflows_load.is_loading()
    }

    pub(crate) fn has_cache_blocking_error(&self) -> bool {
        self.details_error().is_some() || self.files_error().is_some()
    }

    pub(crate) fn clear_errors(&mut self) {
        self.details_load.clear_error();
        self.files_load.clear_error();
        self.checks_load.clear_error();
        self.commits_load.clear_error();
        self.workflows_load.clear_error();
    }

    pub(crate) fn should_load_details(&self) -> bool {
        !self.details_load.is_loading() && !self.details_load.is_finished()
    }

    pub(crate) fn should_load_files(&self) -> bool {
        !self.files_load.is_loading() && !self.files_load.is_finished()
    }

    pub(crate) fn should_load_checks(&self) -> bool {
        !self.checks_load.is_loading() && !self.checks_load.is_finished()
    }
    pub(crate) fn should_load_commits(&self) -> bool {
        !self.commits_load.is_loading() && !self.commits_load.is_finished()
    }

    pub(crate) fn should_load_workflows(&self) -> bool {
        !self.workflows_load.is_loading() && !self.workflows_load.is_finished()
    }

    pub(crate) fn mark_details_stale(&mut self) {
        self.details_load.reset();
    }

    pub(crate) fn mark_checks_stale(&mut self) {
        self.checks_load.reset();
    }

    pub(crate) fn mark_workflows_stale(&mut self) {
        self.workflows_load.reset();
    }

    pub(crate) fn start_details_load(&mut self) {
        self.details_load.start();
    }

    pub(crate) fn start_files_load(&mut self) {
        self.files_load.start();
    }

    pub(crate) fn start_checks_load(&mut self) {
        self.checks_load.start();
    }
    pub(crate) fn start_commits_load(&mut self) {
        self.commits_load.start();
    }

    pub(crate) fn start_workflows_load(&mut self) {
        self.workflows_load.start();
    }

    pub(crate) fn apply_details_success(&mut self) {
        self.details_load.succeed();
    }

    pub(crate) fn apply_files_success(&mut self) {
        self.files_load.succeed();
    }

    pub(crate) fn apply_checks_success(&mut self) {
        self.checks_load.succeed();
    }
    pub(crate) fn apply_commits_success(&mut self) {
        self.commits_load.succeed();
    }

    pub(crate) fn apply_workflows_success(&mut self) {
        self.workflows_load.succeed();
    }

    pub(crate) fn apply_details_failure(&mut self, error: impl Into<String>) {
        self.details_load.fail(error);
    }

    pub(crate) fn apply_files_failure(&mut self, error: impl Into<String>) {
        self.files_load.fail(error);
    }

    pub(crate) fn apply_checks_failure(&mut self, error: impl Into<String>) {
        self.checks_load.fail(error);
    }
    pub(crate) fn apply_commits_failure(&mut self, error: impl Into<String>) {
        self.commits_load.fail(error);
    }

    pub(crate) fn apply_workflows_failure(&mut self, error: impl Into<String>) {
        self.workflows_load.fail(error);
    }

    pub(crate) fn details_loading(&self) -> bool {
        self.details_load.is_loading()
    }

    pub(crate) fn details_loaded(&self) -> bool {
        self.details_load.is_loaded()
    }

    pub(crate) fn files_loading(&self) -> bool {
        self.files_load.is_loading()
    }

    pub(crate) fn checks_loading(&self) -> bool {
        self.checks_load.is_loading()
    }
    pub(crate) fn commits_loading(&self) -> bool {
        self.commits_load.is_loading()
    }

    pub(crate) fn workflows_loading(&self) -> bool {
        self.workflows_load.is_loading()
    }

    pub(crate) fn details_error(&self) -> Option<&str> {
        self.details_load.error()
    }

    pub(crate) fn files_error(&self) -> Option<&str> {
        self.files_load.error()
    }

    pub(crate) fn checks_error(&self) -> Option<&str> {
        self.checks_load.error()
    }
    pub(crate) fn commits_error(&self) -> Option<&str> {
        self.commits_load.error()
    }

    pub(crate) fn workflows_error(&self) -> Option<&str> {
        self.workflows_load.error()
    }
}

fn load_status_from_loaded(loaded: bool) -> LoadStatus {
    if loaded {
        LoadStatus::Loaded
    } else {
        LoadStatus::Idle
    }
}
