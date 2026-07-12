use std::{collections::HashSet, sync::Arc};

use gpui::{AppContext, Context, ListAlignment, ListState, UniformListScrollHandle, Window, px};
use gpui_component::{ActiveTheme, input::InputState};
use harbor_sync::{ActivityState, SyncPolicy};

use crate::{
    actions::{PanelTab, PullRequestMetadataField},
    diff::parse_files_with_syntax,
    panels::CheckRunFilter,
};

use super::{
    ActionRuntimeState, AppView, DIFF_LIST_OVERDRAW, GitHubAuthStatus, GitHubCliAvailability,
    OVERVIEW_LIST_OVERDRAW, PANEL_LIST_OVERDRAW, PullRequestFilters, SettingsSection,
    external_apps::ExternalAppAvailability,
    github_service::GitHubApi,
    notifications::NativeNotificationSink,
    state::{
        NotificationState, OverviewUiState, PanelListState, PullRequestDetailUiState,
        PullRequestInboxState, PullRequestSelectionState, RepositoryActionsUiState,
        RepositoryUiState, ReviewComposerState, ReviewRuntimeState, SyncRuntimeState,
        WorkflowLogState, WorkspaceTasks,
    },
};

impl AppView {
    pub fn new(
        github_api: Arc<dyn GitHubApi>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        Self::new_with_startup_tasks_and_github_api(window, cx, true, github_api)
    }

    #[cfg(test)]
    pub(crate) fn new_without_startup_tasks(window: &mut Window, cx: &mut Context<Self>) -> Self {
        Self::new_with_startup_tasks_and_github_api(
            window,
            cx,
            false,
            Arc::new(super::RealGitHubApi::default()),
        )
    }

    #[cfg(test)]
    pub(crate) fn new_with_github_api(
        github_api: Arc<dyn GitHubApi>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        Self::new_with_startup_tasks_and_github_api(window, cx, false, github_api)
    }

    fn new_with_startup_tasks_and_github_api(
        window: &mut Window,
        cx: &mut Context<Self>,
        start_startup_tasks: bool,
        github_api: Arc<dyn GitHubApi>,
    ) -> Self {
        let pull_requests = Vec::new();
        let files = Vec::new();
        let pull_request_reviews = Vec::new();
        let review_threads = Vec::new();
        let review_comment_input = cx.new(|cx| {
            InputState::new(window, cx)
                .auto_grow(3, 8)
                .placeholder("Leave a comment")
                .clean_on_escape()
        });
        let review_thread_reply_input = cx.new(|cx| {
            InputState::new(window, cx)
                .auto_grow(2, 5)
                .placeholder("Reply to thread")
                .clean_on_escape()
        });
        let review_comment_edit_input = cx.new(|cx| {
            InputState::new(window, cx)
                .auto_grow(2, 6)
                .placeholder("Edit comment")
                .clean_on_escape()
        });
        let pending_review_body_input = cx.new(|cx| {
            InputState::new(window, cx)
                .auto_grow(2, 6)
                .placeholder("Review summary")
                .clean_on_escape()
        });
        let review_action_comment_input = cx.new(|cx| {
            InputState::new(window, cx)
                .auto_grow(3, 8)
                .placeholder("Review comment")
                .clean_on_escape()
        });
        let overview_comment_input = cx.new(|cx| {
            InputState::new(window, cx)
                .multi_line(true)
                .rows(4)
                .placeholder("Add your comment here...")
                .clean_on_escape()
        });
        let pull_request_description_input = cx.new(|cx| {
            InputState::new(window, cx)
                .auto_grow(8, 24)
                .placeholder("Pull request description")
                .clean_on_escape()
        });
        let pull_request_reviewer_input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder(PullRequestMetadataField::Reviewer.input_placeholder())
                .clean_on_escape()
        });
        let pull_request_assignee_input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder(PullRequestMetadataField::Assignee.input_placeholder())
                .clean_on_escape()
        });
        let pull_request_label_input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder(PullRequestMetadataField::Label.input_placeholder())
                .clean_on_escape()
        });
        let repository_search_input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("Search repositories...")
                .clean_on_escape()
        });
        let pull_request_search_input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("Search loaded pull requests...")
                .clean_on_escape()
        });
        let mut subscriptions = vec![
            cx.subscribe_in(
                &repository_search_input,
                window,
                Self::on_switcher_search_event,
            ),
            cx.subscribe_in(
                &pull_request_search_input,
                window,
                Self::on_switcher_search_event,
            ),
            cx.subscribe_in(&review_comment_input, window, Self::on_review_input_event),
            cx.subscribe_in(
                &review_thread_reply_input,
                window,
                Self::on_review_input_event,
            ),
            cx.subscribe_in(
                &review_comment_edit_input,
                window,
                Self::on_review_input_event,
            ),
            cx.subscribe_in(
                &pending_review_body_input,
                window,
                Self::on_review_input_event,
            ),
            cx.subscribe_in(
                &review_action_comment_input,
                window,
                Self::on_review_input_event,
            ),
            cx.subscribe_in(&overview_comment_input, window, Self::on_review_input_event),
            cx.subscribe_in(
                &pull_request_description_input,
                window,
                Self::on_review_input_event,
            ),
            cx.subscribe_in(
                &pull_request_reviewer_input,
                window,
                Self::on_pull_request_metadata_input_event,
            ),
            cx.subscribe_in(
                &pull_request_assignee_input,
                window,
                Self::on_pull_request_metadata_input_event,
            ),
            cx.subscribe_in(
                &pull_request_label_input,
                window,
                Self::on_pull_request_metadata_input_event,
            ),
        ];
        subscriptions.push(cx.observe_window_activation(window, |view, window, cx| {
            view.sync_runtime
                .set_activity(if window.is_window_active() {
                    ActivityState::Focused
                } else {
                    ActivityState::Background
                });
            if view.sync_runtime.activity_state() == ActivityState::Focused {
                view.catch_up_active_inbox_after_focus(cx);
            }
            view.ensure_sync_loop(cx);
            cx.notify();
        }));
        let diffs = parse_files_with_syntax(&files, &cx.theme().highlight_theme);
        let status = if start_startup_tasks {
            "Fetching repositories from GitHub...".to_string()
        } else {
            "Ready".to_string()
        };

        let mut view = Self {
            focus_handle: cx.focus_handle(),
            pull_requests,
            github_api,
            auth_status: GitHubAuthStatus::Loading,
            github_cli_availability: GitHubCliAvailability::Checking,
            github_auth_popover_open: false,
            settings_open: false,
            settings_section: SettingsSection::GitHub,
            auth_switch_status: None,
            tasks: WorkspaceTasks::default(),
            repository_state: RepositoryUiState::new(repository_search_input, start_startup_tasks),
            repository_actions_state: RepositoryActionsUiState::new(),
            detail_state: PullRequestDetailUiState::new(files, diffs, WorkflowLogState::new()),
            review_state: ReviewRuntimeState::new(
                pull_request_reviews,
                review_threads,
                ReviewComposerState::new(
                    review_comment_input,
                    review_thread_reply_input,
                    review_comment_edit_input,
                    pending_review_body_input,
                ),
            ),
            notification_state: NotificationState {
                notification_sink: Arc::new(NativeNotificationSink::new()),
                notification_dedupe: HashSet::new(),
                notifications_enabled: true,
            },
            sync_runtime: SyncRuntimeState::new(
                if window.is_window_active() {
                    ActivityState::Focused
                } else {
                    ActivityState::Background
                },
                SyncPolicy::default(),
            ),
            pr_list_scroll: UniformListScrollHandle::new(),
            file_list_scroll: UniformListScrollHandle::new(),
            diff_list_state: ListState::new(0, ListAlignment::Top, px(DIFF_LIST_OVERDRAW)),
            diff_list_items: Vec::new(),
            overview_state: OverviewUiState::new(ListState::new(
                0,
                ListAlignment::Top,
                px(OVERVIEW_LIST_OVERDRAW),
            )),
            panel_list_state: PanelListState::new(
                ListState::new(0, ListAlignment::Top, px(PANEL_LIST_OVERDRAW)),
                ListState::new(0, ListAlignment::Top, px(PANEL_LIST_OVERDRAW)),
                ListState::new(0, ListAlignment::Top, px(PANEL_LIST_OVERDRAW)),
                ListState::new(0, ListAlignment::Top, px(PANEL_LIST_OVERDRAW)),
                ListState::new(0, ListAlignment::Top, px(PANEL_LIST_OVERDRAW)),
            ),
            selection_state: PullRequestSelectionState::default(),
            active_tab: PanelTab::Overview,
            pull_request_inbox: PullRequestInboxState::visible_by_default(),
            prefetch_inbox_counts: start_startup_tasks,
            pull_request_inbox_search_open: false,
            pull_request_filter_popover_open: false,
            pull_request_filters: PullRequestFilters::default(),
            file_filter_popover_open: false,
            review_action_comment_target: None,
            review_action_comment_input,
            overview_comment_input,
            pull_request_description_editing: false,
            pull_request_description_input,
            pull_request_reviewer_input,
            pull_request_assignee_input,
            pull_request_label_input,
            pull_request_metadata_options: Default::default(),
            pull_request_switcher_selection: 0,
            pull_request_search_input,
            external_app_availability: ExternalAppAvailability::default(),
            collapsed_file_tree_folders: HashSet::new(),
            collapsed_check_groups: HashSet::new(),
            expanded_diff_file_paths: HashSet::new(),
            collapsed_diff_file_paths: HashSet::new(),
            reviewed_file_paths: HashSet::new(),
            excluded_file_type_filters: HashSet::new(),
            show_files_owned_by_current_user: false,
            owned_file_paths: HashSet::new(),
            checks_filter: CheckRunFilter::All,
            action_runtime: ActionRuntimeState::default(),
            status,
            _subscriptions: subscriptions,
        };

        if start_startup_tasks {
            view.load_github_credentials(cx);
            view.load_repository_preferences(cx);
            view.refresh_external_app_availability(cx);
            view.ensure_sync_loop(cx);
        }

        view
    }
}
