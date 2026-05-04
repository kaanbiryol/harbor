mod commands;
mod loaders;
mod render;

use std::{
    collections::{BTreeMap, HashMap, HashSet},
    fs,
    path::{Path, PathBuf},
};

use gpui::{
    App, AppContext, Context, Entity, FocusHandle, ScrollStrategy, Subscription, Task,
    UniformListScrollHandle, Window,
};
use gpui_component::input::{InputEvent, InputState};
use harbor_domain::{
    CheckRun, DiffFile, FileStatus, PullRequest, PullRequestReview, PullRequestReviewState, RepoId,
    ReviewCommentRange, ReviewSide, ReviewThread, ReviewThreadState, WorkflowJob, WorkflowRun,
};
use harbor_github::{GhCliTransport, GitHubClient, SubmitPullRequestReviewEvent};
use harbor_logs::LogChunk;
use harbor_storage::SqliteStore;

use crate::actions::{DEFAULT_REQUEST_CHANGES_BODY, PanelTab};
use crate::diff::{ParsedDiff, parse_files};
use crate::panels::workflow_run_failed;

#[cfg(test)]
pub(crate) use commands::{OpenTargetStatus, github_file_url, open_target_for_app};
#[cfg(test)]
pub(crate) use render::open_with_app_disabled;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ReviewLineTarget {
    pub(crate) hunk_index: usize,
    pub(crate) line_index: usize,
    pub(crate) range: ReviewCommentRange,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ReviewComposer {
    pub(crate) anchor: ReviewLineTarget,
    pub(crate) range: ReviewCommentRange,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ReviewLineSelection {
    pub(crate) anchor: ReviewLineTarget,
    pub(crate) current: ReviewLineTarget,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PendingReviewSession {
    pub(crate) node_id: String,
    pub(crate) comment_count: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ReviewCommentSubmission {
    SingleComment,
    StartReview,
    AddToReview,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ReviewThreadUiError {
    pub(crate) thread_id: String,
    pub(crate) message: String,
}

#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub(crate) enum PullRequestInboxMode {
    #[default]
    Open,
    Closed,
    NeedsReview,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub(crate) struct PullRequestInboxCacheKey {
    repository: RepoId,
    mode: PullRequestInboxMode,
}

impl PullRequestInboxCacheKey {
    pub(crate) fn new(repository: RepoId, mode: PullRequestInboxMode) -> Self {
        Self { repository, mode }
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
    pull_request: PullRequest,
    files: Vec<DiffFile>,
    diffs: Vec<Option<ParsedDiff>>,
    check_runs: Vec<CheckRun>,
    workflow_runs: Vec<WorkflowRun>,
    workflow_jobs: Vec<WorkflowJob>,
    pull_request_reviews: Vec<PullRequestReview>,
    review_threads: Vec<ReviewThread>,
    pending_review: Option<PendingReviewSession>,
    log_chunk: Option<LogChunk>,
    current_user_login: Option<String>,
    collapsed_file_tree_folders: HashSet<String>,
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
    files: Vec<DiffFile>,
    diffs: Vec<Option<ParsedDiff>>,
    check_runs: Vec<CheckRun>,
    workflow_runs: Vec<WorkflowRun>,
    workflow_jobs: Vec<WorkflowJob>,
    pull_request_reviews: Vec<PullRequestReview>,
    review_threads: Vec<ReviewThread>,
    pending_review: Option<PendingReviewSession>,
    log_chunk: Option<LogChunk>,
    current_user_login: Option<String>,
    collapsed_file_tree_folders: HashSet<String>,
    reviewed_file_paths: HashSet<String>,
    excluded_file_type_filters: HashSet<String>,
    show_files_owned_by_current_user: bool,
    owned_file_paths: HashSet<String>,
    selected_pr: usize,
    active_file: usize,
    active_hunk: usize,
    active_tab: PanelTab,
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

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum ChangedFileTreeRow {
    Folder(ChangedFileFolderRow),
    File(ChangedFileRow),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ChangedFileFolderRow {
    pub(crate) path: String,
    pub(crate) name: String,
    pub(crate) depth: usize,
    pub(crate) file_count: usize,
    pub(crate) reviewed_file_count: usize,
    pub(crate) expanded: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ChangedFileRow {
    pub(crate) file_index: usize,
    pub(crate) name: String,
    pub(crate) depth: usize,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct ChangedFileFilters {
    pub(crate) query: String,
    pub(crate) excluded_file_types: HashSet<String>,
    pub(crate) owned_by_current_user_only: bool,
    pub(crate) owned_file_paths: HashSet<String>,
}

impl ChangedFileFilters {
    fn has_active_filter(&self) -> bool {
        !self.query.is_empty()
            || !self.excluded_file_types.is_empty()
            || self.owned_by_current_user_only
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ChangedFileTypeFilter {
    pub(crate) key: String,
    pub(crate) label: String,
    pub(crate) file_count: usize,
    pub(crate) included: bool,
}

#[derive(Default)]
struct ChangedFileTreeNode {
    folders: BTreeMap<String, ChangedFileTreeNode>,
    files: Vec<usize>,
    file_count: usize,
    reviewed_file_count: usize,
}

impl ChangedFileTreeNode {
    fn add_file(&mut self, file_index: usize, path_segments: &[&str], reviewed: bool) {
        self.file_count += 1;
        if reviewed {
            self.reviewed_file_count += 1;
        }

        let Some((next_segment, remaining_segments)) = path_segments.split_first() else {
            self.files.push(file_index);
            return;
        };

        if remaining_segments.is_empty() {
            self.files.push(file_index);
            return;
        }

        self.folders
            .entry((*next_segment).to_string())
            .or_default()
            .add_file(file_index, remaining_segments, reviewed);
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
    command_palette_open: bool,
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
    pub(crate) is_submitting_pending_review: bool,
    pub(crate) review_thread_action_thread_id: Option<String>,
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
    pub(crate) pending_review_error: Option<String>,
    current_user_login: Option<String>,
    did_focus: bool,
    status: String,
    _subscriptions: Vec<Subscription>,
}

impl AppView {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let configured_repo = configured_repo_from_env();
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
                &pending_review_body_input,
                window,
                Self::on_review_input_event,
            ),
        ];
        let diffs = parse_files(&files);
        let repositories = initial_repositories(configured_repo.as_ref(), &pull_requests);
        let status = configured_repo
            .as_ref()
            .map(|repo| format!("Loading open pull requests from {}", repo.full_name()))
            .unwrap_or_else(|| "Fetching repositories from GitHub...".to_string());
        let show_repository_switcher = configured_repo.is_none();

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
            command_palette_open: false,
            repository_switcher_open: show_repository_switcher,
            pull_request_switcher_open: false,
            file_filter_popover_open: false,
            repository_switcher_selection: 0,
            pull_request_switcher_selection: 0,
            repository_search_input,
            pull_request_search_input,
            pull_request_inbox_mode: PullRequestInboxMode::default(),
            pull_request_inbox_cache: HashMap::new(),
            pull_request_detail_cache: HashMap::new(),
            configured_repo,
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
            is_submitting_pending_review: false,
            review_thread_action_thread_id: None,
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
            pending_review_error: None,
            current_user_login: None,
            did_focus: false,
            status,
            _subscriptions: subscriptions,
        };

        view.load_recent_repositories(cx);

        if let Some(repo) = view.configured_repo.clone() {
            view.load_pull_requests(repo, cx);
        }

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

    pub(crate) fn active_diff(&self) -> Option<&ParsedDiff> {
        self.diffs
            .get(self.active_file)
            .and_then(Option::as_ref)
            .filter(|diff| !diff.is_empty())
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
            self.diff_list_scroll.scroll_to_item(0, ScrollStrategy::Top);
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
            self.diff_list_scroll.scroll_to_item(0, ScrollStrategy::Top);
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

    pub(crate) fn start_review_line_selection(
        &mut self,
        target: ReviewLineTarget,
        cx: &mut Context<Self>,
    ) {
        self.review_line_selection = Some(ReviewLineSelection {
            anchor: target.clone(),
            current: target,
        });
        self.review_composer = None;
        self.review_comment_error = None;
        self.active_tab = PanelTab::Diff;
        self.status = "Started review line selection".to_string();
        cx.notify();
    }

    pub(crate) fn extend_review_line_selection(
        &mut self,
        target: ReviewLineTarget,
        cx: &mut Context<Self>,
    ) {
        if let Some(selection) = self.review_line_selection.as_mut() {
            selection.current = target;
        }
        cx.notify();
    }

    pub(crate) fn finish_review_line_selection(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(selection) = self.review_line_selection.take() else {
            return;
        };

        match review_composer_from_selection(&selection.anchor, &selection.current) {
            Ok(composer) => {
                let range = composer.range.clone();
                let label = review_comment_range_label(&range);
                self.review_comment_input.update(cx, |input, cx| {
                    input.set_value("", window, cx);
                    input.focus(window, cx);
                });
                self.review_composer = Some(composer);
                self.review_comment_error = None;
                self.status = format!("Opened review composer for {label}");
            }
            Err(message) => {
                self.review_composer = None;
                self.review_comment_error = Some(message.clone());
                self.status = message;
            }
        }

        cx.notify();
    }

    pub(crate) fn cancel_review_composer(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.clear_review_composer_state();
        self.review_comment_input.update(cx, |input, cx| {
            input.set_value("", window, cx);
        });
        self.status = "Cancelled review comment".to_string();
        cx.notify();
    }

    pub(crate) fn open_review_thread_reply(
        &mut self,
        thread_id: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.review_thread_reply_thread_id = Some(thread_id);
        self.review_thread_reply_error = None;
        self.review_thread_reply_input.update(cx, |input, cx| {
            input.set_value("", window, cx);
            input.focus(window, cx);
        });
        self.status = "Opened review thread reply".to_string();
        cx.notify();
    }

    pub(crate) fn cancel_review_thread_reply(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.review_thread_reply_thread_id = None;
        self.review_thread_reply_error = None;
        self.review_thread_reply_input.update(cx, |input, cx| {
            input.set_value("", window, cx);
        });
        self.status = "Cancelled review thread reply".to_string();
        cx.notify();
    }

    pub(crate) fn submit_review_thread_reply(&mut self, thread_id: String, cx: &mut Context<Self>) {
        if self.is_submitting_review_thread_reply {
            self.status = "A review thread reply is already being submitted".to_string();
            cx.notify();
            return;
        }

        let Some(pr) = self.selected_pull_request().cloned() else {
            self.review_thread_reply_error = Some(ReviewThreadUiError {
                thread_id,
                message: "Select a pull request before replying".to_string(),
            });
            self.status = "Select a pull request before replying".to_string();
            cx.notify();
            return;
        };

        let body = self.review_thread_reply_input.read(cx).value().to_string();
        let body = body.trim().to_string();
        if body.is_empty() {
            self.review_thread_reply_error = Some(ReviewThreadUiError {
                thread_id,
                message: "Enter a reply before sending".to_string(),
            });
            self.status = "Enter a reply before sending".to_string();
            cx.notify();
            return;
        }

        let pending_review_node_id = self
            .pending_review
            .as_ref()
            .map(|pending_review| pending_review.node_id.clone());

        self.is_submitting_review_thread_reply = true;
        self.review_thread_reply_thread_id = Some(thread_id.clone());
        self.review_thread_reply_error = None;
        self.status = format!("Posting reply on PR #{}", pr.number);
        cx.notify();

        cx.spawn(async move |this, cx| {
            let result = GitHubClient::new(GhCliTransport)
                .add_review_thread_reply(&thread_id, pending_review_node_id.as_deref(), &body)
                .await;

            if let Err(error) = this.update(cx, move |view, cx| {
                view.is_submitting_review_thread_reply = false;

                match result {
                    Ok(()) => {
                        if pending_review_node_id.is_some()
                            && let Some(pending_review) = view.pending_review.as_mut()
                        {
                            pending_review.comment_count += 1;
                        }

                        view.review_thread_reply_thread_id = None;
                        view.review_thread_reply_error = None;
                        view.status = format!("Posted reply on PR #{}", pr.number);
                        view.load_selected_review_data(cx);
                    }
                    Err(error) => {
                        let message = format!("Failed to post reply: {error}");
                        view.review_thread_reply_error = Some(ReviewThreadUiError {
                            thread_id,
                            message: message.clone(),
                        });
                        view.status = message;
                    }
                }

                cx.notify();
            }) {
                eprintln!("failed to update review thread reply state: {error}");
            }
        })
        .detach();
    }

    pub(crate) fn set_review_thread_resolved(
        &mut self,
        thread_id: String,
        resolved: bool,
        cx: &mut Context<Self>,
    ) {
        if self.review_thread_action_thread_id.is_some() {
            self.status = "A review thread action is already running".to_string();
            cx.notify();
            return;
        }

        let Some(pr) = self.selected_pull_request().cloned() else {
            self.review_thread_action_error = Some(ReviewThreadUiError {
                thread_id,
                message: "Select a pull request before updating a thread".to_string(),
            });
            self.status = "Select a pull request before updating a thread".to_string();
            cx.notify();
            return;
        };

        self.review_thread_action_thread_id = Some(thread_id.clone());
        self.review_thread_action_error = None;
        self.status = if resolved {
            format!("Resolving review thread on PR #{}", pr.number)
        } else {
            format!("Reopening review thread on PR #{}", pr.number)
        };
        cx.notify();

        cx.spawn(async move |this, cx| {
            let client = GitHubClient::new(GhCliTransport);
            let result = if resolved {
                client.resolve_review_thread(&thread_id).await
            } else {
                client.unresolve_review_thread(&thread_id).await
            };

            if let Err(error) = this.update(cx, move |view, cx| {
                view.review_thread_action_thread_id = None;

                match result {
                    Ok(()) => {
                        if let Some(thread) = view
                            .review_threads
                            .iter_mut()
                            .find(|thread| thread.id == thread_id)
                        {
                            thread.state = if resolved {
                                ReviewThreadState::Resolved
                            } else {
                                ReviewThreadState::Unresolved
                            };
                        }
                        view.sync_unresolved_thread_count();
                        view.review_thread_action_error = None;
                        view.status = if resolved {
                            format!("Resolved review thread on PR #{}", pr.number)
                        } else {
                            format!("Reopened review thread on PR #{}", pr.number)
                        };
                        view.load_selected_review_data(cx);
                    }
                    Err(error) => {
                        let message = if resolved {
                            format!("Failed to resolve review thread: {error}")
                        } else {
                            format!("Failed to reopen review thread: {error}")
                        };
                        view.review_thread_action_error = Some(ReviewThreadUiError {
                            thread_id,
                            message: message.clone(),
                        });
                        view.status = message;
                    }
                }

                cx.notify();
            }) {
                eprintln!("failed to update review thread action state: {error}");
            }
        })
        .detach();
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

    pub(crate) fn current_pull_request_inbox_key(&self) -> Option<PullRequestInboxCacheKey> {
        self.configured_repo.clone().map(|repository| {
            PullRequestInboxCacheKey::new(repository, self.pull_request_inbox_mode)
        })
    }

    pub(crate) fn cache_current_pull_request_inbox_snapshot(&mut self) {
        let Some(key) = self.current_pull_request_inbox_key() else {
            return;
        };

        if self.is_loading_prs
            || self.is_loading_details
            || self.is_loading_files
            || self.is_loading_checks
            || self.is_loading_workflows
            || self.is_loading_reviews
            || self.load_error.is_some()
        {
            return;
        }

        self.pull_request_inbox_cache
            .insert(key, self.current_pull_request_inbox_snapshot());
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

        if self.is_loading_details
            || self.is_loading_files
            || self.is_loading_checks
            || self.is_loading_workflows
            || self.is_loading_reviews
            || self.details_error.is_some()
            || self.files_error.is_some()
        {
            return;
        }

        if let Some(snapshot) = self.current_pull_request_detail_snapshot() {
            self.pull_request_detail_cache.insert(key, snapshot);
            self.cache_current_pull_request_inbox_snapshot();
        }
    }

    fn current_pull_request_detail_snapshot(&self) -> Option<PullRequestDetailSnapshot> {
        let pull_request = self.selected_pull_request().cloned()?;

        Some(PullRequestDetailSnapshot {
            pull_request,
            files: self.files.clone(),
            diffs: self.diffs.clone(),
            check_runs: self.check_runs.clone(),
            workflow_runs: self.workflow_runs.clone(),
            workflow_jobs: self.workflow_jobs.clone(),
            pull_request_reviews: self.pull_request_reviews.clone(),
            review_threads: self.review_threads.clone(),
            pending_review: self.pending_review.clone(),
            log_chunk: self.log_chunk.clone(),
            current_user_login: self.current_user_login.clone(),
            collapsed_file_tree_folders: self.collapsed_file_tree_folders.clone(),
            reviewed_file_paths: self.reviewed_file_paths.clone(),
            excluded_file_type_filters: self.excluded_file_type_filters.clone(),
            show_files_owned_by_current_user: self.show_files_owned_by_current_user,
            owned_file_paths: self.owned_file_paths.clone(),
            active_file: self.active_file,
            active_hunk: self.active_hunk,
            active_tab: self.active_tab,
        })
    }

    pub(crate) fn restore_selected_pull_request_detail_snapshot(
        &mut self,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some(key) = self.selected_pull_request_detail_key() else {
            return false;
        };
        let Some(snapshot) = self.pull_request_detail_cache.get(&key).cloned() else {
            return false;
        };

        self.pr_detail_tasks.clear();
        self.is_loading_details = false;
        self.is_loading_files = false;
        self.is_loading_checks = false;
        self.is_loading_workflows = false;
        self.is_loading_reviews = false;
        self.is_loading_logs = false;
        self.details_error = None;
        self.files_error = None;
        self.checks_error = None;
        self.workflows_error = None;
        self.reviews_error = None;
        self.logs_error = None;
        self.action_error = None;
        self.pr_action_error = None;
        self.review_comment_error = None;
        self.pending_review_error = None;
        self.clear_review_composer_state();

        if let Some(selected) = self.pull_requests.get_mut(self.selected_pr) {
            *selected = snapshot.pull_request;
        }
        self.files = snapshot.files;
        self.diffs = snapshot.diffs;
        self.check_runs = snapshot.check_runs;
        self.workflow_runs = snapshot.workflow_runs;
        self.workflow_jobs = snapshot.workflow_jobs;
        self.pull_request_reviews = snapshot.pull_request_reviews;
        self.review_threads = snapshot.review_threads;
        self.pending_review = snapshot.pending_review;
        self.log_chunk = snapshot.log_chunk;
        self.current_user_login = snapshot.current_user_login;
        self.collapsed_file_tree_folders = snapshot.collapsed_file_tree_folders;
        self.reviewed_file_paths = snapshot.reviewed_file_paths;
        self.excluded_file_type_filters = snapshot.excluded_file_type_filters;
        self.show_files_owned_by_current_user = snapshot.show_files_owned_by_current_user;
        self.owned_file_paths = snapshot.owned_file_paths;
        self.active_file = snapshot.active_file.min(self.files.len().saturating_sub(1));
        self.active_hunk = snapshot.active_hunk;
        self.active_tab = snapshot.active_tab;

        self.pr_list_scroll
            .scroll_to_item(self.selected_pr, ScrollStrategy::Center);
        self.file_list_scroll.scroll_to_item(0, ScrollStrategy::Top);
        self.diff_list_scroll.scroll_to_item(0, ScrollStrategy::Top);
        self.review_list_scroll
            .scroll_to_item(0, ScrollStrategy::Top);
        self.log_list_scroll.scroll_to_item(0, ScrollStrategy::Top);
        self.status = format!("Showing cached PR #{} details", key.number);
        cx.notify();
        true
    }

    fn current_pull_request_inbox_snapshot(&self) -> PullRequestInboxSnapshot {
        PullRequestInboxSnapshot {
            pull_requests: self.pull_requests.clone(),
            files: self.files.clone(),
            diffs: self.diffs.clone(),
            check_runs: self.check_runs.clone(),
            workflow_runs: self.workflow_runs.clone(),
            workflow_jobs: self.workflow_jobs.clone(),
            pull_request_reviews: self.pull_request_reviews.clone(),
            review_threads: self.review_threads.clone(),
            pending_review: self.pending_review.clone(),
            log_chunk: self.log_chunk.clone(),
            current_user_login: self.current_user_login.clone(),
            collapsed_file_tree_folders: self.collapsed_file_tree_folders.clone(),
            reviewed_file_paths: self.reviewed_file_paths.clone(),
            excluded_file_type_filters: self.excluded_file_type_filters.clone(),
            show_files_owned_by_current_user: self.show_files_owned_by_current_user,
            owned_file_paths: self.owned_file_paths.clone(),
            selected_pr: self.selected_pr,
            active_file: self.active_file,
            active_hunk: self.active_hunk,
            active_tab: self.active_tab,
        }
    }

    pub(crate) fn restore_pull_request_inbox_snapshot(
        &mut self,
        key: PullRequestInboxCacheKey,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some(snapshot) = self.pull_request_inbox_cache.get(&key).cloned() else {
            return false;
        };

        self.configured_repo = Some(key.repository.clone());
        self.pull_request_inbox_mode = key.mode;
        self.pr_list_task = None;
        self.pr_detail_tasks.clear();
        self.is_loading_prs = false;
        self.is_loading_details = false;
        self.is_loading_files = false;
        self.is_loading_checks = false;
        self.is_loading_workflows = false;
        self.is_loading_reviews = false;
        self.is_loading_logs = false;
        self.load_error = None;
        self.details_error = None;
        self.files_error = None;
        self.checks_error = None;
        self.workflows_error = None;
        self.reviews_error = None;
        self.logs_error = None;
        self.action_error = None;
        self.pr_action_error = None;
        self.review_comment_error = None;
        self.pending_review_error = None;
        self.clear_review_composer_state();

        self.pull_requests = snapshot.pull_requests;
        self.files = snapshot.files;
        self.diffs = snapshot.diffs;
        self.check_runs = snapshot.check_runs;
        self.workflow_runs = snapshot.workflow_runs;
        self.workflow_jobs = snapshot.workflow_jobs;
        self.pull_request_reviews = snapshot.pull_request_reviews;
        self.review_threads = snapshot.review_threads;
        self.pending_review = snapshot.pending_review;
        self.log_chunk = snapshot.log_chunk;
        self.current_user_login = snapshot.current_user_login;
        self.collapsed_file_tree_folders = snapshot.collapsed_file_tree_folders;
        self.reviewed_file_paths = snapshot.reviewed_file_paths;
        self.excluded_file_type_filters = snapshot.excluded_file_type_filters;
        self.show_files_owned_by_current_user = snapshot.show_files_owned_by_current_user;
        self.owned_file_paths = snapshot.owned_file_paths;
        self.selected_pr = snapshot
            .selected_pr
            .min(self.pull_requests.len().saturating_sub(1));
        self.active_file = snapshot.active_file.min(self.files.len().saturating_sub(1));
        self.active_hunk = snapshot.active_hunk;
        self.active_tab = snapshot.active_tab;

        self.pr_list_scroll
            .scroll_to_item(self.selected_pr, ScrollStrategy::Center);
        self.file_list_scroll.scroll_to_item(0, ScrollStrategy::Top);
        self.diff_list_scroll.scroll_to_item(0, ScrollStrategy::Top);
        self.review_list_scroll
            .scroll_to_item(0, ScrollStrategy::Top);
        self.log_list_scroll.scroll_to_item(0, ScrollStrategy::Top);
        self.status = format!(
            "Showing cached {} from {}",
            key.mode.status_label(),
            key.repository.full_name()
        );
        cx.notify();
        true
    }

    pub(crate) fn set_repository_local_path(&mut self, repository: RepoId, path: PathBuf) {
        self.repository_local_paths.insert(repository, path);
    }

    pub(crate) fn switcher_repositories(&self) -> Vec<RepoId> {
        let mut repositories = self.repositories.clone();

        if let Some(repository) = self.configured_repo.clone()
            && !repositories.iter().any(|existing| existing == &repository)
        {
            repositories.push(repository);
        }

        for pull_request in &self.pull_requests {
            if !repositories
                .iter()
                .any(|repository| repository == &pull_request.repo)
            {
                repositories.push(pull_request.repo.clone());
            }
        }

        repositories
    }

    pub(crate) fn filtered_switcher_repositories(&self, cx: &App) -> Vec<RepoId> {
        let query = normalized_search_query(&self.repository_search_input.read(cx).value());

        self.switcher_repositories()
            .into_iter()
            .filter(|repository| repository_matches_query(repository, &query))
            .collect()
    }

    pub(crate) fn filtered_switcher_pull_requests(&self, cx: &App) -> Vec<(usize, PullRequest)> {
        let query = normalized_search_query(&self.pull_request_search_input.read(cx).value());

        self.current_repository()
            .map(|repository| {
                self.pull_requests
                    .iter()
                    .enumerate()
                    .filter(|(_, pull_request)| &pull_request.repo == repository)
                    .filter(|(_, pull_request)| pull_request_matches_query(pull_request, &query))
                    .map(|(index, pull_request)| (index, pull_request.clone()))
                    .collect()
            })
            .unwrap_or_default()
    }

    pub(crate) fn reset_repository_switcher_selection(&mut self, cx: &App) {
        let current_repository = self.current_repository().cloned();
        let repositories = self.filtered_switcher_repositories(cx);
        self.repository_switcher_selection = current_repository
            .and_then(|current| {
                repositories
                    .iter()
                    .position(|repository| *repository == current)
            })
            .unwrap_or(0);
    }

    pub(crate) fn reset_pull_request_switcher_selection(&mut self, cx: &App) {
        let pull_requests = self.filtered_switcher_pull_requests(cx);
        self.pull_request_switcher_selection = pull_requests
            .iter()
            .position(|(index, _)| *index == self.selected_pr)
            .unwrap_or(0);
    }

    pub(crate) fn move_repository_switcher_selection(
        &mut self,
        delta: isize,
        cx: &mut Context<Self>,
    ) {
        let len = self.filtered_switcher_repositories(cx).len();
        self.repository_switcher_selection =
            next_switcher_index(self.repository_switcher_selection, len, delta);
        cx.notify();
    }

    pub(crate) fn move_pull_request_switcher_selection(
        &mut self,
        delta: isize,
        cx: &mut Context<Self>,
    ) {
        let len = self.filtered_switcher_pull_requests(cx).len();
        self.pull_request_switcher_selection =
            next_switcher_index(self.pull_request_switcher_selection, len, delta);
        cx.notify();
    }

    pub(crate) fn accept_repository_switcher_selection(&mut self, cx: &mut Context<Self>) {
        let repositories = self.filtered_switcher_repositories(cx);
        let query = self.repository_search_input.read(cx).value();
        let Some(repository) = repository_switcher_accepted_repository(
            &repositories,
            self.repository_switcher_selection,
            &query,
        ) else {
            self.status = if self.is_loading_repositories {
                "Fetching repositories from GitHub...".to_string()
            } else {
                "Type owner/repo to open a repository".to_string()
            };
            cx.notify();
            return;
        };

        self.select_repository_from_switcher(repository, cx);
        self.repository_switcher_open = false;
        cx.notify();
    }

    pub(crate) fn accept_pull_request_switcher_selection(&mut self, cx: &mut Context<Self>) {
        let pull_requests = self.filtered_switcher_pull_requests(cx);
        let Some((index, _)) = pull_requests
            .get(
                self.pull_request_switcher_selection
                    .min(pull_requests.len().saturating_sub(1)),
            )
            .cloned()
        else {
            self.status = "No pull requests match search".to_string();
            cx.notify();
            return;
        };

        self.select_pull_request(index, cx);
        self.pull_request_switcher_open = false;
        cx.notify();
    }

    pub(crate) fn clear_review_composer_state(&mut self) {
        self.review_composer = None;
        self.review_line_selection = None;
        self.review_comment_error = None;
        self.review_thread_reply_thread_id = None;
        self.review_thread_reply_error = None;
    }

    pub(crate) fn apply_loaded_review_data(
        &mut self,
        reviews: Vec<PullRequestReview>,
        review_threads: Vec<ReviewThread>,
        current_user_login: Option<String>,
        pending_review_comment_count: Option<usize>,
    ) -> usize {
        let existing_pending_review = self.pending_review.clone();
        self.current_user_login = current_user_login;
        self.pending_review = pending_review_from_reviews(
            &reviews,
            self.current_user_login.as_deref(),
            existing_pending_review.as_ref(),
            pending_review_comment_count,
        );
        self.pull_request_reviews = reviews;
        self.review_threads = review_threads;

        self.sync_unresolved_thread_count()
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

    fn sync_unresolved_thread_count(&mut self) -> usize {
        let unresolved_count = self
            .review_threads
            .iter()
            .filter(|thread| thread.state == ReviewThreadState::Unresolved)
            .count();

        if let Some(selected) = self.pull_requests.get_mut(self.selected_pr) {
            selected.unresolved_threads = unresolved_count;
        }

        unresolved_count
    }

    pub(crate) fn submit_review_comment(
        &mut self,
        submission: ReviewCommentSubmission,
        cx: &mut Context<Self>,
    ) {
        if self.is_submitting_review_comment {
            self.status = "A review comment is already being submitted".to_string();
            cx.notify();
            return;
        }

        let Some(composer) = self.review_composer.clone() else {
            self.review_comment_error = Some("Select diff lines before commenting".to_string());
            self.status = "Select diff lines before commenting".to_string();
            cx.notify();
            return;
        };
        let Some(pr) = self.selected_pull_request().cloned() else {
            self.review_comment_error = Some("Select a pull request before commenting".to_string());
            self.status = "Select a pull request before commenting".to_string();
            cx.notify();
            return;
        };

        let body = self.review_comment_input.read(cx).value().to_string();
        let body = body.trim().to_string();
        if body.is_empty() {
            self.review_comment_error = Some("Enter a comment before sending".to_string());
            self.status = "Enter a comment before sending".to_string();
            cx.notify();
            return;
        }

        let pending_review_node_id = match submission {
            ReviewCommentSubmission::AddToReview => {
                let Some(pending_review) = self.pending_review.clone() else {
                    self.review_comment_error =
                        Some("Start a review before adding a review comment".to_string());
                    self.status = "Start a review before adding a review comment".to_string();
                    cx.notify();
                    return;
                };
                Some(pending_review.node_id)
            }
            ReviewCommentSubmission::SingleComment | ReviewCommentSubmission::StartReview => None,
        };

        if submission == ReviewCommentSubmission::StartReview && pr.node_id.is_empty() {
            self.review_comment_error =
                Some("GitHub did not return a pull request node id".to_string());
            self.status = "Cannot start review without a pull request node id".to_string();
            cx.notify();
            return;
        }

        self.is_submitting_review_comment = true;
        self.review_comment_error = None;
        self.status = match submission {
            ReviewCommentSubmission::SingleComment => {
                format!("Posting comment on PR #{}", pr.number)
            }
            ReviewCommentSubmission::StartReview => {
                format!("Starting pending review on PR #{}", pr.number)
            }
            ReviewCommentSubmission::AddToReview => {
                format!("Adding comment to pending review on PR #{}", pr.number)
            }
        };
        cx.notify();

        cx.spawn(async move |this, cx| {
            let client = GitHubClient::new(GhCliTransport);
            let result = match submission {
                ReviewCommentSubmission::SingleComment => client
                    .create_pull_request_review_comment(
                        &pr.repo.owner,
                        &pr.repo.name,
                        pr.number,
                        &pr.head_sha,
                        &composer.range,
                        &body,
                    )
                    .await
                    .map(|()| None),
                ReviewCommentSubmission::StartReview => client
                    .start_pull_request_review(&pr.node_id, &pr.head_sha, &composer.range, &body)
                    .await
                    .map(Some),
                ReviewCommentSubmission::AddToReview => {
                    if let Some(pending_review_node_id) = pending_review_node_id {
                        client
                            .add_pending_review_thread(
                                &pending_review_node_id,
                                &composer.range,
                                &body,
                            )
                            .await
                            .map(|()| None)
                    } else {
                        Err(harbor_github::GitHubError::Transport(
                            "missing pending review id".to_string(),
                        ))
                    }
                }
            };

            if let Err(error) = this.update(cx, move |view, cx| {
                view.is_submitting_review_comment = false;

                match result {
                    Ok(new_pending_review_node_id) => {
                        match submission {
                            ReviewCommentSubmission::SingleComment => {}
                            ReviewCommentSubmission::StartReview => {
                                if let Some(node_id) = new_pending_review_node_id {
                                    view.pending_review = Some(PendingReviewSession {
                                        node_id,
                                        comment_count: 1,
                                    });
                                }
                            }
                            ReviewCommentSubmission::AddToReview => {
                                if let Some(pending_review) = view.pending_review.as_mut() {
                                    pending_review.comment_count += 1;
                                }
                            }
                        }

                        view.review_composer = None;
                        view.review_line_selection = None;
                        view.review_comment_error = None;
                        view.status = match submission {
                            ReviewCommentSubmission::SingleComment => {
                                format!("Posted comment on PR #{}", pr.number)
                            }
                            ReviewCommentSubmission::StartReview => {
                                format!("Started pending review on PR #{}", pr.number)
                            }
                            ReviewCommentSubmission::AddToReview => {
                                format!("Added review comment on PR #{}", pr.number)
                            }
                        };
                        view.load_selected_review_data(cx);
                    }
                    Err(error) => {
                        let message = format!("Failed to submit review comment: {error}");
                        view.review_comment_error = Some(message.clone());
                        view.status = message;
                    }
                }

                cx.notify();
            }) {
                eprintln!("failed to update review comment submission state: {error}");
            }
        })
        .detach();
    }

    pub(crate) fn submit_pending_pull_request_review(
        &mut self,
        event: SubmitPullRequestReviewEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.is_submitting_pending_review || self.is_running_pr_action {
            self.status = "A pull request action is already running".to_string();
            cx.notify();
            return;
        }

        let Some(pending_review) = self.pending_review.clone() else {
            self.pending_review_error = Some("No pending review to submit".to_string());
            self.status = "No pending review to submit".to_string();
            cx.notify();
            return;
        };
        let Some(pr) = self.selected_pull_request().cloned() else {
            self.pending_review_error =
                Some("Select a pull request before submitting a review".to_string());
            self.status = "Select a pull request before submitting a review".to_string();
            cx.notify();
            return;
        };

        let body = self.pending_review_body_input.read(cx).value().to_string();
        if event == SubmitPullRequestReviewEvent::Comment
            && pending_review.comment_count == 0
            && body.trim().is_empty()
        {
            self.pending_review_error =
                Some("Add a review summary or at least one pending comment".to_string());
            self.status = "Add a review summary or at least one pending comment".to_string();
            cx.notify();
            return;
        }

        let body = match event {
            SubmitPullRequestReviewEvent::RequestChanges if body.trim().is_empty() => {
                Some(DEFAULT_REQUEST_CHANGES_BODY.to_string())
            }
            _ => {
                let trimmed = body.trim();
                (!trimmed.is_empty()).then(|| trimmed.to_string())
            }
        };

        self.is_submitting_pending_review = true;
        self.is_running_pr_action = true;
        self.pending_review_error = None;
        self.status = format!("Submitting pending review on PR #{}", pr.number);
        cx.notify();

        cx.spawn_in(window, async move |this, cx| {
            let result = GitHubClient::new(GhCliTransport)
                .submit_pull_request_review(&pending_review.node_id, event, body.as_deref())
                .await;

            if let Err(error) = this.update_in(cx, move |view, window, cx| {
                view.is_submitting_pending_review = false;
                view.is_running_pr_action = false;

                match result {
                    Ok(()) => {
                        view.pending_review = None;
                        view.pending_review_error = None;
                        view.pending_review_body_input.update(cx, |input, cx| {
                            input.set_value("", window, cx);
                        });
                        view.status = format!("Submitted pending review on PR #{}", pr.number);
                        view.reload_pull_request_inbox(cx);
                    }
                    Err(error) => {
                        let message = format!("Failed to submit pending review: {error}");
                        view.pending_review_error = Some(message.clone());
                        view.status = message;
                    }
                }

                cx.notify();
            }) {
                eprintln!("failed to update pending review submission state: {error}");
            }
        })
        .detach();
    }

    fn on_switcher_search_event(
        &mut self,
        input: &Entity<InputState>,
        event: &InputEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let is_repository_input = input.entity_id() == self.repository_search_input.entity_id();
        let is_pull_request_input = input.entity_id() == self.pull_request_search_input.entity_id();

        match event {
            InputEvent::Change => {
                if is_repository_input {
                    self.repository_switcher_selection = 0;
                } else if is_pull_request_input {
                    self.pull_request_switcher_selection = 0;
                }

                cx.notify();
            }
            InputEvent::PressEnter { .. }
                if is_repository_input && self.repository_switcher_open =>
            {
                self.accept_repository_switcher_selection(cx);
            }
            InputEvent::PressEnter { .. }
                if is_pull_request_input && self.pull_request_switcher_open =>
            {
                self.accept_pull_request_switcher_selection(cx);
            }
            _ => {}
        }
    }

    fn on_review_input_event(
        &mut self,
        _: &Entity<InputState>,
        event: &InputEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if matches!(event, InputEvent::Change) {
            cx.notify();
        }
    }
}

pub(crate) fn normalized_search_query(query: &str) -> String {
    query.trim().to_lowercase()
}

pub(crate) fn repository_switcher_accepted_repository(
    repositories: &[RepoId],
    selected_index: usize,
    query: &str,
) -> Option<RepoId> {
    repositories
        .get(selected_index.min(repositories.len().saturating_sub(1)))
        .cloned()
        .or_else(|| parse_repo_id(query))
}

pub(crate) fn changed_file_tree_rows(
    files: &[DiffFile],
    collapsed_folders: &HashSet<String>,
    reviewed_file_paths: &HashSet<String>,
    filters: &ChangedFileFilters,
) -> Vec<ChangedFileTreeRow> {
    let filters = ChangedFileFilters {
        query: normalized_search_query(&filters.query),
        excluded_file_types: filters.excluded_file_types.clone(),
        owned_by_current_user_only: filters.owned_by_current_user_only,
        owned_file_paths: filters.owned_file_paths.clone(),
    };
    let mut root = ChangedFileTreeNode::default();

    for (file_index, file) in files.iter().enumerate() {
        if !changed_file_matches_filters(file, &filters) {
            continue;
        }

        let path_segments = file
            .path
            .split('/')
            .filter(|segment| !segment.is_empty())
            .collect::<Vec<_>>();
        if path_segments.is_empty() {
            continue;
        }

        root.add_file(
            file_index,
            &path_segments,
            reviewed_file_paths.contains(&file.path),
        );
    }

    let mut rows = Vec::with_capacity(root.file_count + root.folders.len());
    push_changed_file_tree_rows(
        &root,
        "",
        0,
        files,
        collapsed_folders,
        filters.has_active_filter(),
        &mut rows,
    );
    rows
}

pub(crate) fn changed_file_matches_filters(file: &DiffFile, filters: &ChangedFileFilters) -> bool {
    if filters
        .excluded_file_types
        .contains(&changed_file_type_key(file))
    {
        return false;
    }

    if filters.owned_by_current_user_only && !filters.owned_file_paths.contains(&file.path) {
        return false;
    }

    changed_file_matches_query(file, &filters.query)
}

pub(crate) fn changed_file_matches_query(file: &DiffFile, query: &str) -> bool {
    let query = normalized_search_query(query);

    if query.is_empty() {
        return true;
    }

    if file.path.to_lowercase().contains(&query) {
        return true;
    }

    if file
        .previous_path
        .as_deref()
        .map(|path| path.to_lowercase().contains(&query))
        .unwrap_or(false)
    {
        return true;
    }

    changed_file_status_label(file.status).contains(&query)
}

pub(crate) fn changed_file_type_filters(
    files: &[DiffFile],
    excluded_file_types: &HashSet<String>,
) -> Vec<ChangedFileTypeFilter> {
    let mut file_counts_by_type = BTreeMap::<String, usize>::new();

    for file in files {
        let file_type = changed_file_type_key(file);
        *file_counts_by_type.entry(file_type).or_default() += 1;
    }

    file_counts_by_type
        .into_iter()
        .map(|(key, file_count)| ChangedFileTypeFilter {
            label: key.clone(),
            included: !excluded_file_types.contains(&key),
            key,
            file_count,
        })
        .collect()
}

pub(crate) fn changed_file_type_key(file: &DiffFile) -> String {
    let name = changed_file_name(&file.path);

    if let Some((stem, extension)) = name.rsplit_once('.')
        && !stem.is_empty()
        && !extension.is_empty()
    {
        return extension.to_lowercase();
    }

    "no extension".to_string()
}

fn codeowners_owned_file_paths(
    repository_path: &Path,
    files: &[DiffFile],
    current_user_login: &str,
) -> Result<HashSet<String>, String> {
    let Some(codeowners_path) = codeowners_path(repository_path) else {
        return Ok(HashSet::new());
    };
    let contents = fs::read_to_string(&codeowners_path)
        .map_err(|error| format!("failed to read {}: {error}", codeowners_path.display()))?;
    let rules = parse_codeowners_rules(&contents, current_user_login);
    if rules.is_empty() {
        return Ok(HashSet::new());
    }

    let mut owned_paths = HashSet::new();
    for file in files {
        let mut owned = false;

        for rule in &rules {
            if codeowners_pattern_matches_path(&rule.pattern, &file.path) {
                owned = rule.owned_by_current_user;
            }
        }

        if owned {
            owned_paths.insert(file.path.clone());
        }
    }

    Ok(owned_paths)
}

fn codeowners_path(repository_path: &Path) -> Option<PathBuf> {
    [
        repository_path.join(".github").join("CODEOWNERS"),
        repository_path.join("CODEOWNERS"),
        repository_path.join("docs").join("CODEOWNERS"),
    ]
    .into_iter()
    .find(|path| path.is_file())
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct CodeownersRule {
    pattern: String,
    owned_by_current_user: bool,
}

fn parse_codeowners_rules(contents: &str, current_user_login: &str) -> Vec<CodeownersRule> {
    contents
        .lines()
        .filter_map(|line| parse_codeowners_rule(line, current_user_login))
        .collect()
}

fn parse_codeowners_rule(line: &str, current_user_login: &str) -> Option<CodeownersRule> {
    let line = line.split('#').next().unwrap_or_default().trim();
    if line.is_empty() {
        return None;
    }

    let mut parts = line.split_whitespace();
    let pattern = parts.next()?.trim();
    let owned_by_current_user =
        parts.any(|owner| codeowner_matches_user(owner, current_user_login));

    Some(CodeownersRule {
        pattern: pattern.to_string(),
        owned_by_current_user,
    })
}

fn codeowner_matches_user(owner: &str, current_user_login: &str) -> bool {
    let owner = owner.trim().trim_start_matches('@');
    owner == current_user_login
        || owner
            .rsplit('/')
            .next()
            .map(|segment| segment == current_user_login)
            .unwrap_or(false)
}

fn codeowners_pattern_matches_path(pattern: &str, path: &str) -> bool {
    let normalized_pattern = pattern.trim().trim_start_matches('/');
    if normalized_pattern.is_empty() {
        return false;
    }

    if let Some(directory_pattern) = normalized_pattern.strip_suffix('/') {
        return path == directory_pattern || path.starts_with(&format!("{directory_pattern}/"));
    }

    if !normalized_pattern.contains('/') {
        return wildcard_matches(normalized_pattern, changed_file_name(path))
            || path
                .split('/')
                .any(|segment| wildcard_matches(normalized_pattern, segment));
    }

    wildcard_matches(normalized_pattern, path)
        || path == normalized_pattern
        || path.starts_with(&format!("{normalized_pattern}/"))
}

fn wildcard_matches(pattern: &str, value: &str) -> bool {
    wildcard_matches_bytes(pattern.as_bytes(), value.as_bytes())
}

fn wildcard_matches_bytes(pattern: &[u8], value: &[u8]) -> bool {
    match pattern.split_first() {
        None => value.is_empty(),
        Some((b'*', remaining_pattern)) => {
            wildcard_matches_bytes(remaining_pattern, value)
                || value
                    .split_first()
                    .map(|(_, remaining_value)| wildcard_matches_bytes(pattern, remaining_value))
                    .unwrap_or(false)
        }
        Some((b'?', remaining_pattern)) => value
            .split_first()
            .map(|(_, remaining_value)| wildcard_matches_bytes(remaining_pattern, remaining_value))
            .unwrap_or(false),
        Some((expected, remaining_pattern)) => value
            .split_first()
            .map(|(actual, remaining_value)| {
                expected == actual && wildcard_matches_bytes(remaining_pattern, remaining_value)
            })
            .unwrap_or(false),
    }
}

pub(crate) fn changed_file_status_label(status: FileStatus) -> &'static str {
    match status {
        FileStatus::Added => "added",
        FileStatus::Modified => "modified",
        FileStatus::Removed => "removed",
        FileStatus::Renamed => "renamed",
        FileStatus::Copied => "copied",
        FileStatus::Changed => "changed",
        FileStatus::Unchanged => "unchanged",
    }
}

fn push_changed_file_tree_rows(
    node: &ChangedFileTreeNode,
    parent_path: &str,
    depth: usize,
    files: &[DiffFile],
    collapsed_folders: &HashSet<String>,
    force_expanded: bool,
    rows: &mut Vec<ChangedFileTreeRow>,
) {
    for (folder_name, child_node) in &node.folders {
        let folder_path = if parent_path.is_empty() {
            folder_name.clone()
        } else {
            format!("{parent_path}/{folder_name}")
        };
        let expanded = force_expanded || !collapsed_folders.contains(&folder_path);

        rows.push(ChangedFileTreeRow::Folder(ChangedFileFolderRow {
            path: folder_path.clone(),
            name: folder_name.clone(),
            depth,
            file_count: child_node.file_count,
            reviewed_file_count: child_node.reviewed_file_count,
            expanded,
        }));

        if expanded {
            push_changed_file_tree_rows(
                child_node,
                &folder_path,
                depth + 1,
                files,
                collapsed_folders,
                force_expanded,
                rows,
            );
        }
    }

    let mut file_indices = node.files.clone();
    file_indices.sort_by(|left, right| {
        let left_name = files
            .get(*left)
            .map(|file| changed_file_name(&file.path))
            .unwrap_or_default();
        let right_name = files
            .get(*right)
            .map(|file| changed_file_name(&file.path))
            .unwrap_or_default();

        left_name.cmp(right_name)
    });

    for file_index in file_indices {
        let Some(file) = files.get(file_index) else {
            continue;
        };

        rows.push(ChangedFileTreeRow::File(ChangedFileRow {
            file_index,
            name: changed_file_name(&file.path).to_string(),
            depth,
        }));
    }
}

fn changed_file_name(path: &str) -> &str {
    path.rsplit('/')
        .next()
        .filter(|segment| !segment.is_empty())
        .unwrap_or(path)
}

pub(crate) fn repository_matches_query(repository: &RepoId, query: &str) -> bool {
    if query.is_empty() {
        return true;
    }

    repository.full_name().to_lowercase().contains(query)
}

pub(crate) fn pull_request_matches_query(pull_request: &PullRequest, query: &str) -> bool {
    if query.is_empty() {
        return true;
    }

    pull_request.title.to_lowercase().contains(query)
        || pull_request.number.to_string().contains(query)
        || pull_request.author.to_lowercase().contains(query)
}

pub(crate) fn next_switcher_index(current: usize, len: usize, delta: isize) -> usize {
    if len == 0 {
        return 0;
    }

    let current = current.min(len - 1) as isize;
    (current + delta).rem_euclid(len as isize) as usize
}

fn review_composer_from_selection(
    anchor: &ReviewLineTarget,
    current: &ReviewLineTarget,
) -> std::result::Result<ReviewComposer, String> {
    let range = review_range_from_targets(anchor, current)?;
    let anchor = if anchor.line_index >= current.line_index {
        anchor.clone()
    } else {
        current.clone()
    };

    Ok(ReviewComposer { anchor, range })
}

pub(crate) fn review_range_from_targets(
    anchor: &ReviewLineTarget,
    current: &ReviewLineTarget,
) -> std::result::Result<ReviewCommentRange, String> {
    if anchor.hunk_index != current.hunk_index {
        return Err("Review comments can only span lines in one diff hunk".to_string());
    }

    if anchor.range.path != current.range.path {
        return Err("Review comments can only span one file".to_string());
    }

    if anchor.range.side != current.range.side {
        return Err("Review comments can only span one diff side".to_string());
    }

    let (start, end) = if anchor.line_index <= current.line_index {
        (anchor, current)
    } else {
        (current, anchor)
    };
    let mut range = end.range.clone();

    if start.line_index != end.line_index {
        range.start_line = Some(start.range.line);
        range.start_side = Some(start.range.side);
    } else {
        range.start_line = None;
        range.start_side = None;
    }

    Ok(range)
}

fn pending_review_from_reviews(
    reviews: &[PullRequestReview],
    current_user_login: Option<&str>,
    existing_pending_review: Option<&PendingReviewSession>,
    pending_review_comment_count: Option<usize>,
) -> Option<PendingReviewSession> {
    reviews
        .iter()
        .find(|review| {
            review.state == PullRequestReviewState::Pending
                && current_user_login.is_none_or(|login| review.author == login)
                && review
                    .node_id
                    .as_ref()
                    .is_some_and(|node_id| !node_id.is_empty())
        })
        .and_then(|review| {
            let node_id = review.node_id.clone()?;
            let comment_count = pending_review_comment_count.unwrap_or_else(|| {
                existing_pending_review
                    .filter(|pending_review| pending_review.node_id == node_id)
                    .map_or(0, |pending_review| pending_review.comment_count)
            });

            Some(PendingReviewSession {
                node_id,
                comment_count,
            })
        })
        .or_else(|| existing_pending_review.cloned())
}

fn review_comment_range_label(range: &ReviewCommentRange) -> String {
    let side = match range.side {
        ReviewSide::Left => "left",
        ReviewSide::Right => "right",
    };

    if let Some(start_line) = range.start_line {
        format!("{side} lines {start_line}-{}", range.line)
    } else {
        format!("{side} line {}", range.line)
    }
}

fn initial_repositories(
    configured_repo: Option<&RepoId>,
    pull_requests: &[PullRequest],
) -> Vec<RepoId> {
    let mut repositories = Vec::new();

    if let Some(repository) = configured_repo {
        repositories.push(repository.clone());
    }

    for pull_request in pull_requests {
        if !repositories
            .iter()
            .any(|repository| repository == &pull_request.repo)
        {
            repositories.push(pull_request.repo.clone());
        }
    }

    repositories
}

pub(crate) fn configured_repo_from_env() -> Option<RepoId> {
    std::env::var("HARBOR_REPO")
        .ok()
        .or_else(|| std::env::var("GH_REPO").ok())
        .and_then(|value| parse_repo_id(&value))
}

pub(crate) fn parse_repo_id(value: &str) -> Option<RepoId> {
    let value = value.trim();
    let (owner, name) = value.split_once('/')?;

    if owner.is_empty()
        || name.is_empty()
        || name.contains('/')
        || owner.chars().any(char::is_whitespace)
        || name.chars().any(char::is_whitespace)
    {
        None
    } else {
        Some(RepoId::new(owner, name))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pending_review_uses_loaded_comment_count() {
        let reviews = vec![pending_review("review-rest", "review-node", "alex")];

        let pending_review = pending_review_from_reviews(&reviews, Some("alex"), None, Some(3))
            .expect("pending review should be detected");

        assert_eq!(pending_review.node_id, "review-node");
        assert_eq!(pending_review.comment_count, 3);
    }

    #[test]
    fn pending_review_keeps_existing_count_without_loaded_count() {
        let reviews = vec![pending_review("review-rest", "review-node", "alex")];
        let existing = PendingReviewSession {
            node_id: "review-node".to_string(),
            comment_count: 2,
        };

        let pending_review =
            pending_review_from_reviews(&reviews, Some("alex"), Some(&existing), None)
                .expect("pending review should be detected");

        assert_eq!(pending_review.comment_count, 2);
    }

    fn pending_review(id: &str, node_id: &str, author: &str) -> PullRequestReview {
        PullRequestReview {
            id: id.to_string(),
            node_id: Some(node_id.to_string()),
            author: author.to_string(),
            state: PullRequestReviewState::Pending,
            body: None,
            submitted_at: None,
        }
    }
}
