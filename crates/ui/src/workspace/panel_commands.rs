use gpui::{Context, Window};
use harbor_domain::RepoId;

use crate::actions::*;
use crate::workspace::AppView;

impl AppView {
    pub(super) fn cycle_panel_tab(
        &mut self,
        _: &CyclePanelTab,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.select_panel_tab(self.active_tab.next(), cx);
    }

    pub(super) fn select_overview_panel(
        &mut self,
        _: &SelectOverviewPanel,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.select_panel_tab(PanelTab::Overview, cx);
    }

    pub(super) fn select_diff_panel(
        &mut self,
        _: &SelectDiffPanel,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.select_panel_tab(PanelTab::Diff, cx);
    }

    pub(super) fn select_review_panel(
        &mut self,
        _: &SelectReviewPanel,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.select_panel_tab(PanelTab::Review, cx);
    }

    pub(super) fn select_checks_panel(
        &mut self,
        _: &SelectChecksPanel,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.select_panel_tab(PanelTab::Checks, cx);
    }

    pub(super) fn select_commits_panel(
        &mut self,
        _: &SelectCommitsPanel,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.select_panel_tab(PanelTab::Commits, cx);
    }

    pub(super) fn select_actions_panel(
        &mut self,
        _: &SelectActionsPanel,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.select_panel_tab(PanelTab::Actions, cx);
    }

    pub(super) fn select_logs_panel(
        &mut self,
        _: &SelectLogsPanel,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.select_panel_tab(PanelTab::Logs, cx);
    }

    pub(super) fn toggle_pull_request_inbox(
        &mut self,
        _: &TogglePullRequestInbox,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.pull_request_inbox.toggle_visible();
        self.repository_state.repository_switcher_open = false;
        self.pull_request_inbox_search_open = false;
        self.pull_request_filter_popover_open = false;
        self.file_filter_popover_open = false;
        self.review_action_comment_target = None;
        self.status = if self.pull_request_inbox.is_visible() {
            "Pull request inbox shown".to_string()
        } else {
            "Pull request inbox hidden".to_string()
        };
        cx.notify();
    }

    pub(crate) fn select_panel_tab(&mut self, tab: PanelTab, cx: &mut Context<Self>) {
        if self.active_tab == tab {
            return;
        }

        self.active_tab = tab;
        self.status = format!("Switched to {} panel", tab.label());
        self.load_active_panel_data_if_needed(cx);
        cx.notify();
    }

    pub(super) fn toggle_repository_switcher(
        &mut self,
        _: &ToggleRepositorySwitcher,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.github_auth_gate_visible() {
            self.repository_state.repository_switcher_open = false;
            cx.notify();
            return;
        }

        self.repository_state.repository_switcher_open =
            !self.repository_state.repository_switcher_open;
        if self.repository_state.repository_switcher_open {
            self.pull_request_inbox_search_open = false;
            self.pull_request_filter_popover_open = false;
            self.file_filter_popover_open = false;
            self.review_action_comment_target = None;
            self.repository_state
                .repository_search_input
                .update(cx, |input, cx| {
                    input.set_value("", window, cx);
                    input.focus(window, cx);
                });
            self.reset_repository_switcher_selection(cx);
        }
        self.status = if self.repository_state.repository_switcher_open {
            "Repository switcher opened".to_string()
        } else {
            "Repository switcher closed".to_string()
        };
        cx.notify();
    }

    pub(super) fn open_pull_request_search(
        &mut self,
        _: &OpenPullRequestSearch,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.current_repository().is_none() {
            self.status = "Select a repository before searching pull requests".to_string();
            cx.notify();
            return;
        }

        self.pull_request_inbox.set_visible(true);
        self.pull_request_inbox_search_open = true;
        self.repository_state.repository_switcher_open = false;
        self.file_filter_popover_open = false;
        self.review_action_comment_target = None;
        self.pull_request_search_input.update(cx, |input, cx| {
            input.set_value("", window, cx);
            input.focus(window, cx);
        });
        self.reset_pull_request_switcher_selection(cx);
        self.status = "Pull request search opened".to_string();
        cx.notify();
    }

    pub(super) fn close_panel(
        &mut self,
        _: &ClosePanel,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.settings_open {
            self.close_settings(&CloseSettings, window, cx);
            return;
        }

        self.settings_open = false;
        self.repository_state.repository_switcher_open = false;
        self.pull_request_inbox_search_open = false;
        self.pull_request_filter_popover_open = false;
        self.file_filter_popover_open = false;
        self.review_action_comment_target = None;
        self.status = "Closed transient UI".to_string();
        cx.notify();
    }

    pub(crate) fn select_repository_from_switcher(
        &mut self,
        repository: RepoId,
        cx: &mut Context<Self>,
    ) {
        let selected_repository = repository.full_name();
        if self.repository_state.configured_repo() == Some(&repository) {
            self.status = format!("Selected repository {selected_repository}");
            cx.notify();
            return;
        }

        self.load_pull_requests(repository, cx);
    }

    pub(super) fn open_logs(&mut self, _: &OpenLogs, _: &mut Window, cx: &mut Context<Self>) {
        if self.selected_pull_request().is_some() {
            self.active_tab = PanelTab::Logs;
            self.load_active_panel_data_if_needed(cx);
            cx.notify();
        } else {
            self.status = "Select a pull request before opening logs".to_string();
            cx.notify();
        }
    }

    pub(super) fn filter_current_list(
        &mut self,
        _: &FilterCurrentList,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.file_filter_popover_open = !self.file_filter_popover_open;
        self.repository_state.repository_switcher_open = false;
        self.pull_request_inbox_search_open = false;
        self.pull_request_filter_popover_open = false;
        self.review_action_comment_target = None;
        self.status = if self.file_filter_popover_open {
            "Opened changed-file filters".to_string()
        } else {
            "Closed changed-file filters".to_string()
        };
        cx.notify();
    }
}
