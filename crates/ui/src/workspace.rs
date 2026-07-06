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
mod pull_request_detail_cache_loader;
mod pull_request_detail_content_loaders;
mod pull_request_detail_loaders;
mod pull_request_inbox_refresh;
mod render;
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
    Context, Entity, FocusHandle, ListState, ScrollStrategy, Subscription, UniformListScrollHandle,
};
use gpui_component::input::InputState;
use harbor_domain::{PullRequest, RepoId, WorkflowRun};
pub(crate) use harbor_sync::PullRequestInboxMode;

use crate::actions::PanelTab;
use crate::panels::{DiffListItem, workflow_run_failed};

pub(crate) use cache::{
    PullRequestDetailCacheKey, PullRequestDetailSnapshot, PullRequestInboxCacheKey,
    PullRequestInboxSnapshot,
};
pub(crate) use changed_files::{
    ChangedFileFilters, ChangedFileFolderRow, ChangedFileRow, ChangedFileTreeRow,
    ChangedFileTypeFilter, changed_file_tree_rows, changed_file_type_filters,
};
use external_apps::ExternalAppAvailability;
use github_service::GitHubApi;
use reviews::ReviewReactionKey;
pub(crate) use reviews::{
    PendingReviewSession, ReviewCommentSubmission, ReviewCommentUiError, ReviewComposer,
    ReviewLineSelection, ReviewLineTarget, ReviewReactionAction, ReviewThreadUiError,
    review_comment_pending_sync, review_range_from_targets, review_reaction,
};
use state::{
    NotificationState, PullRequestDetailUiState, PullRequestInboxState,
    PullRequestRowEnrichmentKey, PullRequestSelectionState, RepositoryUiState, ReviewComposerState,
    ReviewRuntimeState, SyncRuntimeState, WorkflowLogState, WorkspaceTasks,
};
use status::ActionRuntimeState;
pub(crate) use switchers::{RepositorySwitcherChoice, normalized_search_query};

pub(crate) use auth_state::{GitHubAuthSource, GitHubAuthStatus, GitHubCliAvailability};
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
}

pub(super) fn log_entity_update_error(context: &'static str, error: impl std::fmt::Display) {
    tracing::warn!(%error, "{}", context);
}

const DIFF_LIST_OVERDRAW: f32 = 240.0;
const PANEL_LIST_OVERDRAW: f32 = 160.0;

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
    pub(crate) detail_state: PullRequestDetailUiState,
    pub(crate) review_state: ReviewRuntimeState,
    notification_state: NotificationState,
    sync_runtime: SyncRuntimeState,
    pr_list_scroll: UniformListScrollHandle,
    file_list_scroll: UniformListScrollHandle,
    diff_list_state: ListState,
    diff_list_items: Vec<DiffListItem>,
    review_list_state: ListState,
    checks_list_state: ListState,
    actions_list_state: ListState,
    selection_state: PullRequestSelectionState,
    active_tab: PanelTab,
    pull_request_inbox: PullRequestInboxState,
    prefetch_inbox_counts: bool,
    pull_request_inbox_search_open: bool,
    file_filter_popover_open: bool,
    review_action_comment_target: Option<ReviewActionCommentTarget>,
    review_action_comment_input: Entity<InputState>,
    pull_request_switcher_selection: usize,
    pull_request_search_input: Entity<InputState>,
    external_app_availability: ExternalAppAvailability,
    collapsed_file_tree_folders: HashSet<String>,
    expanded_diff_file_paths: HashSet<String>,
    collapsed_diff_file_paths: HashSet<String>,
    reviewed_file_paths: HashSet<String>,
    excluded_file_type_filters: HashSet<String>,
    show_files_owned_by_current_user: bool,
    owned_file_paths: HashSet<String>,
    action_runtime: ActionRuntimeState,
    status: String,
    _subscriptions: Vec<Subscription>,
}

impl AppView {
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
        self.selection_state.set_pull_request_index(index);

        if self.restore_selected_pull_request_detail_snapshot(cx) {
            return;
        }

        self.reset_diff_selection();
        self.collapsed_file_tree_folders.clear();
        self.expanded_diff_file_paths.clear();
        self.collapsed_diff_file_paths.clear();
        self.reviewed_file_paths.clear();
        self.reset_changed_file_filters();
        self.owned_file_paths.clear();
        self.clear_detail_loaded_state();
        self.detail_state.clear_workflow_jobs();
        self.clear_log_content();
        self.clear_review_data_state();
        self.review_state.clear_reviews_error();
        self.clear_log_error();
        self.action_runtime.clear_pull_request_action_error();
        self.review_state.clear_submission_errors();
        self.sync_diff_list_items(cx);
        self.pr_list_scroll
            .scroll_to_item(index, ScrollStrategy::Center);
        self.file_list_scroll.scroll_to_item(0, ScrollStrategy::Top);
        self.reset_diff_list_scroll();
        self.reset_panel_list_scrolls();
        self.status = format!("Selected {}", self.selected_pr_label());

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
            self.load_repository_pull_requests_from_cache(repository, mode, cx);
        } else {
            self.pull_request_inbox.set_mode(mode);
            self.pull_requests.clear();
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
