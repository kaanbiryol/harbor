use std::collections::HashSet;

use gpui::{Context, ScrollStrategy};
use harbor_domain::{
    CheckRun, DiffFile, PullRequest, PullRequestReview, RepoId, ReviewThread, WorkflowJob,
    WorkflowRun,
};
use harbor_logs::LogChunk;

use super::state::PullRequestDetailLoadedState;
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
    detail_loaded: PullRequestDetailLoadedState,
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
    detail_loaded: PullRequestDetailLoadedState,
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

impl PullRequestInboxSnapshot {
    pub(crate) fn pull_request_count(&self) -> usize {
        self.pull_requests.len()
    }
}

impl AppView {
    pub(crate) fn current_pull_request_inbox_key(&self) -> Option<PullRequestInboxCacheKey> {
        self.repository_state
            .configured_repo
            .clone()
            .map(|repository| {
                PullRequestInboxCacheKey::new(repository, self.pull_request_inbox.mode)
            })
    }

    pub(crate) fn cache_current_pull_request_inbox_snapshot(&mut self) {
        let Some(key) = self.current_pull_request_inbox_key() else {
            return;
        };

        if self.is_loading_prs
            || self.detail_state.is_any_loading()
            || self.review_state.reviews_loading()
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

        if self.detail_state.is_any_loading()
            || self.review_state.reviews_loading()
            || self.detail_state.has_cache_blocking_error()
        {
            return;
        }

        if let Some(snapshot) = self.current_pull_request_detail_snapshot() {
            self.detail_state
                .pull_request_detail_cache
                .insert(key, snapshot);
            self.cache_current_pull_request_inbox_snapshot();
        }
    }

    fn current_pull_request_detail_snapshot(&self) -> Option<PullRequestDetailSnapshot> {
        let pull_request = self.selected_pull_request().cloned()?;

        Some(PullRequestDetailSnapshot {
            pull_request,
            files: self.detail_state.files.clone(),
            diffs: self.detail_state.diffs.clone(),
            check_runs: self.detail_state.check_runs.clone(),
            workflow_runs: self.detail_state.workflow_runs.clone(),
            workflow_jobs: self.detail_state.workflow_jobs.clone(),
            pull_request_reviews: self.review_state.pull_request_reviews.clone(),
            review_threads: self.review_state.review_threads.clone(),
            detail_loaded: self
                .detail_state
                .loaded_sections(self.review_state.reviews_finished()),
            pending_review: self.review_state.pending_review.clone(),
            log_chunk: self.detail_state.log_state.chunk.clone(),
            current_user_login: self.review_state.current_user_login.clone(),
            collapsed_file_tree_folders: self.collapsed_file_tree_folders.clone(),
            reviewed_file_paths: self.reviewed_file_paths.clone(),
            excluded_file_type_filters: self.excluded_file_type_filters.clone(),
            show_files_owned_by_current_user: self.show_files_owned_by_current_user,
            owned_file_paths: self.owned_file_paths.clone(),
            active_file: self.diff_selection.file_index,
            active_hunk: self.diff_selection.hunk_index,
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
        let Some(snapshot) = self
            .detail_state
            .pull_request_detail_cache
            .get(&key)
            .cloned()
        else {
            return false;
        };

        self.tasks.clear_pull_request_detail_tasks();
        self.set_detail_loading(false);
        self.set_log_loading(false);
        self.clear_detail_errors();
        self.clear_log_error();
        self.clear_action_errors();
        self.clear_review_submission_errors();
        self.clear_review_composer_state();

        self.replace_selected_pull_request_preserving_row_fields(snapshot.pull_request);
        self.detail_state.files = snapshot.files;
        self.detail_state.diffs = snapshot.diffs;
        self.detail_state.check_runs = snapshot.check_runs;
        self.detail_state.workflow_runs = snapshot.workflow_runs;
        self.detail_state.workflow_jobs = snapshot.workflow_jobs;
        self.review_state.pull_request_reviews = snapshot.pull_request_reviews;
        self.review_state.review_threads = snapshot.review_threads;
        self.detail_state
            .restore_loaded_sections(snapshot.detail_loaded);
        if snapshot.detail_loaded.reviews {
            self.review_state.apply_reviews_success();
        } else {
            self.review_state.reset_reviews_load();
        }
        self.review_state.pending_review = snapshot.pending_review;
        self.detail_state.log_state.chunk = snapshot.log_chunk;
        self.review_state.current_user_login = snapshot.current_user_login;
        self.collapsed_file_tree_folders = snapshot.collapsed_file_tree_folders;
        self.reviewed_file_paths = snapshot.reviewed_file_paths;
        self.excluded_file_type_filters = snapshot.excluded_file_type_filters;
        self.show_files_owned_by_current_user = snapshot.show_files_owned_by_current_user;
        self.owned_file_paths = snapshot.owned_file_paths;
        self.diff_selection.file_index = snapshot
            .active_file
            .min(self.detail_state.files.len().saturating_sub(1));
        self.diff_selection.hunk_index = snapshot.active_hunk;
        self.active_tab = snapshot.active_tab;

        self.pr_list_scroll
            .scroll_to_item(self.selected_pr, ScrollStrategy::Center);
        self.reset_detail_scrolls();
        self.status = format!("Showing cached PR #{} details", key.number);
        self.load_active_panel_data_if_needed(cx);
        cx.notify();
        true
    }

    fn current_pull_request_inbox_snapshot(&self) -> PullRequestInboxSnapshot {
        PullRequestInboxSnapshot {
            pull_requests: self.pull_requests.clone(),
            files: self.detail_state.files.clone(),
            diffs: self.detail_state.diffs.clone(),
            check_runs: self.detail_state.check_runs.clone(),
            workflow_runs: self.detail_state.workflow_runs.clone(),
            workflow_jobs: self.detail_state.workflow_jobs.clone(),
            pull_request_reviews: self.review_state.pull_request_reviews.clone(),
            review_threads: self.review_state.review_threads.clone(),
            detail_loaded: self
                .detail_state
                .loaded_sections(self.review_state.reviews_finished()),
            pending_review: self.review_state.pending_review.clone(),
            log_chunk: self.detail_state.log_state.chunk.clone(),
            current_user_login: self.review_state.current_user_login.clone(),
            collapsed_file_tree_folders: self.collapsed_file_tree_folders.clone(),
            reviewed_file_paths: self.reviewed_file_paths.clone(),
            excluded_file_type_filters: self.excluded_file_type_filters.clone(),
            show_files_owned_by_current_user: self.show_files_owned_by_current_user,
            owned_file_paths: self.owned_file_paths.clone(),
            selected_pr: self.selected_pr,
            active_file: self.diff_selection.file_index,
            active_hunk: self.diff_selection.hunk_index,
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

        self.repository_state
            .select_repository(key.repository.clone());
        self.pull_request_inbox.mode = key.mode;
        self.tasks.clear_pull_request_list_task();
        self.tasks.clear_pull_request_detail_tasks();
        self.is_loading_prs = false;
        self.set_detail_loading(false);
        self.set_log_loading(false);
        self.load_error = None;
        self.clear_detail_errors();
        self.clear_log_error();
        self.clear_action_errors();
        self.clear_review_submission_errors();
        self.clear_review_composer_state();

        self.pull_requests = snapshot.pull_requests;
        self.detail_state.files = snapshot.files;
        self.detail_state.diffs = snapshot.diffs;
        self.detail_state.check_runs = snapshot.check_runs;
        self.detail_state.workflow_runs = snapshot.workflow_runs;
        self.detail_state.workflow_jobs = snapshot.workflow_jobs;
        self.review_state.pull_request_reviews = snapshot.pull_request_reviews;
        self.review_state.review_threads = snapshot.review_threads;
        self.detail_state
            .restore_loaded_sections(snapshot.detail_loaded);
        if snapshot.detail_loaded.reviews {
            self.review_state.apply_reviews_success();
        } else {
            self.review_state.reset_reviews_load();
        }
        self.review_state.pending_review = snapshot.pending_review;
        self.detail_state.log_state.chunk = snapshot.log_chunk;
        self.review_state.current_user_login = snapshot.current_user_login;
        self.collapsed_file_tree_folders = snapshot.collapsed_file_tree_folders;
        self.reviewed_file_paths = snapshot.reviewed_file_paths;
        self.excluded_file_type_filters = snapshot.excluded_file_type_filters;
        self.show_files_owned_by_current_user = snapshot.show_files_owned_by_current_user;
        self.owned_file_paths = snapshot.owned_file_paths;
        self.selected_pr = snapshot
            .selected_pr
            .min(self.pull_requests.len().saturating_sub(1));
        self.diff_selection.file_index = snapshot
            .active_file
            .min(self.detail_state.files.len().saturating_sub(1));
        self.diff_selection.hunk_index = snapshot.active_hunk;
        self.active_tab = snapshot.active_tab;

        self.pr_list_scroll
            .scroll_to_item(self.selected_pr, ScrollStrategy::Center);
        self.reset_detail_scrolls();
        self.status = format!(
            "Showing cached {} from {}",
            key.mode.status_label(),
            key.repository.full_name()
        );
        self.load_active_panel_data_if_needed(cx);
        cx.notify();
        true
    }
}
