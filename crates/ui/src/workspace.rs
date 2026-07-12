mod action_commands;
pub(crate) mod async_updates;
mod auth;
mod auth_cli;
mod auth_oauth;
mod auth_state;
mod cache;
mod changed_file_interactions;
mod changed_files;
mod codeowners;
mod commands;
mod external_apps;
pub(crate) mod github_service;
mod initialization;
mod loaders;
mod local_commands;
mod navigation_commands;
mod notifications;
mod panel_commands;
mod pull_request_ci_loaders;
mod pull_request_commit_loaders;
mod pull_request_description_actions;
mod pull_request_detail_cache_loader;
mod pull_request_detail_content_loaders;
mod pull_request_detail_loaders;
mod pull_request_filters;
mod pull_request_inbox_refresh;
mod pull_request_metadata_actions;
mod render;
mod repository_actions_loaders;
mod repository_loaders;
mod review_data_loaders;
mod review_interactions;
mod review_state;
mod review_submissions;
mod reviews;
mod settings;
mod state;
mod state_reset;
mod status;
mod switchers;
mod sync_loop;
mod workflow_log_loaders;

use std::{collections::HashSet, path::PathBuf, sync::Arc};

use gpui::{
    Context, Entity, FocusHandle, ListOffset, ListState, ScrollStrategy, Subscription,
    UniformListScrollHandle, px,
};
use gpui_component::input::InputState;
use harbor_domain::{PullRequest, RepoId, WorkflowRun};
pub(crate) use harbor_sync::PullRequestInboxMode;

use crate::actions::PanelTab;
use crate::panels::{CheckRunFilter, DiffListItem, workflow_run_failed};

pub(crate) use cache::{
    PullRequestDetailCacheKey, PullRequestDetailSnapshot, PullRequestInboxCacheKey,
    PullRequestInboxSnapshot,
};
pub(crate) use changed_files::{
    ChangedFileFilters, ChangedFileFolderRow, ChangedFileRow, ChangedFileTreeRow,
    ChangedFileTypeFilter, changed_file_tree_rows, changed_file_type_filters,
};
use external_apps::ExternalAppAvailability;
pub use github_service::{GitHubApi, RealGitHubApi};
pub(crate) use pull_request_filters::{
    PullRequestFilterFacet, PullRequestFilterOption, PullRequestFilterSections, PullRequestFilters,
};
use pull_request_metadata_actions::PullRequestMetadataOptionsState;
use reviews::ReviewReactionKey;
pub(crate) use reviews::{
    PendingReviewSession, ReviewCommentSubmission, ReviewCommentUiError, ReviewComposer,
    ReviewLineSelection, ReviewLineTarget, ReviewReactionAction, ReviewThreadUiError,
    review_comment_pending_sync, review_range_from_targets, review_reaction,
};
use state::{
    NotificationState, OverviewUiState, PanelListState, PullRequestDetailUiState,
    PullRequestInboxState, PullRequestRowEnrichmentKey, PullRequestSelectionState,
    RepositoryActionsUiState, RepositoryUiState, ReviewComposerState, ReviewRuntimeState,
    SyncRuntimeState, WorkflowLogState, WorkspaceTasks,
};
use status::ActionRuntimeState;
pub(crate) use switchers::{RepositorySwitcherChoice, normalized_search_query};

pub use auth_state::GitHubAuthSource;
pub(crate) use auth_state::{GitHubAuthStatus, GitHubCliAvailability};
pub(crate) use settings::{AuthSwitchStatus, SettingsSection};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ReviewActionCommentTarget {
    Approve,
    RequestChanges,
}

impl ReviewActionCommentTarget {
    pub(crate) fn title(self) -> &'static str {
        match self {
            Self::Approve => "Approve with comment",
            Self::RequestChanges => "Request changes",
        }
    }

    pub(crate) fn placeholder(self) -> &'static str {
        match self {
            Self::Approve => "Leave an approval comment",
            Self::RequestChanges => "Describe the requested changes",
        }
    }

    pub(crate) fn submit_label(self) -> &'static str {
        match self {
            Self::Approve => "Approve",
            Self::RequestChanges => "Request changes",
        }
    }

    pub(crate) fn requires_body(self) -> bool {
        match self {
            Self::Approve => true,
            Self::RequestChanges => false,
        }
    }

    pub(crate) fn empty_body_status(self) -> &'static str {
        match self {
            Self::Approve => "Add a comment before approving with comment",
            Self::RequestChanges => "Describe the requested changes before submitting",
        }
    }
}

pub(super) fn log_entity_update_error(context: &'static str, error: impl std::fmt::Display) {
    tracing::warn!(%error, "{}", context);
}

const DIFF_LIST_OVERDRAW: f32 = 240.0;
const PANEL_LIST_OVERDRAW: f32 = 160.0;
const OVERVIEW_LIST_OVERDRAW: f32 = 64.0;

pub struct AppView {
    focus_handle: FocusHandle,
    pull_requests: Vec<PullRequest>,
    github_api: Arc<dyn GitHubApi>,
    auth_status: GitHubAuthStatus,
    github_cli_availability: GitHubCliAvailability,
    github_auth_popover_open: bool,
    settings_open: bool,
    settings_section: SettingsSection,
    auth_switch_status: Option<AuthSwitchStatus>,
    tasks: WorkspaceTasks,
    repository_state: RepositoryUiState,
    pub(crate) repository_actions_state: RepositoryActionsUiState,
    pub(crate) detail_state: PullRequestDetailUiState,
    pub(crate) review_state: ReviewRuntimeState,
    notification_state: NotificationState,
    sync_runtime: SyncRuntimeState,
    pr_list_scroll: UniformListScrollHandle,
    file_list_scroll: UniformListScrollHandle,
    diff_list_state: ListState,
    diff_list_items: Arc<[DiffListItem]>,
    overview_state: OverviewUiState,
    panel_list_state: PanelListState,
    selection_state: PullRequestSelectionState,
    active_tab: PanelTab,
    pull_request_inbox: PullRequestInboxState,
    prefetch_inbox_counts: bool,
    pull_request_inbox_search_open: bool,
    pull_request_filter_popover_open: bool,
    pull_request_filters: PullRequestFilters,
    file_filter_popover_open: bool,
    review_action_comment_target: Option<ReviewActionCommentTarget>,
    review_action_comment_input: Entity<InputState>,
    overview_comment_input: Entity<InputState>,
    pull_request_description_editing: bool,
    pull_request_description_input: Entity<InputState>,
    pull_request_reviewer_input: Entity<InputState>,
    pull_request_assignee_input: Entity<InputState>,
    pull_request_label_input: Entity<InputState>,
    pull_request_metadata_options: PullRequestMetadataOptionsState,
    pull_request_switcher_selection: usize,
    pull_request_search_input: Entity<InputState>,
    external_app_availability: ExternalAppAvailability,
    collapsed_file_tree_folders: HashSet<String>,
    collapsed_check_groups: HashSet<String>,
    expanded_diff_file_paths: HashSet<String>,
    collapsed_diff_file_paths: HashSet<String>,
    reviewed_file_paths: HashSet<String>,
    excluded_file_type_filters: HashSet<String>,
    show_files_owned_by_current_user: bool,
    owned_file_paths: HashSet<String>,
    checks_filter: CheckRunFilter,
    action_runtime: ActionRuntimeState,
    status: String,
    _subscriptions: Vec<Subscription>,
}

impl AppView {
    pub(crate) fn set_status(&mut self, status: impl Into<String>, cx: &mut Context<Self>) {
        self.status = status.into();
        cx.notify();
    }

    fn selected_pull_request(&self) -> Option<&PullRequest> {
        self.pull_requests
            .get(self.selection_state.pull_request_index())
    }

    fn selected_pull_request_number(&self) -> Option<u64> {
        self.selected_pull_request().map(|pr| pr.number)
    }

    pub(crate) fn selected_pull_request_index(&self) -> usize {
        self.selection_state.pull_request_index()
    }

    pub(crate) fn collapsed_check_groups(&self) -> &HashSet<String> {
        &self.collapsed_check_groups
    }

    pub(crate) fn checks_filter(&self) -> CheckRunFilter {
        self.checks_filter
    }

    pub(crate) fn set_checks_filter(&mut self, filter: CheckRunFilter, cx: &mut Context<Self>) {
        let filter = if filter == CheckRunFilter::All || self.checks_filter == filter {
            CheckRunFilter::All
        } else {
            filter
        };

        self.checks_filter = filter;
        self.panel_list_state.checks.scroll_to(ListOffset {
            item_ix: 0,
            offset_in_item: px(0.0),
        });
        self.status = filter.status_message().to_string();
        cx.notify();
    }

    pub(crate) fn toggle_check_group(&mut self, group_name: String, cx: &mut Context<Self>) {
        self.status = if self.collapsed_check_groups.remove(&group_name) {
            format!("Expanded {group_name} checks")
        } else {
            self.collapsed_check_groups.insert(group_name.clone());
            format!("Collapsed {group_name} checks")
        };
        cx.notify();
    }

    fn selected_pr_label(&self) -> String {
        self.selected_pull_request()
            .map(|pr| format!("PR #{}", pr.number))
            .unwrap_or_else(|| "no selected pull request".to_string())
    }

    fn selected_workflow_run_for_logs(&self) -> Option<&WorkflowRun> {
        self.detail_state
            .workflow_runs()
            .iter()
            .find(|run| workflow_run_failed(run))
            .or_else(|| self.detail_state.workflow_runs().first())
    }

    pub(crate) fn select_pull_request(&mut self, index: usize, cx: &mut Context<Self>) {
        if index >= self.pull_requests.len() {
            self.status = "No pull requests to select".to_string();
            cx.notify();
            return;
        }

        self.cache_current_pull_request_detail_snapshot();
        self.pull_request_description_editing = false;
        self.selection_state.set_pull_request_index(index);
        self.active_tab = PanelTab::Overview;

        if self.restore_selected_pull_request_detail_snapshot(cx) {
            return;
        }

        self.pr_list_scroll.scroll_to_item(
            self.selected_pull_request_list_position(),
            ScrollStrategy::Center,
        );
        self.load_selected_pull_request(cx);
    }

    pub(crate) fn select_pull_request_inbox_mode(
        &mut self,
        mode: PullRequestInboxMode,
        cx: &mut Context<Self>,
    ) {
        if self.pull_request_inbox.mode() == mode {
            return;
        }

        if let Some(repository) = self.repository_state.configured_repo_cloned() {
            self.switch_pull_request_inbox_mode(repository, mode, cx);
        } else {
            self.pull_request_inbox.set_mode(mode);
            self.pull_requests.clear();
            self.reset_pull_request_filters();
            self.clear_changed_file_state();
            self.clear_workflow_state();
            self.clear_review_data_state();
            self.clear_log_content();
            self.selection_state.reset_pull_request_index();
            self.reset_diff_selection();
            self.status =
                "Select a repository from the header before loading pull requests".to_string();
            cx.notify();
        }
    }

    pub(crate) fn remember_repository(&mut self, repository: RepoId) {
        self.repository_state.remember_repository(repository);
    }

    pub(crate) fn current_repository(&self) -> Option<&RepoId> {
        self.repository_state.configured_repo().or_else(|| {
            self.selected_pull_request()
                .map(|pull_request| &pull_request.repo)
        })
    }

    pub(crate) fn current_repository_local_path(&self) -> Option<&PathBuf> {
        self.current_repository()
            .and_then(|repository| self.repository_state.local_path(repository))
    }

    pub(crate) fn set_repository_local_path(&mut self, repository: RepoId, path: PathBuf) {
        self.repository_state.set_local_path(repository, path);
    }
}

#[cfg(test)]
mod tests;
