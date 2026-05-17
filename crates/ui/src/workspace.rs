mod action_commands;
pub(crate) mod async_updates;
mod auth;
mod cache;
mod changed_files;
mod commands;
mod external_apps;
pub(crate) mod github_service;
mod loaders;
mod local_commands;
mod navigation_commands;
mod notifications;
mod panel_commands;
mod pull_request_detail_loaders;
mod render;
mod review_data_loaders;
mod review_interactions;
mod review_state;
mod review_submissions;
mod reviews;
mod settings;
mod state;
mod state_reset;
mod switchers;
mod sync_loop;
mod workflow_log_loaders;

use std::{collections::HashSet, path::PathBuf, sync::Arc};

use gpui::{
    App, AppContext, Context, Entity, FocusHandle, ListAlignment, ListOffset, ListState,
    ScrollStrategy, Subscription, UniformListScrollHandle, Window, px,
};
use gpui_component::{ActiveTheme, input::InputState};
use harbor_domain::{DiffFile, PullRequest, RepoId, WorkflowRun};
pub(crate) use harbor_sync::PullRequestInboxMode;
use harbor_sync::{ActivityState, SyncPolicy};

use crate::actions::PanelTab;
use crate::diff::{ParsedDiff, parse_files_with_syntax};
use crate::panels::{
    ContinuousDiffLayoutInput, DiffListItem, continuous_diff_file_item_index,
    continuous_diff_items, sync_diff_list_state, workflow_run_failed,
};

pub(crate) use cache::{
    PullRequestDetailCacheKey, PullRequestDetailSnapshot, PullRequestInboxCacheKey,
    PullRequestInboxSnapshot,
};
use changed_files::codeowners_owned_file_paths;
pub(crate) use changed_files::{
    ChangedFileFilters, ChangedFileFolderRow, ChangedFileRow, ChangedFileTreeRow,
    ChangedFileTypeFilter, changed_file_status_label, changed_file_tree_rows,
    changed_file_type_filters,
};
use external_apps::ExternalAppAvailability;
use github_service::{GitHubApi, RealGitHubApi};
use notifications::NativeNotificationSink;
use reviews::ReviewReactionKey;
pub(crate) use reviews::{
    PendingReviewSession, ReviewCommentSubmission, ReviewCommentUiError, ReviewComposer,
    ReviewLineSelection, ReviewLineTarget, ReviewReactionAction, ReviewThreadUiError,
    review_comment_pending_sync, review_range_from_targets, review_reaction,
};
use state::{
    ActionRuntimeState, NotificationState, PullRequestDetailUiState, PullRequestInboxState,
    PullRequestSelectionState, RepositoryUiState, ReviewComposerState, ReviewRuntimeState,
    SyncRuntimeState, WorkflowLogState, WorkspaceTasks,
};
pub(crate) use switchers::{RepositorySwitcherChoice, normalized_search_query};

pub(crate) use auth::{GitHubAuthSource, GitHubAuthStatus, GitHubCliAvailability};
pub(crate) use settings::{AuthSwitchStatus, SettingsSection};

pub(super) fn log_entity_update_error(context: &'static str, error: impl std::fmt::Display) {
    tracing::warn!(%error, "{}", context);
}

const DIFF_LIST_OVERDRAW: f32 = 240.0;

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
    review_list_scroll: UniformListScrollHandle,
    selection_state: PullRequestSelectionState,
    active_tab: PanelTab,
    pull_request_inbox: PullRequestInboxState,
    prefetch_inbox_counts: bool,
    pull_request_inbox_search_open: bool,
    file_filter_popover_open: bool,
    pull_request_switcher_selection: usize,
    pull_request_search_input: Entity<InputState>,
    external_app_availability: ExternalAppAvailability,
    collapsed_file_tree_folders: HashSet<String>,
    reviewed_file_paths: HashSet<String>,
    excluded_file_type_filters: HashSet<String>,
    show_files_owned_by_current_user: bool,
    owned_file_paths: HashSet<String>,
    action_runtime: ActionRuntimeState,
    status: String,
    _subscriptions: Vec<Subscription>,
}

impl AppView {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        Self::new_with_startup_tasks(window, cx, true)
    }

    #[cfg(test)]
    pub(crate) fn new_without_startup_tasks(window: &mut Window, cx: &mut Context<Self>) -> Self {
        Self::new_with_startup_tasks(window, cx, false)
    }

    #[cfg(test)]
    pub(crate) fn new_with_github_api(
        github_api: Arc<dyn GitHubApi>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        Self::new_with_startup_tasks_and_github_api(window, cx, false, github_api)
    }

    fn new_with_startup_tasks(
        window: &mut Window,
        cx: &mut Context<Self>,
        start_startup_tasks: bool,
    ) -> Self {
        Self::new_with_startup_tasks_and_github_api(
            window,
            cx,
            start_startup_tasks,
            Arc::new(RealGitHubApi::default()),
        )
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
            review_list_scroll: UniformListScrollHandle::new(),
            selection_state: PullRequestSelectionState::default(),
            active_tab: PanelTab::Diff,
            pull_request_inbox: PullRequestInboxState::visible_by_default(),
            prefetch_inbox_counts: start_startup_tasks,
            pull_request_inbox_search_open: false,
            file_filter_popover_open: false,
            pull_request_switcher_selection: 0,
            pull_request_search_input,
            external_app_availability: ExternalAppAvailability::default(),
            collapsed_file_tree_folders: HashSet::new(),
            reviewed_file_paths: HashSet::new(),
            excluded_file_type_filters: HashSet::new(),
            show_files_owned_by_current_user: false,
            owned_file_paths: HashSet::new(),
            action_runtime: ActionRuntimeState::default(),
            status,
            _subscriptions: subscriptions,
        };

        if start_startup_tasks {
            view.load_github_credentials(cx);
            view.load_recent_repositories(cx);
            view.refresh_external_app_availability(cx);
            view.ensure_sync_loop(cx);
        }

        view
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

    fn selected_pr_label(&self) -> String {
        self.selected_pull_request()
            .map(|pr| format!("PR #{}", pr.number))
            .unwrap_or_else(|| "no selected pull request".to_string())
    }

    pub(crate) fn active_file(&self) -> Option<&DiffFile> {
        self.detail_state.files.get(self.active_file_index())
    }

    pub(crate) fn active_file_index(&self) -> usize {
        self.selection_state.file_index()
    }

    pub(crate) fn active_hunk_index(&self) -> usize {
        self.selection_state.hunk_index()
    }

    pub(crate) fn diff_files(&self) -> &[DiffFile] {
        &self.detail_state.files
    }

    pub(crate) fn parsed_diffs(&self) -> &[Option<ParsedDiff>] {
        &self.detail_state.diffs
    }

    pub(crate) fn reviewed_file_paths(&self) -> &HashSet<String> {
        &self.reviewed_file_paths
    }

    pub(crate) fn changed_file_tree_rows(&self, _cx: &App) -> Vec<ChangedFileTreeRow> {
        let filters = self.changed_file_filters();

        changed_file_tree_rows(
            &self.detail_state.files,
            &self.collapsed_file_tree_folders,
            &self.reviewed_file_paths,
            &filters,
        )
    }

    pub(crate) fn visible_file_indices(&self, cx: &App) -> Vec<usize> {
        self.changed_file_tree_rows(cx)
            .into_iter()
            .filter_map(|row| match row {
                ChangedFileTreeRow::File(file_row) => Some(file_row.file_index),
                ChangedFileTreeRow::Folder(_) => None,
            })
            .collect()
    }

    pub(crate) fn reviewed_file_count(&self) -> usize {
        self.detail_state
            .files
            .iter()
            .filter(|file| self.reviewed_file_paths.contains(&file.path))
            .count()
    }

    pub(crate) fn changed_file_filters(&self) -> ChangedFileFilters {
        ChangedFileFilters {
            query: String::new(),
            excluded_file_types: self.excluded_file_type_filters.clone(),
            owned_by_current_user_only: self.show_files_owned_by_current_user,
            owned_file_paths: self.owned_file_paths.clone(),
        }
    }

    pub(crate) fn changed_file_type_filters(&self) -> Vec<ChangedFileTypeFilter> {
        changed_file_type_filters(&self.detail_state.files, &self.excluded_file_type_filters)
    }

    pub(crate) fn included_file_type_filter_count(&self) -> usize {
        self.changed_file_type_filters()
            .into_iter()
            .filter(|filter| filter.included)
            .count()
    }

    pub(crate) fn has_owned_file_filter_data(&self) -> bool {
        !self.owned_file_paths.is_empty()
    }

    fn file_tree_row_index_for_file(&self, file_index: usize, cx: &App) -> Option<usize> {
        self.changed_file_tree_rows(cx)
            .into_iter()
            .position(|row| matches!(row, ChangedFileTreeRow::File(file_row) if file_row.file_index == file_index))
    }

    fn diff_item_index_for_file(&self, file_index: usize, cx: &App) -> Option<usize> {
        let visible_file_indices = self.visible_file_indices(cx);

        continuous_diff_file_item_index(
            ContinuousDiffLayoutInput {
                files: &self.detail_state.files,
                diffs: &self.detail_state.diffs,
                visible_file_indices: &visible_file_indices,
                reviewed_file_paths: &self.reviewed_file_paths,
                review_threads: &self.review_state.review_threads,
                review_composer: self.review_state.review_composer_state.inline_composer(),
            },
            file_index,
        )
    }

    fn sync_diff_list_items(&mut self, cx: &App) {
        let visible_file_indices = self.visible_file_indices(cx);
        let next_items = continuous_diff_items(ContinuousDiffLayoutInput {
            files: &self.detail_state.files,
            diffs: &self.detail_state.diffs,
            visible_file_indices: &visible_file_indices,
            reviewed_file_paths: &self.reviewed_file_paths,
            review_threads: &self.review_state.review_threads,
            review_composer: self.review_state.review_composer_state.inline_composer(),
        });
        sync_diff_list_state(&self.diff_list_state, &mut self.diff_list_items, next_items);
    }

    pub(super) fn scroll_diff_list_to_item(&mut self, item_index: usize) {
        self.diff_list_state.scroll_to(ListOffset {
            item_ix: item_index,
            offset_in_item: px(0.0),
        });
    }

    pub(super) fn reset_diff_list_scroll(&mut self) {
        self.scroll_diff_list_to_item(0);
    }

    fn ensure_active_file_visible(&mut self, cx: &mut Context<Self>) {
        let visible_files = self.visible_file_indices(cx);
        if visible_files.is_empty() || visible_files.contains(&self.active_file_index()) {
            return;
        }

        if let Some(file_index) = visible_files.first().copied() {
            self.select_diff_file_index(file_index);
            self.clear_review_composer_state();
            self.sync_diff_list_items(cx);
            if let Some(row_index) = self.file_tree_row_index_for_file(file_index, cx) {
                self.file_list_scroll
                    .scroll_to_item(row_index, ScrollStrategy::Center);
            }
            if let Some(item_index) = self.diff_item_index_for_file(file_index, cx) {
                self.scroll_diff_list_to_item(item_index);
            } else {
                self.reset_diff_list_scroll();
            }
        }
    }

    fn prune_reviewed_file_paths(&mut self) {
        let file_paths = self
            .detail_state
            .files
            .iter()
            .map(|file| file.path.clone())
            .collect::<HashSet<_>>();
        self.reviewed_file_paths
            .retain(|path| file_paths.contains(path));
        self.owned_file_paths
            .retain(|path| file_paths.contains(path));
    }

    fn reset_changed_file_filters(&mut self) {
        self.excluded_file_type_filters.clear();
        self.show_files_owned_by_current_user = false;
    }

    fn selected_workflow_run_for_logs(&self) -> Option<&WorkflowRun> {
        self.detail_state
            .workflow_runs
            .iter()
            .find(|run| workflow_run_failed(run))
            .or_else(|| self.detail_state.workflow_runs.first())
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
        self.reviewed_file_paths.clear();
        self.reset_changed_file_filters();
        self.owned_file_paths.clear();
        self.clear_detail_loaded_state();
        self.detail_state.workflow_jobs.clear();
        self.clear_log_content();
        self.clear_review_data_state();
        self.review_state.clear_reviews_error();
        self.clear_log_error();
        self.action_runtime.clear_pull_request_action_error();
        self.review_state.clear_submission_errors();
        self.pr_list_scroll
            .scroll_to_item(index, ScrollStrategy::Center);
        self.file_list_scroll.scroll_to_item(0, ScrollStrategy::Top);
        self.reset_diff_list_scroll();
        self.review_list_scroll
            .scroll_to_item(0, ScrollStrategy::Top);
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

    pub(crate) fn select_file(&mut self, index: usize, cx: &mut Context<Self>) {
        if let Some(path) = self
            .detail_state
            .files
            .get(index)
            .map(|file| file.path.clone())
        {
            self.select_diff_file_index(index);
            self.active_tab = PanelTab::Diff;
            self.clear_review_composer_state();
            self.sync_diff_list_items(cx);
            if let Some(row_index) = self.file_tree_row_index_for_file(index, cx) {
                self.file_list_scroll
                    .scroll_to_item(row_index, ScrollStrategy::Center);
            }
            if let Some(item_index) = self.diff_item_index_for_file(index, cx) {
                self.scroll_diff_list_to_item(item_index);
            }
            self.status = format!("Selected {path}");
        }

        cx.notify();
    }

    pub(crate) fn toggle_changed_file_folder(
        &mut self,
        folder_path: String,
        cx: &mut Context<Self>,
    ) {
        let status = if self.collapsed_file_tree_folders.remove(&folder_path) {
            format!("Expanded {folder_path}")
        } else {
            self.collapsed_file_tree_folders.insert(folder_path.clone());
            format!("Collapsed {folder_path}")
        };

        self.ensure_active_file_visible(cx);
        self.status = status;
        cx.notify();
    }

    pub(crate) fn toggle_changed_file_reviewed(
        &mut self,
        file_index: usize,
        cx: &mut Context<Self>,
    ) {
        let Some(path) = self
            .detail_state
            .files
            .get(file_index)
            .map(|file| file.path.clone())
        else {
            self.status = "No changed file to mark reviewed".to_string();
            cx.notify();
            return;
        };

        let reviewed = if self.reviewed_file_paths.remove(&path) {
            false
        } else {
            self.reviewed_file_paths.insert(path.clone());
            true
        };
        let reviewed_count = self.reviewed_file_count();
        let total_count = self.detail_state.files.len();

        self.status = if reviewed {
            format!("Marked {path} as reviewed ({reviewed_count}/{total_count})")
        } else {
            format!("Marked {path} as unreviewed ({reviewed_count}/{total_count})")
        };
        cx.notify();
    }

    pub(crate) fn toggle_changed_file_type_filter(
        &mut self,
        file_type: String,
        cx: &mut Context<Self>,
    ) {
        let included = if self.excluded_file_type_filters.remove(&file_type) {
            true
        } else {
            self.excluded_file_type_filters.insert(file_type.clone());
            false
        };
        let visible_count = self.visible_file_indices(cx).len();

        self.ensure_active_file_visible(cx);
        self.status = if included {
            format!("Included {file_type} files ({visible_count} visible)")
        } else {
            format!("Excluded {file_type} files ({visible_count} visible)")
        };
        cx.notify();
    }

    pub(crate) fn include_all_changed_file_types(&mut self, cx: &mut Context<Self>) {
        self.excluded_file_type_filters.clear();
        self.ensure_active_file_visible(cx);
        let visible_count = self.visible_file_indices(cx).len();
        self.status = format!("Included all file types ({visible_count} visible)");
        cx.notify();
    }

    pub(crate) fn show_all_changed_files(&mut self, cx: &mut Context<Self>) {
        self.show_files_owned_by_current_user = false;
        self.ensure_active_file_visible(cx);
        let visible_count = self.visible_file_indices(cx).len();
        self.status = format!("Showing all changed files ({visible_count} visible)");
        cx.notify();
    }

    pub(crate) fn toggle_files_owned_by_current_user_filter(&mut self, cx: &mut Context<Self>) {
        if !self.has_owned_file_filter_data() {
            self.status = "No owned-file metadata is available for this pull request".to_string();
            cx.notify();
            return;
        }

        self.show_files_owned_by_current_user = !self.show_files_owned_by_current_user;
        self.ensure_active_file_visible(cx);
        let visible_count = self.visible_file_indices(cx).len();

        self.status = if self.show_files_owned_by_current_user {
            format!("Showing {visible_count} files owned by you")
        } else {
            format!("Showing {visible_count} changed files")
        };
        cx.notify();
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

    pub(crate) fn refresh_owned_file_filters(&mut self, cx: &mut Context<Self>) {
        let Some(current_user_login) = self.review_state.current_user_login.clone() else {
            self.owned_file_paths.clear();
            self.show_files_owned_by_current_user = false;
            cx.notify();
            return;
        };
        let Some(repository_path) = self.current_repository_local_path().cloned() else {
            self.owned_file_paths.clear();
            self.show_files_owned_by_current_user = false;
            cx.notify();
            return;
        };
        if self.detail_state.files.is_empty() {
            self.owned_file_paths.clear();
            self.show_files_owned_by_current_user = false;
            cx.notify();
            return;
        }

        let files = self.detail_state.files.clone();
        let selected_repository = self.current_repository().cloned();
        let selected_pr_number = self.selected_pull_request_number();
        let task = cx.background_spawn(async move {
            codeowners_owned_file_paths(&repository_path, &files, &current_user_login)
        });

        cx.spawn(async move |this, cx| {
            let result = task.await;

            if let Err(error) = this.update(cx, move |view, cx| {
                if view.current_repository().cloned() != selected_repository
                    || view.selected_pull_request_number() != selected_pr_number
                {
                    return;
                }

                match result {
                    Ok(paths) => {
                        view.owned_file_paths = paths;
                    }
                    Err(_) => {
                        view.owned_file_paths.clear();
                    }
                }

                if !view.has_owned_file_filter_data() {
                    view.show_files_owned_by_current_user = false;
                }
                view.ensure_active_file_visible(cx);
                cx.notify();
            }) {
                crate::workspace::log_entity_update_error(
                    "failed to update file ownership filters",
                    error,
                );
            }
        })
        .detach();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::actions::{CloseSettings, OpenSettings, ToggleRepositorySwitcher};
    use gpui::TestAppContext;
    use gpui_component::{Root, Theme, ThemeMode};

    #[test]
    fn defaults_pull_request_inbox_to_open_mode() {
        assert_eq!(PullRequestInboxMode::default(), PullRequestInboxMode::Open);
        assert_eq!(PullRequestInboxMode::Open.label(), "Open");
        assert_eq!(PullRequestInboxMode::Closed.label(), "Closed");
        assert_eq!(PullRequestInboxMode::NeedsReview.label(), "Needs review");
        assert_eq!(
            PullRequestInboxMode::Closed.empty_message(),
            "No closed pull requests"
        );
    }

    #[gpui::test]
    async fn repository_switcher_starts_closed(cx: &mut TestAppContext) {
        cx.update(|cx| {
            gpui_component::init(cx);
            Theme::change(ThemeMode::Dark, None, cx);
        });

        let mut view_entity = None;
        let (_, cx) = cx.add_window_view(|window, cx| {
            let view = cx.new(|cx| AppView::new_without_startup_tasks(window, cx));
            view_entity = Some(view.clone());
            Root::new(view, window, cx)
        });

        view_entity
            .expect("test AppView should be created")
            .read_with(cx, |view, _| {
                assert!(!view.repository_state.repository_switcher_open);
            });
    }

    #[gpui::test]
    async fn repository_switcher_does_not_open_while_auth_gate_is_visible(cx: &mut TestAppContext) {
        cx.update(|cx| {
            gpui_component::init(cx);
            Theme::change(ThemeMode::Dark, None, cx);
        });

        let mut view_entity = None;
        let (_, cx) = cx.add_window_view(|window, cx| {
            let view = cx.new(|cx| AppView::new_without_startup_tasks(window, cx));
            view.update(cx, |view, cx| {
                view.auth_status = GitHubAuthStatus::SignedOut;
                view.toggle_repository_switcher(&ToggleRepositorySwitcher, window, cx);
                assert!(!view.repository_state.repository_switcher_open);
            });
            view_entity = Some(view.clone());
            Root::new(view, window, cx)
        });

        view_entity
            .expect("test AppView should be created")
            .read_with(cx, |view, _| {
                assert!(!view.repository_state.repository_switcher_open);
            });
    }

    #[gpui::test]
    async fn dismissing_github_auth_popover_keeps_pending_sign_in(cx: &mut TestAppContext) {
        cx.update(|cx| {
            gpui_component::init(cx);
            Theme::change(ThemeMode::Dark, None, cx);
        });

        let mut view_entity = None;
        let (_, cx) = cx.add_window_view(|window, cx| {
            let view = cx.new(|cx| AppView::new_without_startup_tasks(window, cx));
            view.update(cx, |view, cx| {
                view.auth_status = GitHubAuthStatus::SigningIn {
                    user_code: "8F0B-1F01".to_string(),
                    verification_uri: "https://github.com/login/device".to_string(),
                };
                view.github_auth_popover_open = true;
                view.tasks
                    .set_auth_task(cx.spawn(async move |_view, _cx| {}));

                view.dismiss_github_auth_popover(cx);

                assert_eq!(
                    view.auth_status,
                    GitHubAuthStatus::SigningIn {
                        user_code: "8F0B-1F01".to_string(),
                        verification_uri: "https://github.com/login/device".to_string(),
                    }
                );
                assert!(!view.github_auth_popover_open());
                assert!(view.tasks.has_auth_task());
                assert_eq!(view.status, "Waiting for GitHub authorization");
            });
            view_entity = Some(view.clone());
            Root::new(view, window, cx)
        });

        view_entity
            .expect("test AppView should be created")
            .read_with(cx, |view, _| {
                assert!(matches!(
                    view.auth_status,
                    GitHubAuthStatus::SigningIn { .. }
                ));
            });
    }

    #[gpui::test]
    async fn github_account_settings_open_and_close(cx: &mut TestAppContext) {
        cx.update(|cx| {
            gpui_component::init(cx);
            Theme::change(ThemeMode::Dark, None, cx);
        });

        let mut view_entity = None;
        let (_, cx) = cx.add_window_view(|window, cx| {
            let view = cx.new(|cx| AppView::new_without_startup_tasks(window, cx));
            view.update(cx, |view, cx| {
                view.github_auth_popover_open = true;
                view.open_settings(&OpenSettings, window, cx);

                assert!(view.settings_open());
                assert_eq!(view.settings_section(), SettingsSection::GitHub);
                assert!(!view.github_auth_popover_open);

                view.close_settings(&CloseSettings, window, cx);
                assert!(!view.settings_open());
            });
            view_entity = Some(view.clone());
            Root::new(view, window, cx)
        });

        view_entity
            .expect("test AppView should be created")
            .read_with(cx, |view, _| {
                assert!(!view.settings_open());
            });
    }

    #[gpui::test]
    async fn pending_auth_switch_preserves_current_source(cx: &mut TestAppContext) {
        cx.update(|cx| {
            gpui_component::init(cx);
            Theme::change(ThemeMode::Dark, None, cx);
        });

        let mut view_entity = None;
        let (_, cx) = cx.add_window_view(|window, cx| {
            let view = cx.new(|cx| AppView::new_without_startup_tasks(window, cx));
            view.update(cx, |view, _| {
                view.auth_status = GitHubAuthStatus::SignedIn {
                    login: Some("octocat".to_string()),
                    source: GitHubAuthSource::GhCli,
                };
                view.auth_switch_status = Some(AuthSwitchStatus::StartingOAuth);

                assert_eq!(
                    view.current_github_auth_source(),
                    Some(GitHubAuthSource::GhCli)
                );
                assert_eq!(
                    view.auth_switch_status(),
                    Some(&AuthSwitchStatus::StartingOAuth)
                );
            });
            view_entity = Some(view.clone());
            Root::new(view, window, cx)
        });

        view_entity
            .expect("test AppView should be created")
            .read_with(cx, |view, _| {
                assert_eq!(
                    view.current_github_auth_source(),
                    Some(GitHubAuthSource::GhCli)
                );
            });
    }
}
