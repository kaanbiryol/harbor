use gpui::ScrollStrategy;

use super::state::PullRequestDetailLoadedState;
use crate::workspace::AppView;

impl AppView {
    pub(super) fn reset_diff_selection(&mut self) {
        self.diff_selection.file_index = 0;
        self.diff_selection.hunk_index = 0;
    }

    pub(super) fn select_diff_file_index(&mut self, file_index: usize) {
        self.diff_selection.file_index = file_index;
        self.diff_selection.hunk_index = 0;
    }

    pub(super) fn set_detail_loading(&mut self, loading: bool) {
        self.detail_state.detail_loading.details = loading;
        self.detail_state.detail_loading.files = loading;
        self.detail_state.detail_loading.checks = loading;
        self.detail_state.detail_loading.workflows = loading;
        self.detail_state.detail_loading.reviews = loading;
    }

    pub(super) fn clear_detail_loaded_state(&mut self) {
        self.detail_state.detail_loaded = PullRequestDetailLoadedState::default();
    }

    pub(super) fn clear_detail_errors(&mut self) {
        self.detail_state.details_error = None;
        self.detail_state.files_error = None;
        self.detail_state.checks_error = None;
        self.detail_state.workflows_error = None;
        self.review_state.reviews_error = None;
    }

    pub(super) fn clear_action_errors(&mut self) {
        self.action_error = None;
        self.pr_action_error = None;
    }

    pub(super) fn clear_review_submission_errors(&mut self) {
        self.review_state.review_comment_error = None;
        self.review_state.pending_review_error = None;
    }

    pub(super) fn clear_review_data_state(&mut self) {
        self.review_state.pull_request_reviews.clear();
        self.review_state.review_threads.clear();
        self.clear_review_composer_state();
        self.review_state.pending_review = None;
    }

    pub(super) fn clear_changed_file_state(&mut self) {
        self.detail_state.files.clear();
        self.detail_state.diffs.clear();
        self.collapsed_file_tree_folders.clear();
        self.reviewed_file_paths.clear();
        self.reset_changed_file_filters();
        self.owned_file_paths.clear();
    }

    pub(super) fn clear_workflow_state(&mut self) {
        self.detail_state.check_runs.clear();
        self.detail_state.workflow_runs.clear();
        self.detail_state.workflow_jobs.clear();
    }

    pub(super) fn clear_log_content(&mut self) {
        self.detail_state.log_state.chunk = None;
    }

    pub(super) fn clear_log_error(&mut self) {
        self.detail_state.log_state.error = None;
    }

    pub(super) fn set_log_loading(&mut self, loading: bool) {
        self.detail_state.log_state.is_loading = loading;
    }

    pub(super) fn reset_detail_scrolls(&mut self) {
        self.file_list_scroll.scroll_to_item(0, ScrollStrategy::Top);
        self.reset_diff_list_scroll();
        self.review_list_scroll
            .scroll_to_item(0, ScrollStrategy::Top);
        self.detail_state
            .log_state
            .list_scroll
            .scroll_to_item(0, ScrollStrategy::Top);
    }
}
