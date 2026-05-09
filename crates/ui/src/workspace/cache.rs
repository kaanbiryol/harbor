use std::collections::HashSet;

use gpui::{Context, ScrollStrategy};
use harbor_domain::{
    CheckRun, DiffFile, PullRequest, PullRequestReview, RepoId, ReviewThread, WorkflowJob,
    WorkflowRun,
};
use harbor_logs::LogChunk;

use crate::{
    actions::PanelTab,
    diff::ParsedDiff,
    workspace::{AppView, PendingReviewSession, PullRequestInboxMode},
};

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub(crate) struct PullRequestInboxCacheKey {
    repository: RepoId,
    mode: PullRequestInboxMode,
}

impl PullRequestInboxCacheKey {
    pub(crate) fn new(repository: RepoId, mode: PullRequestInboxMode) -> Self {
        Self { repository, mode }
    }
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub(crate) struct PullRequestDetailCacheKey {
    repository: RepoId,
    number: u64,
    head_sha: String,
}

impl PullRequestDetailCacheKey {
    pub(crate) fn new(repository: RepoId, number: u64, head_sha: String) -> Self {
        Self {
            repository,
            number,
            head_sha,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PullRequestDetailSnapshot {
    pub(super) pull_request: PullRequest,
    files: Vec<DiffFile>,
    diffs: Vec<Option<ParsedDiff>>,
    check_runs: Vec<CheckRun>,
    workflow_runs: Vec<WorkflowRun>,
    workflow_jobs: Vec<WorkflowJob>,
    pull_request_reviews: Vec<PullRequestReview>,
    pub(super) review_threads: Vec<ReviewThread>,
    pub(super) pending_review: Option<PendingReviewSession>,
    log_chunk: Option<LogChunk>,
    current_user_login: Option<String>,
    collapsed_file_tree_folders: HashSet<String>,
    reviewed_file_paths: HashSet<String>,
    excluded_file_type_filters: HashSet<String>,
    show_files_owned_by_current_user: bool,
    owned_file_paths: HashSet<String>,
    active_file: usize,
    active_hunk: usize,
    active_tab: PanelTab,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PullRequestInboxSnapshot {
    pull_requests: Vec<PullRequest>,
    files: Vec<DiffFile>,
    diffs: Vec<Option<ParsedDiff>>,
    check_runs: Vec<CheckRun>,
    workflow_runs: Vec<WorkflowRun>,
    workflow_jobs: Vec<WorkflowJob>,
    pull_request_reviews: Vec<PullRequestReview>,
    review_threads: Vec<ReviewThread>,
    pending_review: Option<PendingReviewSession>,
    log_chunk: Option<LogChunk>,
    current_user_login: Option<String>,
    collapsed_file_tree_folders: HashSet<String>,
    reviewed_file_paths: HashSet<String>,
    excluded_file_type_filters: HashSet<String>,
    show_files_owned_by_current_user: bool,
    owned_file_paths: HashSet<String>,
    selected_pr: usize,
    active_file: usize,
    active_hunk: usize,
    active_tab: PanelTab,
}

impl AppView {
    pub(crate) fn current_pull_request_inbox_key(&self) -> Option<PullRequestInboxCacheKey> {
        self.configured_repo.clone().map(|repository| {
            PullRequestInboxCacheKey::new(repository, self.pull_request_inbox.mode)
        })
    }

    pub(crate) fn cache_current_pull_request_inbox_snapshot(&mut self) {
        let Some(key) = self.current_pull_request_inbox_key() else {
            return;
        };

        if self.is_loading_prs
            || self.is_loading_details
            || self.is_loading_files
            || self.is_loading_checks
            || self.is_loading_workflows
            || self.is_loading_reviews
            || self.load_error.is_some()
        {
            return;
        }

        self.pull_request_inbox
            .cache
            .insert(key, self.current_pull_request_inbox_snapshot());
    }

    pub(crate) fn selected_pull_request_detail_key(&self) -> Option<PullRequestDetailCacheKey> {
        self.selected_pull_request().map(|pull_request| {
            PullRequestDetailCacheKey::new(
                pull_request.repo.clone(),
                pull_request.number,
                pull_request.head_sha.clone(),
            )
        })
    }

    pub(crate) fn cache_current_pull_request_detail_snapshot(&mut self) {
        let Some(key) = self.selected_pull_request_detail_key() else {
            return;
        };

        if self.is_loading_details
            || self.is_loading_files
            || self.is_loading_checks
            || self.is_loading_workflows
            || self.is_loading_reviews
            || self.details_error.is_some()
            || self.files_error.is_some()
        {
            return;
        }

        if let Some(snapshot) = self.current_pull_request_detail_snapshot() {
            self.pull_request_detail_cache.insert(key, snapshot);
            self.cache_current_pull_request_inbox_snapshot();
        }
    }

    fn current_pull_request_detail_snapshot(&self) -> Option<PullRequestDetailSnapshot> {
        let pull_request = self.selected_pull_request().cloned()?;

        Some(PullRequestDetailSnapshot {
            pull_request,
            files: self.files.clone(),
            diffs: self.diffs.clone(),
            check_runs: self.check_runs.clone(),
            workflow_runs: self.workflow_runs.clone(),
            workflow_jobs: self.workflow_jobs.clone(),
            pull_request_reviews: self.pull_request_reviews.clone(),
            review_threads: self.review_threads.clone(),
            pending_review: self.pending_review.clone(),
            log_chunk: self.log_chunk.clone(),
            current_user_login: self.current_user_login.clone(),
            collapsed_file_tree_folders: self.collapsed_file_tree_folders.clone(),
            reviewed_file_paths: self.reviewed_file_paths.clone(),
            excluded_file_type_filters: self.excluded_file_type_filters.clone(),
            show_files_owned_by_current_user: self.show_files_owned_by_current_user,
            owned_file_paths: self.owned_file_paths.clone(),
            active_file: self.active_file,
            active_hunk: self.active_hunk,
            active_tab: self.active_tab,
        })
    }

    pub(crate) fn restore_selected_pull_request_detail_snapshot(
        &mut self,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some(key) = self.selected_pull_request_detail_key() else {
            return false;
        };
        let Some(snapshot) = self.pull_request_detail_cache.get(&key).cloned() else {
            return false;
        };

        self.pr_detail_tasks.clear();
        self.is_loading_details = false;
        self.is_loading_files = false;
        self.is_loading_checks = false;
        self.is_loading_workflows = false;
        self.is_loading_reviews = false;
        self.is_loading_logs = false;
        self.details_error = None;
        self.files_error = None;
        self.checks_error = None;
        self.workflows_error = None;
        self.reviews_error = None;
        self.logs_error = None;
        self.action_error = None;
        self.pr_action_error = None;
        self.review_comment_error = None;
        self.pending_review_error = None;
        self.clear_review_composer_state();

        if let Some(selected) = self.pull_requests.get_mut(self.selected_pr) {
            *selected = snapshot.pull_request;
        }
        self.files = snapshot.files;
        self.diffs = snapshot.diffs;
        self.check_runs = snapshot.check_runs;
        self.workflow_runs = snapshot.workflow_runs;
        self.workflow_jobs = snapshot.workflow_jobs;
        self.pull_request_reviews = snapshot.pull_request_reviews;
        self.review_threads = snapshot.review_threads;
        self.pending_review = snapshot.pending_review;
        self.log_chunk = snapshot.log_chunk;
        self.current_user_login = snapshot.current_user_login;
        self.collapsed_file_tree_folders = snapshot.collapsed_file_tree_folders;
        self.reviewed_file_paths = snapshot.reviewed_file_paths;
        self.excluded_file_type_filters = snapshot.excluded_file_type_filters;
        self.show_files_owned_by_current_user = snapshot.show_files_owned_by_current_user;
        self.owned_file_paths = snapshot.owned_file_paths;
        self.active_file = snapshot.active_file.min(self.files.len().saturating_sub(1));
        self.active_hunk = snapshot.active_hunk;
        self.active_tab = snapshot.active_tab;

        self.pr_list_scroll
            .scroll_to_item(self.selected_pr, ScrollStrategy::Center);
        self.file_list_scroll.scroll_to_item(0, ScrollStrategy::Top);
        self.diff_list_scroll.scroll_to_item(0, ScrollStrategy::Top);
        self.review_list_scroll
            .scroll_to_item(0, ScrollStrategy::Top);
        self.log_list_scroll.scroll_to_item(0, ScrollStrategy::Top);
        self.status = format!("Showing cached PR #{} details", key.number);
        cx.notify();
        true
    }

    fn current_pull_request_inbox_snapshot(&self) -> PullRequestInboxSnapshot {
        PullRequestInboxSnapshot {
            pull_requests: self.pull_requests.clone(),
            files: self.files.clone(),
            diffs: self.diffs.clone(),
            check_runs: self.check_runs.clone(),
            workflow_runs: self.workflow_runs.clone(),
            workflow_jobs: self.workflow_jobs.clone(),
            pull_request_reviews: self.pull_request_reviews.clone(),
            review_threads: self.review_threads.clone(),
            pending_review: self.pending_review.clone(),
            log_chunk: self.log_chunk.clone(),
            current_user_login: self.current_user_login.clone(),
            collapsed_file_tree_folders: self.collapsed_file_tree_folders.clone(),
            reviewed_file_paths: self.reviewed_file_paths.clone(),
            excluded_file_type_filters: self.excluded_file_type_filters.clone(),
            show_files_owned_by_current_user: self.show_files_owned_by_current_user,
            owned_file_paths: self.owned_file_paths.clone(),
            selected_pr: self.selected_pr,
            active_file: self.active_file,
            active_hunk: self.active_hunk,
            active_tab: self.active_tab,
        }
    }

    pub(crate) fn restore_pull_request_inbox_snapshot(
        &mut self,
        key: PullRequestInboxCacheKey,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some(snapshot) = self.pull_request_inbox.cache.get(&key).cloned() else {
            return false;
        };

        self.configured_repo = Some(key.repository.clone());
        self.pull_request_inbox.mode = key.mode;
        self.pr_list_task = None;
        self.pr_detail_tasks.clear();
        self.is_loading_prs = false;
        self.is_loading_details = false;
        self.is_loading_files = false;
        self.is_loading_checks = false;
        self.is_loading_workflows = false;
        self.is_loading_reviews = false;
        self.is_loading_logs = false;
        self.load_error = None;
        self.details_error = None;
        self.files_error = None;
        self.checks_error = None;
        self.workflows_error = None;
        self.reviews_error = None;
        self.logs_error = None;
        self.action_error = None;
        self.pr_action_error = None;
        self.review_comment_error = None;
        self.pending_review_error = None;
        self.clear_review_composer_state();

        self.pull_requests = snapshot.pull_requests;
        self.files = snapshot.files;
        self.diffs = snapshot.diffs;
        self.check_runs = snapshot.check_runs;
        self.workflow_runs = snapshot.workflow_runs;
        self.workflow_jobs = snapshot.workflow_jobs;
        self.pull_request_reviews = snapshot.pull_request_reviews;
        self.review_threads = snapshot.review_threads;
        self.pending_review = snapshot.pending_review;
        self.log_chunk = snapshot.log_chunk;
        self.current_user_login = snapshot.current_user_login;
        self.collapsed_file_tree_folders = snapshot.collapsed_file_tree_folders;
        self.reviewed_file_paths = snapshot.reviewed_file_paths;
        self.excluded_file_type_filters = snapshot.excluded_file_type_filters;
        self.show_files_owned_by_current_user = snapshot.show_files_owned_by_current_user;
        self.owned_file_paths = snapshot.owned_file_paths;
        self.selected_pr = snapshot
            .selected_pr
            .min(self.pull_requests.len().saturating_sub(1));
        self.active_file = snapshot.active_file.min(self.files.len().saturating_sub(1));
        self.active_hunk = snapshot.active_hunk;
        self.active_tab = snapshot.active_tab;

        self.pr_list_scroll
            .scroll_to_item(self.selected_pr, ScrollStrategy::Center);
        self.file_list_scroll.scroll_to_item(0, ScrollStrategy::Top);
        self.diff_list_scroll.scroll_to_item(0, ScrollStrategy::Top);
        self.review_list_scroll
            .scroll_to_item(0, ScrollStrategy::Top);
        self.log_list_scroll.scroll_to_item(0, ScrollStrategy::Top);
        self.status = format!(
            "Showing cached {} from {}",
            key.mode.status_label(),
            key.repository.full_name()
        );
        cx.notify();
        true
    }
}
