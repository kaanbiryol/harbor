mod action_commands;
mod cache;
mod changed_files;
mod commands;
mod loaders;
mod local_commands;
mod pull_request_detail_loaders;
mod render;
mod review_data_loaders;
mod review_interactions;
mod review_state;
mod review_submissions;
mod reviews;
mod switchers;
mod workflow_log_loaders;

use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
};

use gpui::{
    App, AppContext, Context, Entity, FocusHandle, ScrollStrategy, Subscription, Task,
    UniformListScrollHandle, Window,
};
use gpui_component::{ActiveTheme, input::InputState};
use harbor_domain::{
    CheckRun, DiffFile, PullRequest, PullRequestReview, RepoId, ReviewThread, ReviewThreadState,
    WorkflowJob, WorkflowRun,
};
use harbor_logs::LogChunk;
use harbor_storage::SqliteStore;

use crate::actions::PanelTab;
use crate::diff::{ParsedDiff, parse_files_with_syntax};
use crate::panels::{continuous_diff_file_row_index, workflow_run_failed};

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
#[cfg(test)]
pub(crate) use changed_files::{
    changed_file_matches_filters, changed_file_matches_query, changed_file_type_key,
};
use reviews::ReviewReactionKey;
pub(crate) use reviews::{
    PendingReviewSession, ReviewCommentSubmission, ReviewCommentUiError, ReviewComposer,
    ReviewLineSelection, ReviewLineTarget, ReviewReactionAction, ReviewThreadUiError,
    review_comment_pending_sync, review_range_from_targets, review_reaction,
};
#[cfg(test)]
pub(crate) use switchers::{
    next_switcher_index, pull_request_matches_query, repository_matches_query,
    repository_switcher_accepted_repository,
};
pub(crate) use switchers::{normalized_search_query, parse_repo_id};

#[cfg(test)]
pub(crate) use commands::github_file_url;
#[cfg(test)]
pub(crate) use local_commands::{OpenTargetStatus, open_target_for_app};
#[cfg(test)]
pub(crate) use render::header::open_with_app_disabled;

#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub(crate) enum PullRequestInboxMode {
    #[default]
    Open,
    Closed,
    NeedsReview,
}

impl PullRequestInboxMode {
    pub(crate) const ALL: [Self; 3] = [Self::Open, Self::Closed, Self::NeedsReview];

    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Open => "Open",
            Self::Closed => "Closed",
            Self::NeedsReview => "Needs review",
        }
    }

    pub(crate) fn status_label(self) -> &'static str {
        match self {
            Self::Open => "open pull requests",
            Self::Closed => "closed pull requests",
            Self::NeedsReview => "pull requests requesting your review",
        }
    }

    pub(crate) fn empty_message(self) -> &'static str {
        match self {
            Self::Open => "No open pull requests",
            Self::Closed => "No closed pull requests",
            Self::NeedsReview => "No pull requests require your review",
        }
    }

    pub(crate) fn key(self) -> &'static str {
        match self {
            Self::Open => "open",
            Self::Closed => "closed",
            Self::NeedsReview => "needs-review",
        }
    }
}

pub struct AppView {
    focus_handle: FocusHandle,
    pull_requests: Vec<PullRequest>,
    repositories: Vec<RepoId>,
    files: Vec<DiffFile>,
    diffs: Vec<Option<ParsedDiff>>,
    check_runs: Vec<CheckRun>,
    workflow_runs: Vec<WorkflowRun>,
    workflow_jobs: Vec<WorkflowJob>,
    pull_request_reviews: Vec<PullRequestReview>,
    pub(crate) review_threads: Vec<ReviewThread>,
    pub(crate) review_composer: Option<ReviewComposer>,
    pub(crate) review_line_selection: Option<ReviewLineSelection>,
    pub(crate) pending_review: Option<PendingReviewSession>,
    pub(crate) review_comment_input: Entity<InputState>,
    pub(crate) review_thread_reply_thread_id: Option<String>,
    pub(crate) review_thread_reply_input: Entity<InputState>,
    pub(crate) review_comment_edit_comment_id: Option<String>,
    pub(crate) review_comment_edit_input: Entity<InputState>,
    pub(crate) pending_review_body_input: Entity<InputState>,
    pub(crate) log_chunk: Option<LogChunk>,
    pr_list_task: Option<Task<()>>,
    pr_detail_tasks: Vec<Task<()>>,
    log_task: Option<Task<()>>,
    repository_task: Option<Task<()>>,
    local_task: Option<Task<()>>,
    pr_list_scroll: UniformListScrollHandle,
    file_list_scroll: UniformListScrollHandle,
    diff_list_scroll: UniformListScrollHandle,
    review_list_scroll: UniformListScrollHandle,
    log_list_scroll: UniformListScrollHandle,
    selected_pr: usize,
    active_file: usize,
    pub(crate) active_hunk: usize,
    active_tab: PanelTab,
    pull_request_inbox_visible: bool,
    repository_switcher_open: bool,
    pull_request_switcher_open: bool,
    file_filter_popover_open: bool,
    repository_switcher_selection: usize,
    pull_request_switcher_selection: usize,
    repository_search_input: Entity<InputState>,
    pull_request_search_input: Entity<InputState>,
    pull_request_inbox_mode: PullRequestInboxMode,
    pull_request_inbox_cache: HashMap<PullRequestInboxCacheKey, PullRequestInboxSnapshot>,
    pull_request_detail_cache: HashMap<PullRequestDetailCacheKey, PullRequestDetailSnapshot>,
    configured_repo: Option<RepoId>,
    repository_store: Option<SqliteStore>,
    repository_local_paths: HashMap<RepoId, PathBuf>,
    collapsed_file_tree_folders: HashSet<String>,
    reviewed_file_paths: HashSet<String>,
    excluded_file_type_filters: HashSet<String>,
    show_files_owned_by_current_user: bool,
    owned_file_paths: HashSet<String>,
    is_loading_repositories: bool,
    is_loading_prs: bool,
    is_loading_details: bool,
    is_loading_files: bool,
    is_loading_checks: bool,
    is_loading_workflows: bool,
    is_loading_reviews: bool,
    is_loading_logs: bool,
    is_running_action: bool,
    is_running_pr_action: bool,
    pub(crate) is_submitting_review_comment: bool,
    pub(crate) is_submitting_review_thread_reply: bool,
    pub(crate) is_submitting_review_comment_edit: bool,
    pub(crate) is_submitting_pending_review: bool,
    pub(crate) review_thread_action_thread_id: Option<String>,
    pub(crate) review_comment_action_comment_id: Option<String>,
    pub(crate) review_reaction_action: Option<ReviewReactionAction>,
    review_thread_state_overrides: HashMap<String, ReviewThreadState>,
    review_reaction_overrides: HashMap<ReviewReactionKey, bool>,
    load_error: Option<String>,
    details_error: Option<String>,
    files_error: Option<String>,
    checks_error: Option<String>,
    workflows_error: Option<String>,
    reviews_error: Option<String>,
    logs_error: Option<String>,
    repository_error: Option<String>,
    action_error: Option<String>,
    pr_action_error: Option<String>,
    pub(crate) review_comment_error: Option<String>,
    pub(crate) review_thread_reply_error: Option<ReviewThreadUiError>,
    pub(crate) review_thread_action_error: Option<ReviewThreadUiError>,
    pub(crate) review_comment_edit_error: Option<ReviewCommentUiError>,
    pub(crate) review_comment_action_error: Option<ReviewCommentUiError>,
    pub(crate) review_reaction_error: Option<ReviewCommentUiError>,
    pub(crate) pending_review_error: Option<String>,
    current_user_login: Option<String>,
    local_review_comment_sequence: u64,
    review_data_generation: u64,
    did_focus: bool,
    status: String,
    _subscriptions: Vec<Subscription>,
}

impl AppView {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
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
                .placeholder("Search pull requests...")
                .clean_on_escape()
        });
        let subscriptions = vec![
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
        let diffs = parse_files_with_syntax(&files, &cx.theme().highlight_theme);
        let repositories = Vec::new();
        let status = "Fetching repositories from GitHub...".to_string();

        let mut view = Self {
            focus_handle: cx.focus_handle(),
            pull_requests,
            repositories,
            files,
            diffs,
            check_runs: Vec::new(),
            workflow_runs: Vec::new(),
            workflow_jobs: Vec::new(),
            pull_request_reviews,
            review_threads,
            review_composer: None,
            review_line_selection: None,
            pending_review: None,
            review_comment_input,
            review_thread_reply_thread_id: None,
            review_thread_reply_input,
            review_comment_edit_comment_id: None,
            review_comment_edit_input,
            pending_review_body_input,
            log_chunk: None,
            pr_list_task: None,
            pr_detail_tasks: Vec::new(),
            log_task: None,
            repository_task: None,
            local_task: None,
            pr_list_scroll: UniformListScrollHandle::new(),
            file_list_scroll: UniformListScrollHandle::new(),
            diff_list_scroll: UniformListScrollHandle::new(),
            review_list_scroll: UniformListScrollHandle::new(),
            log_list_scroll: UniformListScrollHandle::new(),
            selected_pr: 0,
            active_file: 0,
            active_hunk: 0,
            active_tab: PanelTab::Diff,
            pull_request_inbox_visible: true,
            repository_switcher_open: true,
            pull_request_switcher_open: false,
            file_filter_popover_open: false,
            repository_switcher_selection: 0,
            pull_request_switcher_selection: 0,
            repository_search_input,
            pull_request_search_input,
            pull_request_inbox_mode: PullRequestInboxMode::default(),
            pull_request_inbox_cache: HashMap::new(),
            pull_request_detail_cache: HashMap::new(),
            configured_repo: None,
            repository_store: None,
            repository_local_paths: HashMap::new(),
            collapsed_file_tree_folders: HashSet::new(),
            reviewed_file_paths: HashSet::new(),
            excluded_file_type_filters: HashSet::new(),
            show_files_owned_by_current_user: false,
            owned_file_paths: HashSet::new(),
            is_loading_repositories: true,
            is_loading_prs: false,
            is_loading_details: false,
            is_loading_files: false,
            is_loading_checks: false,
            is_loading_workflows: false,
            is_loading_reviews: false,
            is_loading_logs: false,
            is_running_action: false,
            is_running_pr_action: false,
            is_submitting_review_comment: false,
            is_submitting_review_thread_reply: false,
            is_submitting_review_comment_edit: false,
            is_submitting_pending_review: false,
            review_thread_action_thread_id: None,
            review_comment_action_comment_id: None,
            review_reaction_action: None,
            review_thread_state_overrides: HashMap::new(),
            review_reaction_overrides: HashMap::new(),
            load_error: None,
            details_error: None,
            files_error: None,
            checks_error: None,
            workflows_error: None,
            reviews_error: None,
            logs_error: None,
            repository_error: None,
            action_error: None,
            pr_action_error: None,
            review_comment_error: None,
            review_thread_reply_error: None,
            review_thread_action_error: None,
            review_comment_edit_error: None,
            review_comment_action_error: None,
            review_reaction_error: None,
            pending_review_error: None,
            current_user_login: None,
            local_review_comment_sequence: 0,
            review_data_generation: 0,
            did_focus: false,
            status,
            _subscriptions: subscriptions,
        };

        view.load_recent_repositories(cx);

        view
    }

    fn selected_pull_request(&self) -> Option<&PullRequest> {
        self.pull_requests.get(self.selected_pr)
    }

    fn selected_pull_request_number(&self) -> Option<u64> {
        self.selected_pull_request().map(|pr| pr.number)
    }

    fn selected_pr_label(&self) -> String {
        self.selected_pull_request()
            .map(|pr| format!("PR #{}", pr.number))
            .unwrap_or_else(|| "no selected pull request".to_string())
    }

    pub(crate) fn active_file(&self) -> Option<&DiffFile> {
        self.files.get(self.active_file)
    }

    pub(crate) fn active_file_index(&self) -> usize {
        self.active_file
    }

    pub(crate) fn diff_files(&self) -> &[DiffFile] {
        &self.files
    }

    pub(crate) fn parsed_diffs(&self) -> &[Option<ParsedDiff>] {
        &self.diffs
    }

    pub(crate) fn reviewed_file_paths(&self) -> &HashSet<String> {
        &self.reviewed_file_paths
    }

    pub(crate) fn changed_file_tree_rows(&self, _cx: &App) -> Vec<ChangedFileTreeRow> {
        let filters = self.changed_file_filters();

        changed_file_tree_rows(
            &self.files,
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
        self.files
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
        changed_file_type_filters(&self.files, &self.excluded_file_type_filters)
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

    fn diff_row_index_for_file(&self, file_index: usize, cx: &App) -> Option<usize> {
        let visible_file_indices = self.visible_file_indices(cx);

        continuous_diff_file_row_index(
            &self.files,
            &self.diffs,
            &visible_file_indices,
            &self.reviewed_file_paths,
            file_index,
            &self.review_threads,
            self.review_composer.as_ref(),
            self.review_comment_error.as_deref(),
            self.review_thread_reply_thread_id.as_deref(),
            self.review_comment_edit_comment_id.as_deref(),
        )
    }

    fn ensure_active_file_visible(&mut self, cx: &mut Context<Self>) {
        let visible_files = self.visible_file_indices(cx);
        if visible_files.is_empty() || visible_files.contains(&self.active_file) {
            return;
        }

        if let Some(file_index) = visible_files.first().copied() {
            self.active_file = file_index;
            self.active_hunk = 0;
            self.clear_review_composer_state();
            if let Some(row_index) = self.file_tree_row_index_for_file(file_index, cx) {
                self.file_list_scroll
                    .scroll_to_item(row_index, ScrollStrategy::Center);
            }
            if let Some(row_index) = self.diff_row_index_for_file(file_index, cx) {
                self.diff_list_scroll
                    .scroll_to_item(row_index, ScrollStrategy::Top);
            } else {
                self.diff_list_scroll.scroll_to_item(0, ScrollStrategy::Top);
            }
        }
    }

    fn prune_reviewed_file_paths(&mut self) {
        let file_paths = self
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
        self.workflow_runs
            .iter()
            .find(|run| workflow_run_failed(run))
            .or_else(|| self.workflow_runs.first())
    }

    pub(crate) fn select_pull_request(&mut self, index: usize, cx: &mut Context<Self>) {
        if index >= self.pull_requests.len() {
            self.status = "No pull requests to select".to_string();
            cx.notify();
            return;
        }

        self.cache_current_pull_request_detail_snapshot();
        self.selected_pr = index;

        if self.restore_selected_pull_request_detail_snapshot(cx) {
            return;
        }

        self.active_file = 0;
        self.active_hunk = 0;
        self.collapsed_file_tree_folders.clear();
        self.reviewed_file_paths.clear();
        self.reset_changed_file_filters();
        self.owned_file_paths.clear();
        self.workflow_jobs.clear();
        self.log_chunk = None;
        self.pull_request_reviews.clear();
        self.review_threads.clear();
        self.clear_review_composer_state();
        self.pending_review = None;
        self.reviews_error = None;
        self.logs_error = None;
        self.pr_action_error = None;
        self.review_comment_error = None;
        self.pending_review_error = None;
        self.pr_list_scroll
            .scroll_to_item(index, ScrollStrategy::Center);
        self.file_list_scroll.scroll_to_item(0, ScrollStrategy::Top);
        self.diff_list_scroll.scroll_to_item(0, ScrollStrategy::Top);
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
        if self.pull_request_inbox_mode == mode {
            return;
        }

        if let Some(repository) = self.configured_repo.clone() {
            self.load_repository_pull_requests_from_cache(repository, mode, cx);
        } else {
            self.pull_request_inbox_mode = mode;
            self.pull_requests.clear();
            self.files.clear();
            self.diffs.clear();
            self.collapsed_file_tree_folders.clear();
            self.reviewed_file_paths.clear();
            self.reset_changed_file_filters();
            self.owned_file_paths.clear();
            self.check_runs.clear();
            self.workflow_runs.clear();
            self.workflow_jobs.clear();
            self.pull_request_reviews.clear();
            self.review_threads.clear();
            self.clear_review_composer_state();
            self.pending_review = None;
            self.log_chunk = None;
            self.selected_pr = 0;
            self.active_file = 0;
            self.active_hunk = 0;
            self.status =
                "Select a repository from the header before loading pull requests".to_string();
            cx.notify();
        }
    }

    pub(crate) fn select_file(&mut self, index: usize, cx: &mut Context<Self>) {
        if let Some(path) = self.files.get(index).map(|file| file.path.clone()) {
            self.active_file = index;
            self.active_hunk = 0;
            self.active_tab = PanelTab::Diff;
            self.clear_review_composer_state();
            if let Some(row_index) = self.file_tree_row_index_for_file(index, cx) {
                self.file_list_scroll
                    .scroll_to_item(row_index, ScrollStrategy::Center);
            }
            if let Some(row_index) = self.diff_row_index_for_file(index, cx) {
                self.diff_list_scroll
                    .scroll_to_item(row_index, ScrollStrategy::Top);
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
        let Some(path) = self.files.get(file_index).map(|file| file.path.clone()) else {
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
        let total_count = self.files.len();

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
        self.repositories.retain(|existing| existing != &repository);
        self.repositories.insert(0, repository);
    }

    pub(crate) fn current_repository(&self) -> Option<&RepoId> {
        self.configured_repo.as_ref().or_else(|| {
            self.selected_pull_request()
                .map(|pull_request| &pull_request.repo)
        })
    }

    pub(crate) fn current_repository_local_path(&self) -> Option<&PathBuf> {
        self.current_repository()
            .and_then(|repository| self.repository_local_paths.get(repository))
    }

    pub(crate) fn set_repository_local_path(&mut self, repository: RepoId, path: PathBuf) {
        self.repository_local_paths.insert(repository, path);
    }

    pub(crate) fn refresh_owned_file_filters(&mut self, cx: &mut Context<Self>) {
        let Some(current_user_login) = self.current_user_login.clone() else {
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
        if self.files.is_empty() {
            self.owned_file_paths.clear();
            self.show_files_owned_by_current_user = false;
            cx.notify();
            return;
        }

        let files = self.files.clone();
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
                eprintln!("failed to update file ownership filters: {error}");
            }
        })
        .detach();
    }
}
