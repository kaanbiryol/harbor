use gpui::ScrollStrategy;

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
        self.detail_loading.details = loading;
        self.detail_loading.files = loading;
        self.detail_loading.checks = loading;
        self.detail_loading.workflows = loading;
        self.detail_loading.reviews = loading;
    }

    pub(super) fn clear_detail_errors(&mut self) {
        self.details_error = None;
        self.files_error = None;
        self.checks_error = None;
        self.workflows_error = None;
        self.reviews_error = None;
    }

    pub(super) fn clear_action_errors(&mut self) {
        self.action_error = None;
        self.pr_action_error = None;
    }

    pub(super) fn clear_review_submission_errors(&mut self) {
        self.review_comment_error = None;
        self.pending_review_error = None;
    }

    pub(super) fn clear_review_data_state(&mut self) {
        self.pull_request_reviews.clear();
        self.review_threads.clear();
        self.clear_review_composer_state();
        self.pending_review = None;
    }

    pub(super) fn clear_changed_file_state(&mut self) {
        self.files.clear();
        self.diffs.clear();
        self.collapsed_file_tree_folders.clear();
        self.reviewed_file_paths.clear();
        self.reset_changed_file_filters();
        self.owned_file_paths.clear();
    }

    pub(super) fn clear_workflow_state(&mut self) {
        self.check_runs.clear();
        self.workflow_runs.clear();
        self.workflow_jobs.clear();
    }

    pub(super) fn clear_log_content(&mut self) {
        self.log_state.chunk = None;
    }

    pub(super) fn clear_log_error(&mut self) {
        self.log_state.error = None;
    }

    pub(super) fn set_log_loading(&mut self, loading: bool) {
        self.log_state.is_loading = loading;
    }

    pub(super) fn reset_detail_scrolls(&mut self) {
        self.file_list_scroll.scroll_to_item(0, ScrollStrategy::Top);
        self.reset_diff_list_scroll();
        self.review_list_scroll
            .scroll_to_item(0, ScrollStrategy::Top);
        self.log_state
            .list_scroll
            .scroll_to_item(0, ScrollStrategy::Top);
    }
}
