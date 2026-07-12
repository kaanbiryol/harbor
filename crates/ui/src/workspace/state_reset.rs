use std::sync::Arc;

use gpui::{Context, ListOffset, ScrollStrategy, Window, px};

use crate::{actions::PanelTab, workspace::AppView};

impl AppView {
    pub(super) fn reset_diff_selection(&mut self) {
        self.selection_state.reset_diff_selection();
    }

    pub(super) fn select_diff_file_index(&mut self, file_index: usize) {
        self.selection_state.select_file_index(file_index);
    }

    pub(super) fn set_detail_loading(&mut self, loading: bool) {
        if loading {
            self.detail_state.start_details_load();
            self.detail_state.start_files_load();
            self.detail_state.start_checks_load();
            self.detail_state.start_workflows_load();
            self.review_state.start_reviews_load();
        } else {
            self.detail_state.reset_for_selection();
            self.review_state.reset_reviews_load();
        }
    }

    pub(super) fn clear_detail_loaded_state(&mut self) {
        self.detail_state.reset_for_selection();
        self.review_state.reset_reviews_load();
    }

    pub(super) fn clear_detail_errors(&mut self) {
        self.detail_state.clear_errors();
        self.review_state.clear_reviews_error();
    }

    pub(super) fn clear_action_errors(&mut self) {
        self.action_runtime.clear_errors();
    }

    pub(super) fn clear_review_submission_errors(&mut self) {
        self.review_state.clear_submission_errors();
    }

    pub(super) fn clear_review_data_state(&mut self) {
        self.review_state.clear_review_data();
        self.overview_state.clear_content();
    }

    pub(super) fn clear_changed_file_state(&mut self) {
        self.detail_state.clear_diff_files();
        self.changed_files_state.reset();
    }

    pub(super) fn clear_workflow_state(&mut self) {
        self.detail_state.clear_check_runs();
        self.detail_state.clear_commits();
        self.detail_state.clear_workflow_runs();
        self.detail_state.clear_workflow_jobs();
        self.checks_state.reset();
    }

    pub(super) fn clear_log_content(&mut self) {
        self.detail_state.log_state.clear_content();
    }

    pub(super) fn clear_selected_pull_request_detail_state(&mut self) {
        self.active_commit_sha = None;
        self.pull_request_description_editing = false;
        self.clear_changed_file_state();
        self.clear_workflow_state();
        self.clear_review_data_state();
        self.clear_detail_loaded_state();
        self.clear_review_submission_errors();
        self.review_action_comment_target = None;
        self.clear_log_content();
        self.reset_diff_selection();
        self.reset_detail_scrolls();
    }

    pub(super) fn clear_overview_comment_input(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.overview_comment_input
            .update(cx, |input, cx| input.set_value("", window, cx));
    }

    pub(super) fn clear_log_error(&mut self) {
        self.detail_state.log_state.clear_error();
    }

    pub(super) fn set_log_loading(&mut self, loading: bool) {
        self.detail_state.log_state.set_loading(loading);
    }

    pub(super) fn reset_detail_scrolls(&mut self) {
        self.file_list_scroll.scroll_to_item(0, ScrollStrategy::Top);
        self.reset_diff_list_scroll();
        self.reset_panel_list_scrolls();
        self.detail_state
            .log_state
            .list_scroll
            .scroll_to_item(0, ScrollStrategy::Top);
    }

    pub(super) fn reset_panel_list_scrolls(&mut self) {
        self.overview_state.list_state.scroll_to(ListOffset {
            item_ix: 0,
            offset_in_item: px(0.0),
        });
        self.panel_list_state.review.scroll_to(ListOffset {
            item_ix: 0,
            offset_in_item: px(0.0),
        });
        self.panel_list_state.checks.scroll_to(ListOffset {
            item_ix: 0,
            offset_in_item: px(0.0),
        });
        self.panel_list_state
            .action_workflows
            .scroll_to(ListOffset {
                item_ix: 0,
                offset_in_item: px(0.0),
            });
        self.panel_list_state.action_runs.scroll_to(ListOffset {
            item_ix: 0,
            offset_in_item: px(0.0),
        });
    }

    pub(super) fn clear_authenticated_github_content(&mut self) {
        self.tasks.cancel_pull_request_list_task();
        self.tasks.cancel_selected_pull_request_tasks();
        self.tasks.cancel_repository_actions_tasks();
        self.repository_state.clear_visible_repositories();
        self.repository_actions_state.clear();
        self.repository_state.finish_loading();
        self.pull_request_inbox.reset_load();
        self.pull_request_inbox.clear_page_info();
        self.pull_requests.clear();
        self.reset_pull_request_filters();
        self.selection_state.reset_pull_request_index();
        self.reset_diff_selection();
        self.clear_changed_file_state();
        self.clear_workflow_state();
        self.clear_detail_loaded_state();
        self.clear_detail_errors();
        self.clear_action_errors();
        self.clear_review_data_state();
        self.clear_review_submission_errors();
        self.clear_log_content();
        self.clear_log_error();
        self.set_log_loading(false);
        self.review_state.set_current_user_login(None);
        self.diff_list_items = Arc::from([]);
        self.active_tab = PanelTab::Overview;
        self.pull_request_switcher_selection = 0;
        self.pr_list_scroll.scroll_to_item(0, ScrollStrategy::Top);
        self.reset_detail_scrolls();
    }
}
