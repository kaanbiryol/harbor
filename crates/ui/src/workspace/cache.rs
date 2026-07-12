use std::{collections::HashSet, sync::Arc};

use gpui::{Context, ScrollStrategy};
use harbor_domain::{
    CheckRun, DiffFile, PullRequest, PullRequestComment, PullRequestCommit, PullRequestReview,
    RepoId, ReviewThread, WorkflowJob, WorkflowRun,
};
use harbor_logs::LogChunk;
use harbor_sync::PullRequestInboxPageInfo;

use super::state::PullRequestDetailLoadedState;
use crate::{
    actions::PanelTab,
    diff::ParsedDiff,
    workspace::{AppView, PendingReviewSession, PullRequestInboxMode},
};

#[cfg(test)]
mod tests;

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub(crate) struct PullRequestInboxCacheKey {
    repository: RepoId,
    mode: PullRequestInboxMode,
}

impl PullRequestInboxCacheKey {
    pub(crate) fn new(repository: RepoId, mode: PullRequestInboxMode) -> Self {
        Self { repository, mode }
    }

    pub(crate) fn repository(&self) -> &RepoId {
        &self.repository
    }

    pub(crate) fn mode(&self) -> PullRequestInboxMode {
        self.mode
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
    commits: Vec<PullRequestCommit>,
    workflow_runs: Vec<WorkflowRun>,
    workflow_jobs: Vec<WorkflowJob>,
    pull_request_reviews: Vec<PullRequestReview>,
    pull_request_comments: Vec<PullRequestComment>,
    pub(super) review_threads: Vec<ReviewThread>,
    detail_loaded: PullRequestDetailLoadedState,
    pub(super) pending_review: Option<PendingReviewSession>,
    log_chunk: Option<LogChunk>,
    current_user_login: Option<String>,
    collapsed_file_tree_folders: HashSet<String>,
    collapsed_check_groups: HashSet<String>,
    expanded_diff_file_paths: HashSet<String>,
    collapsed_diff_file_paths: HashSet<String>,
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
    page_info: PullRequestInboxPageInfo,
    detail: Option<Arc<PullRequestDetailSnapshot>>,
    selected_pr: usize,
}

impl PullRequestInboxSnapshot {
    pub(crate) fn count(&self) -> Option<usize> {
        self.page_info
            .total_count
            .or_else(|| (!self.page_info.has_next_page()).then_some(self.pull_requests.len()))
    }
}

impl AppView {
    pub(crate) fn current_pull_request_inbox_key(&self) -> Option<PullRequestInboxCacheKey> {
        self.repository_state
            .configured_repo_cloned()
            .map(|repository| {
                PullRequestInboxCacheKey::new(repository, self.pull_request_inbox.mode())
            })
    }

    pub(crate) fn cache_current_pull_request_inbox_snapshot(&mut self) {
        let Some(key) = self.current_pull_request_inbox_key() else {
            return;
        };

        if !self.pull_request_inbox.can_cache_snapshot()
            || self.detail_state.is_any_loading()
            || self.review_state.reviews_loading()
        {
            return;
        }

        let detail = self
            .current_pull_request_detail_snapshot_for_inbox()
            .map(Arc::new);
        self.cache_current_pull_request_inbox_snapshot_with_detail(key, detail);
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

        if let Some(snapshot) = self.current_pull_request_detail_snapshot().map(Arc::new) {
            self.detail_state.cache_snapshot(key, snapshot.clone());
            if let Some(inbox_key) = self.current_pull_request_inbox_key() {
                self.cache_current_pull_request_inbox_snapshot_with_detail(
                    inbox_key,
                    Some(snapshot),
                );
            }
        }
    }

    fn current_pull_request_detail_snapshot(&self) -> Option<PullRequestDetailSnapshot> {
        let pull_request = self.selected_pull_request().cloned()?;

        Some(self.pull_request_detail_snapshot(pull_request))
    }

    fn current_pull_request_detail_snapshot_for_inbox(&self) -> Option<PullRequestDetailSnapshot> {
        let last_index = self.pull_requests.len().checked_sub(1)?;
        let pull_request = self
            .pull_requests
            .get(self.selected_pull_request_index().min(last_index))?
            .clone();

        Some(self.pull_request_detail_snapshot(pull_request))
    }

    fn pull_request_detail_snapshot(&self, pull_request: PullRequest) -> PullRequestDetailSnapshot {
        PullRequestDetailSnapshot {
            pull_request,
            files: self.detail_state.files().to_vec(),
            diffs: self.detail_state.diffs().to_vec(),
            check_runs: self.detail_state.check_runs().to_vec(),
            commits: self.detail_state.commits().to_vec(),
            workflow_runs: self.detail_state.workflow_runs().to_vec(),
            workflow_jobs: self.detail_state.workflow_jobs().to_vec(),
            pull_request_reviews: self.review_state.pull_request_reviews().to_vec(),
            pull_request_comments: self.review_state.pull_request_comments().to_vec(),
            review_threads: self.review_state.review_threads().to_vec(),
            detail_loaded: self
                .detail_state
                .loaded_sections(self.review_state.reviews_finished()),
            pending_review: self.review_state.pending_review_cloned(),
            log_chunk: self.detail_state.log_state.chunk().cloned(),
            current_user_login: self.review_state.current_user_login().map(str::to_string),
            collapsed_file_tree_folders: self.collapsed_file_tree_folders.clone(),
            collapsed_check_groups: self.checks_state.collapsed_groups.clone(),
            expanded_diff_file_paths: self.expanded_diff_file_paths.clone(),
            collapsed_diff_file_paths: self.collapsed_diff_file_paths.clone(),
            reviewed_file_paths: self.reviewed_file_paths.clone(),
            excluded_file_type_filters: self.excluded_file_type_filters.clone(),
            show_files_owned_by_current_user: self.show_files_owned_by_current_user,
            owned_file_paths: self.owned_file_paths.clone(),
            active_file: self.active_file_index(),
            active_hunk: self.active_hunk_index(),
            active_tab: self.active_tab,
        }
    }

    pub(crate) fn restore_selected_pull_request_detail_snapshot(
        &mut self,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some(key) = self.selected_pull_request_detail_key() else {
            return false;
        };
        let Some(snapshot) = self.detail_state.snapshot(&key).cloned() else {
            return false;
        };

        self.prepare_detail_snapshot_restore();
        self.apply_detail_snapshot(&snapshot, cx);

        self.scroll_selected_pull_request_into_view(ScrollStrategy::Center);
        self.reset_detail_scrolls();
        self.status = format!("Showing cached PR #{} details", key.number);
        self.load_active_panel_data_if_needed(cx);
        cx.notify();
        true
    }

    fn cache_current_pull_request_inbox_snapshot_with_detail(
        &mut self,
        key: PullRequestInboxCacheKey,
        detail: Option<Arc<PullRequestDetailSnapshot>>,
    ) {
        if !self.pull_request_inbox.can_cache_snapshot()
            || self.detail_state.is_any_loading()
            || self.review_state.reviews_loading()
        {
            return;
        }

        let snapshot = self.current_pull_request_inbox_snapshot(detail);
        self.pull_request_inbox.insert_snapshot(key, snapshot);
    }

    fn current_pull_request_inbox_snapshot(
        &self,
        detail: Option<Arc<PullRequestDetailSnapshot>>,
    ) -> PullRequestInboxSnapshot {
        PullRequestInboxSnapshot {
            pull_requests: self.pull_requests.clone(),
            page_info: self.pull_request_inbox.page_info().clone(),
            detail,
            selected_pr: self.selected_pull_request_index(),
        }
    }

    fn prepare_detail_snapshot_restore(&mut self) {
        self.tasks.cancel_selected_pull_request_tasks();
        self.set_detail_loading(false);
        self.set_log_loading(false);
        self.clear_detail_errors();
        self.clear_log_error();
        self.clear_action_errors();
        self.clear_review_submission_errors();
        self.clear_review_composer_state();
    }

    fn apply_detail_snapshot(
        &mut self,
        snapshot: &PullRequestDetailSnapshot,
        cx: &mut Context<Self>,
    ) {
        self.replace_selected_pull_request_preserving_row_fields(snapshot.pull_request.clone());
        self.detail_state
            .replace_diff_files(snapshot.files.clone(), snapshot.diffs.clone());
        self.detail_state
            .replace_check_runs(snapshot.check_runs.clone());
        self.detail_state.replace_commits(snapshot.commits.clone());
        self.detail_state
            .replace_workflow_runs(snapshot.workflow_runs.clone());
        self.detail_state
            .replace_workflow_jobs(snapshot.workflow_jobs.clone());
        self.detail_state
            .restore_loaded_sections(snapshot.detail_loaded);
        self.review_state.restore_review_snapshot(
            snapshot.pull_request_reviews.clone(),
            snapshot.pull_request_comments.clone(),
            snapshot.review_threads.clone(),
            snapshot.pending_review.clone(),
            snapshot.current_user_login.clone(),
            snapshot.detail_loaded.reviews,
        );
        self.detail_state
            .log_state
            .set_chunk(snapshot.log_chunk.clone());
        self.collapsed_file_tree_folders = snapshot.collapsed_file_tree_folders.clone();
        self.checks_state.collapsed_groups = snapshot.collapsed_check_groups.clone();
        self.expanded_diff_file_paths = snapshot.expanded_diff_file_paths.clone();
        self.collapsed_diff_file_paths = snapshot.collapsed_diff_file_paths.clone();
        self.reviewed_file_paths = snapshot.reviewed_file_paths.clone();
        self.excluded_file_type_filters = snapshot.excluded_file_type_filters.clone();
        self.show_files_owned_by_current_user = snapshot.show_files_owned_by_current_user;
        self.owned_file_paths = snapshot.owned_file_paths.clone();
        self.selection_state.restore_diff_position(
            snapshot.active_file,
            snapshot.active_hunk,
            self.detail_state.files().len(),
        );
        self.active_tab = snapshot.active_tab;
        self.sync_diff_list_items(cx);
    }

    pub(crate) fn restore_pull_request_inbox_snapshot(
        &mut self,
        key: PullRequestInboxCacheKey,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some(snapshot) = self.pull_request_inbox.snapshot(&key).cloned() else {
            return false;
        };

        let PullRequestInboxSnapshot {
            pull_requests,
            page_info,
            detail,
            selected_pr,
        } = snapshot;

        self.repository_state
            .select_repository(key.repository.clone());
        self.pull_request_inbox.set_mode(key.mode);
        self.pull_request_inbox.set_page_info(page_info);
        self.tasks.cancel_pull_request_list_task();
        self.pull_request_inbox.reset_load();
        self.prepare_detail_snapshot_restore();

        self.pull_requests = pull_requests;
        self.selection_state
            .restore_pull_request_index(selected_pr, self.pull_requests.len());
        if let Some(detail) = detail {
            self.apply_detail_snapshot(&detail, cx);
        } else {
            self.clear_selected_pull_request_detail_state();
            self.active_tab = PanelTab::Overview;
            self.sync_diff_list_items(cx);
        }

        self.scroll_selected_pull_request_into_view(ScrollStrategy::Center);
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
