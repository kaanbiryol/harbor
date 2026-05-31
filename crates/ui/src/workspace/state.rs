use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
    sync::Arc,
};

use chrono::Utc;
use gpui::{Entity, Task, UniformListScrollHandle};
use gpui_component::input::InputState;
use harbor_domain::{
    CheckRun, DiffFile, PullRequestReview, ReactionContent, RepoId, ReviewComment,
    ReviewCommentPosition, ReviewCommentRange, ReviewThread, ReviewThreadState, WorkflowJob,
    WorkflowRun,
};
use harbor_logs::LogChunk;
use harbor_storage::SqliteStore;
use harbor_sync::{
    ActivityState, PullRequestInboxPageInfo, SyncDecision, SyncPolicy, SyncReason, SyncSignals,
    SyncState, SyncTarget,
};

use super::status::LoadStatus;
use super::{
    PendingReviewSession, PullRequestDetailCacheKey, PullRequestDetailSnapshot,
    PullRequestInboxCacheKey, PullRequestInboxMode, PullRequestInboxSnapshot, ReviewCommentUiError,
    ReviewComposer, ReviewLineSelection, ReviewLineTarget, ReviewReactionAction, ReviewReactionKey,
    ReviewThreadUiError,
    notifications::NotificationSink,
    reviews::{
        LOCAL_REVIEW_COMMENT_ID_PREFIX, LOCAL_REVIEW_THREAD_ID_PREFIX,
        OptimisticReviewCommentHandle, apply_review_reaction_overrides,
        apply_review_thread_state_overrides, increment_pending_review_comment_count,
        merge_optimistic_review_threads, pending_review_from_reviews,
        remove_review_comment_from_threads, review_position_from_range,
        rollback_pending_review_comment_count, set_review_comment_reaction_state,
        unresolved_review_thread_count,
    },
};
use crate::diff::ParsedDiff;

#[derive(Default)]
pub(crate) struct WorkspaceTasks {
    pr_list_task: Option<Task<()>>,
    pr_detail_tasks: Vec<Task<()>>,
    repository_task: Option<Task<()>>,
    local_task: Option<Task<()>>,
    external_app_availability_task: Option<Task<()>>,
    sync_task: Option<Task<()>>,
    auth_task: Option<Task<()>>,
}

impl WorkspaceTasks {
    pub(crate) fn clear_pull_request_detail_tasks(&mut self) {
        self.pr_detail_tasks.clear();
    }

    pub(crate) fn set_pull_request_list_task(&mut self, task: Task<()>) {
        self.pr_list_task = Some(task);
    }

    pub(crate) fn push_pull_request_detail_task(&mut self, task: Task<()>) {
        self.pr_detail_tasks.push(task);
    }

    pub(crate) fn clear_pull_request_list_task(&mut self) {
        self.pr_list_task = None;
    }

    pub(crate) fn set_repository_task(&mut self, task: Task<()>) {
        self.repository_task = Some(task);
    }

    pub(crate) fn set_local_task(&mut self, task: Task<()>) {
        self.local_task = Some(task);
    }

    pub(crate) fn clear_local_task(&mut self) {
        self.local_task = None;
    }

    pub(crate) fn set_external_app_availability_task(&mut self, task: Task<()>) {
        self.external_app_availability_task = Some(task);
    }

    pub(crate) fn clear_external_app_availability_task(&mut self) {
        self.external_app_availability_task = None;
    }

    pub(crate) fn set_sync_task(&mut self, task: Task<()>) {
        self.sync_task = Some(task);
    }

    pub(crate) fn set_auth_task(&mut self, task: Task<()>) {
        self.auth_task = Some(task);
    }

    #[cfg(test)]
    pub(crate) fn has_auth_task(&self) -> bool {
        self.auth_task.is_some()
    }

    pub(crate) fn sync_task_running(&self) -> bool {
        self.sync_task.is_some()
    }

    pub(crate) fn local_task_running(&self) -> bool {
        self.local_task.is_some()
    }
}

pub(crate) struct RepositoryUiState {
    repositories: Vec<RepoId>,
    pub(crate) repository_switcher_open: bool,
    pub(crate) repository_switcher_selection: usize,
    pub(crate) repository_search_input: Entity<InputState>,
    configured_repo: Option<RepoId>,
    repository_store: Option<SqliteStore>,
    repository_local_paths: HashMap<RepoId, PathBuf>,
    is_loading_repositories: bool,
    repository_error: Option<String>,
    repository_notice: Option<String>,
}

impl RepositoryUiState {
    pub(crate) fn new(repository_search_input: Entity<InputState>, is_loading: bool) -> Self {
        Self {
            repositories: Vec::new(),
            repository_switcher_open: false,
            repository_switcher_selection: 0,
            repository_search_input,
            configured_repo: None,
            repository_store: None,
            repository_local_paths: HashMap::new(),
            is_loading_repositories: is_loading,
            repository_error: None,
            repository_notice: None,
        }
    }

    pub(crate) fn repositories(&self) -> &[RepoId] {
        &self.repositories
    }

    pub(crate) fn configured_repo(&self) -> Option<&RepoId> {
        self.configured_repo.as_ref()
    }

    pub(crate) fn configured_repo_cloned(&self) -> Option<RepoId> {
        self.configured_repo.clone()
    }

    pub(crate) fn has_configured_repo(&self) -> bool {
        self.configured_repo.is_some()
    }

    pub(crate) fn store(&self) -> Option<SqliteStore> {
        self.repository_store.clone()
    }

    pub(crate) fn local_path(&self, repository: &RepoId) -> Option<&PathBuf> {
        self.repository_local_paths.get(repository)
    }

    pub(crate) fn is_loading(&self) -> bool {
        self.is_loading_repositories
    }

    pub(crate) fn error(&self) -> Option<&str> {
        self.repository_error.as_deref()
    }

    pub(crate) fn notice(&self) -> Option<&str> {
        self.repository_notice.as_deref()
    }

    pub(crate) fn start_loading(&mut self) {
        self.is_loading_repositories = true;
    }

    pub(crate) fn finish_loading(&mut self) {
        self.is_loading_repositories = false;
    }

    pub(crate) fn set_store(&mut self, store: SqliteStore) {
        self.repository_store = Some(store);
        self.repository_error = None;
        self.repository_notice = None;
    }

    pub(crate) fn clear_store_with_error(&mut self, error: impl Into<String>) {
        self.repository_store = None;
        self.is_loading_repositories = false;
        self.repository_error = Some(error.into());
        self.repository_notice = None;
    }

    pub(crate) fn set_error(&mut self, error: impl Into<String>) {
        self.repository_error = Some(error.into());
        self.repository_notice = None;
    }

    pub(crate) fn clear_error(&mut self) {
        self.repository_error = None;
    }

    pub(crate) fn set_notice(&mut self, notice: impl Into<String>) {
        self.repository_error = None;
        self.repository_notice = Some(notice.into());
    }

    pub(crate) fn clear_notice(&mut self) {
        self.repository_notice = None;
    }

    pub(crate) fn clear_visible_repositories(&mut self) {
        self.repositories.clear();
        self.configured_repo = None;
        self.repository_local_paths.clear();
        self.repository_switcher_selection = 0;
        self.repository_error = None;
        self.repository_notice = None;
    }

    pub(crate) fn select_repository(&mut self, repository: RepoId) {
        self.configured_repo = Some(repository);
    }

    pub(crate) fn remember_repository(&mut self, repository: RepoId) {
        self.repositories.retain(|existing| existing != &repository);
        self.repositories.insert(0, repository);
    }

    pub(crate) fn set_local_path(&mut self, repository: RepoId, path: PathBuf) {
        self.repository_local_paths.insert(repository, path);
    }
}

pub(crate) struct PullRequestDetailUiState {
    pub(crate) files: Vec<DiffFile>,
    pub(crate) diffs: Vec<Option<ParsedDiff>>,
    pub(crate) check_runs: Vec<CheckRun>,
    pub(crate) workflow_runs: Vec<WorkflowRun>,
    pub(crate) workflow_jobs: Vec<WorkflowJob>,
    pull_request_detail_cache: HashMap<PullRequestDetailCacheKey, PullRequestDetailSnapshot>,
    details_load: LoadStatus,
    files_load: LoadStatus,
    checks_load: LoadStatus,
    workflows_load: LoadStatus,
    pub(crate) log_state: WorkflowLogState,
}

pub(crate) struct ReviewRuntimeState {
    pub(crate) pull_request_reviews: Vec<PullRequestReview>,
    pub(crate) review_threads: Vec<ReviewThread>,
    pub(crate) review_composer_state: ReviewComposerState,
    pending_review: Option<PendingReviewSession>,
    is_submitting_review_comment: bool,
    is_submitting_review_thread_reply: bool,
    is_submitting_review_comment_edit: bool,
    is_submitting_pending_review: bool,
    review_thread_action_thread_id: Option<String>,
    review_comment_action_comment_id: Option<String>,
    review_reaction_action: Option<ReviewReactionAction>,
    review_thread_state_overrides: HashMap<String, ReviewThreadState>,
    review_reaction_overrides: HashMap<ReviewReactionKey, bool>,
    reviews_load: LoadStatus,
    review_comment_error: Option<String>,
    review_thread_reply_error: Option<ReviewThreadUiError>,
    review_thread_action_error: Option<ReviewThreadUiError>,
    review_comment_edit_error: Option<ReviewCommentUiError>,
    review_comment_action_error: Option<ReviewCommentUiError>,
    review_reaction_error: Option<ReviewCommentUiError>,
    pending_review_error: Option<String>,
    pub(crate) current_user_login: Option<String>,
    local_review_comment_sequence: u64,
    review_data_generation: u64,
}

pub(crate) struct NotificationState {
    pub(crate) notification_sink: Arc<dyn NotificationSink>,
    pub(crate) notification_dedupe: HashSet<String>,
    pub(crate) notifications_enabled: bool,
}

pub(crate) struct SyncRuntimeState {
    activity_state: ActivityState,
    sync_policy: SyncPolicy,
    sync_states: HashMap<SyncTarget, SyncState>,
    did_focus: bool,
}

impl SyncRuntimeState {
    pub(crate) fn new(activity_state: ActivityState, sync_policy: SyncPolicy) -> Self {
        Self {
            activity_state,
            sync_policy,
            sync_states: HashMap::new(),
            did_focus: false,
        }
    }

    pub(crate) fn set_activity(&mut self, activity_state: ActivityState) {
        self.activity_state = activity_state;
    }

    pub(crate) fn activity_state(&self) -> ActivityState {
        self.activity_state
    }

    pub(crate) fn is_background(&self) -> bool {
        self.activity_state == ActivityState::Background
    }

    pub(crate) fn did_focus(&self) -> bool {
        self.did_focus
    }

    pub(crate) fn mark_focused_once(&mut self) {
        self.did_focus = true;
    }

    pub(crate) fn mark_attempt(&mut self, target: SyncTarget) {
        self.sync_states
            .entry(target)
            .or_default()
            .mark_attempt(Utc::now());
    }

    pub(crate) fn mark_success(&mut self, target: SyncTarget) {
        self.sync_states
            .entry(target)
            .or_default()
            .mark_success(Utc::now());
    }

    pub(crate) fn mark_failure(&mut self, target: SyncTarget) {
        self.sync_states.entry(target).or_default().mark_failure();
    }

    pub(crate) fn mark_stale(&mut self, target: SyncTarget) {
        self.sync_states.entry(target).or_default().mark_stale();
    }

    pub(crate) fn decision(
        &self,
        target: SyncTarget,
        reason: SyncReason,
        signals: SyncSignals,
    ) -> SyncDecision {
        let empty_state = SyncState::default();
        let state = self.sync_states.get(&target).unwrap_or(&empty_state);

        self.sync_policy.decision(
            target,
            reason,
            self.activity_state,
            state,
            signals,
            Utc::now(),
        )
    }

    #[cfg(test)]
    pub(crate) fn sync_state(&self, target: SyncTarget) -> Option<&SyncState> {
        self.sync_states.get(&target)
    }

    #[cfg(test)]
    pub(crate) fn set_sync_state(&mut self, target: SyncTarget, state: SyncState) {
        self.sync_states.insert(target, state);
    }
}

#[derive(Default)]
pub(crate) struct PullRequestInboxState {
    visible: bool,
    mode: PullRequestInboxMode,
    cache: HashMap<PullRequestInboxCacheKey, PullRequestInboxSnapshot>,
    counts: HashMap<PullRequestInboxCacheKey, usize>,
    page_info: PullRequestInboxPageInfo,
    load: LoadStatus,
    more_load: LoadStatus,
}

impl PullRequestInboxState {
    pub(crate) fn visible_by_default() -> Self {
        Self {
            visible: true,
            ..Self::default()
        }
    }

    pub(crate) fn is_visible(&self) -> bool {
        self.visible
    }

    pub(crate) fn set_visible(&mut self, visible: bool) {
        self.visible = visible;
    }

    pub(crate) fn toggle_visible(&mut self) {
        self.visible = !self.visible;
    }

    pub(crate) fn mode(&self) -> PullRequestInboxMode {
        self.mode
    }

    pub(crate) fn set_mode(&mut self, mode: PullRequestInboxMode) {
        self.mode = mode;
    }

    pub(crate) fn start_loading(&mut self) {
        self.load.start();
        self.more_load.reset();
    }

    pub(crate) fn apply_success(&mut self) {
        self.load.succeed();
    }

    pub(crate) fn apply_failure(&mut self, error: impl Into<String>) {
        self.load.fail(error);
    }

    pub(crate) fn reset_load(&mut self) {
        self.load.reset();
        self.more_load.reset();
    }

    pub(crate) fn is_loading(&self) -> bool {
        self.load.is_loading()
    }

    pub(crate) fn load_error(&self) -> Option<&str> {
        self.load.error()
    }

    pub(crate) fn can_cache_snapshot(&self) -> bool {
        !self.is_loading()
            && self.load_error().is_none()
            && !self.is_loading_more()
            && self.load_more_error().is_none()
    }

    pub(crate) fn page_info(&self) -> &PullRequestInboxPageInfo {
        &self.page_info
    }

    pub(crate) fn set_page_info(&mut self, page_info: PullRequestInboxPageInfo) {
        self.page_info = page_info;
    }

    pub(crate) fn clear_page_info(&mut self) {
        self.page_info = PullRequestInboxPageInfo::default();
    }

    pub(crate) fn total_count(&self) -> Option<usize> {
        self.page_info.total_count
    }

    pub(crate) fn has_next_page(&self) -> bool {
        self.page_info.has_next_page()
    }

    pub(crate) fn next_page_cursor(&self) -> Option<harbor_github::PullRequestPageCursor> {
        self.page_info.next_cursor.clone()
    }

    pub(crate) fn start_loading_more(&mut self) {
        self.more_load.start();
    }

    pub(crate) fn apply_load_more_success(&mut self) {
        self.more_load.succeed();
    }

    pub(crate) fn apply_load_more_failure(&mut self, error: impl Into<String>) {
        self.more_load.fail(error);
    }

    pub(crate) fn is_loading_more(&self) -> bool {
        self.more_load.is_loading()
    }

    pub(crate) fn load_more_error(&self) -> Option<&str> {
        self.more_load.error()
    }

    pub(crate) fn insert_snapshot(
        &mut self,
        key: PullRequestInboxCacheKey,
        snapshot: PullRequestInboxSnapshot,
    ) {
        if let Some(count) = snapshot.count() {
            self.counts.insert(key.clone(), count);
        }
        self.cache.insert(key, snapshot);
    }

    pub(crate) fn insert_count(&mut self, key: PullRequestInboxCacheKey, count: usize) {
        self.counts.insert(key, count);
    }

    pub(crate) fn stored_count(&self, key: &PullRequestInboxCacheKey) -> Option<usize> {
        self.counts.get(key).copied()
    }

    pub(crate) fn snapshot(
        &self,
        key: &PullRequestInboxCacheKey,
    ) -> Option<&PullRequestInboxSnapshot> {
        self.cache.get(key)
    }

    pub(crate) fn snapshot_count(&self, key: &PullRequestInboxCacheKey) -> Option<usize> {
        self.counts.get(key).copied().or_else(|| {
            self.cache
                .get(key)
                .and_then(PullRequestInboxSnapshot::count)
        })
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct PullRequestSelectionState {
    pull_request_index: usize,
    file_index: usize,
    hunk_index: usize,
}

impl PullRequestSelectionState {
    pub(crate) fn pull_request_index(&self) -> usize {
        self.pull_request_index
    }

    pub(crate) fn file_index(&self) -> usize {
        self.file_index
    }

    pub(crate) fn hunk_index(&self) -> usize {
        self.hunk_index
    }

    pub(crate) fn set_pull_request_index(&mut self, index: usize) {
        self.pull_request_index = index;
    }

    pub(crate) fn restore_pull_request_index(&mut self, index: usize, pull_request_count: usize) {
        self.pull_request_index = index.min(pull_request_count.saturating_sub(1));
    }

    pub(crate) fn reset_pull_request_index(&mut self) {
        self.pull_request_index = 0;
    }

    pub(crate) fn reset_diff_selection(&mut self) {
        self.file_index = 0;
        self.hunk_index = 0;
    }

    pub(crate) fn select_file_index(&mut self, file_index: usize) {
        self.file_index = file_index;
        self.hunk_index = 0;
    }

    pub(crate) fn set_diff_position(&mut self, file_index: usize, hunk_index: usize) {
        self.file_index = file_index;
        self.hunk_index = hunk_index;
    }

    pub(crate) fn restore_diff_position(
        &mut self,
        file_index: usize,
        hunk_index: usize,
        file_count: usize,
    ) {
        self.file_index = file_index.min(file_count.saturating_sub(1));
        self.hunk_index = hunk_index;
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct PullRequestDetailLoadedState {
    pub(crate) details: bool,
    pub(crate) files: bool,
    pub(crate) checks: bool,
    pub(crate) workflows: bool,
    pub(crate) reviews: bool,
}

pub(crate) struct ReviewComposerState {
    mode: ReviewComposerMode,
    pub(crate) comment_input: Entity<InputState>,
    pub(crate) thread_reply_input: Entity<InputState>,
    pub(crate) comment_edit_input: Entity<InputState>,
    pub(crate) pending_review_body_input: Entity<InputState>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum ReviewComposerMode {
    Idle,
    Selecting {
        line_selection: ReviewLineSelection,
    },
    Inline {
        composer: ReviewComposer,
        line_selection: ReviewLineSelection,
    },
    ThreadReply {
        thread_id: String,
    },
    CommentEdit {
        comment_id: String,
    },
}

pub(crate) struct WorkflowLogState {
    chunk: Option<LogChunk>,
    task: Option<Task<()>>,
    pub(crate) list_scroll: UniformListScrollHandle,
    is_loading: bool,
    error: Option<String>,
}

impl WorkflowLogState {
    pub(crate) fn new() -> Self {
        Self {
            chunk: None,
            task: None,
            list_scroll: UniformListScrollHandle::new(),
            is_loading: false,
            error: None,
        }
    }

    pub(crate) fn chunk(&self) -> Option<&LogChunk> {
        self.chunk.as_ref()
    }

    pub(crate) fn set_chunk(&mut self, chunk: Option<LogChunk>) {
        self.chunk = chunk;
    }

    pub(crate) fn set_task(&mut self, task: Task<()>) {
        self.task = Some(task);
    }

    pub(crate) fn is_loading(&self) -> bool {
        self.is_loading
    }

    pub(crate) fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }

    pub(crate) fn has_error(&self) -> bool {
        self.error.is_some()
    }

    pub(crate) fn start_loading(&mut self) {
        self.is_loading = true;
        self.error = None;
        self.chunk = None;
    }

    pub(crate) fn set_loading(&mut self, loading: bool) {
        self.is_loading = loading;
    }

    pub(crate) fn clear_content(&mut self) {
        self.chunk = None;
    }

    pub(crate) fn clear_error(&mut self) {
        self.error = None;
    }

    pub(crate) fn apply_jobs_failure(&mut self, error: impl Into<String>) {
        self.error = Some(error.into());
    }

    pub(crate) fn apply_log_success(&mut self, chunk: LogChunk) {
        self.chunk = Some(chunk);
        self.is_loading = false;
    }

    pub(crate) fn apply_log_failure(&mut self, error: impl Into<String>) {
        self.error = Some(error.into());
        self.is_loading = false;
    }
}

impl PullRequestDetailUiState {
    pub(crate) fn new(
        files: Vec<DiffFile>,
        diffs: Vec<Option<ParsedDiff>>,
        log_state: WorkflowLogState,
    ) -> Self {
        Self {
            files,
            diffs,
            check_runs: Vec::new(),
            workflow_runs: Vec::new(),
            workflow_jobs: Vec::new(),
            pull_request_detail_cache: HashMap::new(),
            details_load: LoadStatus::Idle,
            files_load: LoadStatus::Idle,
            checks_load: LoadStatus::Idle,
            workflows_load: LoadStatus::Idle,
            log_state,
        }
    }

    pub(crate) fn reset_for_selection(&mut self) {
        self.details_load.reset();
        self.files_load.reset();
        self.checks_load.reset();
        self.workflows_load.reset();
    }

    pub(crate) fn cache_snapshot(
        &mut self,
        key: PullRequestDetailCacheKey,
        snapshot: PullRequestDetailSnapshot,
    ) {
        self.pull_request_detail_cache.insert(key, snapshot);
    }

    pub(crate) fn snapshot(
        &self,
        key: &PullRequestDetailCacheKey,
    ) -> Option<&PullRequestDetailSnapshot> {
        self.pull_request_detail_cache.get(key)
    }

    pub(crate) fn remove_optimistic_comment_from_snapshot(
        &mut self,
        key: &PullRequestDetailCacheKey,
        comment_id: &str,
    ) {
        if let Some(snapshot) = self.pull_request_detail_cache.get_mut(key) {
            remove_review_comment_from_threads(&mut snapshot.review_threads, comment_id);
            snapshot.pull_request.unresolved_threads =
                unresolved_review_thread_count(&snapshot.review_threads);
        }
    }

    pub(crate) fn rollback_pending_review_comment_count_in_snapshot(
        &mut self,
        key: &PullRequestDetailCacheKey,
        previous_pending_review: Option<&PendingReviewSession>,
    ) {
        if let Some(snapshot) = self.pull_request_detail_cache.get_mut(key) {
            rollback_pending_review_comment_count(
                &mut snapshot.pending_review,
                previous_pending_review,
            );
        }
    }

    pub(crate) fn set_pending_review_in_snapshot(
        &mut self,
        key: &PullRequestDetailCacheKey,
        pending_review: PendingReviewSession,
    ) {
        if let Some(snapshot) = self.pull_request_detail_cache.get_mut(key) {
            snapshot.pending_review = Some(pending_review);
        }
    }

    pub(crate) fn restore_loaded_sections(&mut self, sections: PullRequestDetailLoadedState) {
        self.details_load = load_status_from_loaded(sections.details);
        self.files_load = load_status_from_loaded(sections.files);
        self.checks_load = load_status_from_loaded(sections.checks);
        self.workflows_load = load_status_from_loaded(sections.workflows);
    }

    pub(crate) fn loaded_sections(&self, reviews_loaded: bool) -> PullRequestDetailLoadedState {
        PullRequestDetailLoadedState {
            details: self.details_load.is_finished(),
            files: self.files_load.is_finished(),
            checks: self.checks_load.is_finished(),
            workflows: self.workflows_load.is_finished(),
            reviews: reviews_loaded,
        }
    }

    pub(crate) fn is_any_loading(&self) -> bool {
        self.details_load.is_loading()
            || self.files_load.is_loading()
            || self.checks_load.is_loading()
            || self.workflows_load.is_loading()
    }

    pub(crate) fn has_cache_blocking_error(&self) -> bool {
        self.details_error().is_some() || self.files_error().is_some()
    }

    pub(crate) fn clear_errors(&mut self) {
        self.details_load.clear_error();
        self.files_load.clear_error();
        self.checks_load.clear_error();
        self.workflows_load.clear_error();
    }

    pub(crate) fn should_load_details(&self) -> bool {
        !self.details_load.is_loading() && !self.details_load.is_finished()
    }

    pub(crate) fn should_load_files(&self) -> bool {
        !self.files_load.is_loading() && !self.files_load.is_finished()
    }

    pub(crate) fn should_load_checks(&self) -> bool {
        !self.checks_load.is_loading() && !self.checks_load.is_finished()
    }

    pub(crate) fn should_load_workflows(&self) -> bool {
        !self.workflows_load.is_loading() && !self.workflows_load.is_finished()
    }

    pub(crate) fn mark_details_stale(&mut self) {
        self.details_load.reset();
    }

    pub(crate) fn mark_checks_stale(&mut self) {
        self.checks_load.reset();
    }

    pub(crate) fn mark_workflows_stale(&mut self) {
        self.workflows_load.reset();
    }

    pub(crate) fn start_details_load(&mut self) {
        self.details_load.start();
    }

    pub(crate) fn start_files_load(&mut self) {
        self.files_load.start();
    }

    pub(crate) fn start_checks_load(&mut self) {
        self.checks_load.start();
    }

    pub(crate) fn start_workflows_load(&mut self) {
        self.workflows_load.start();
    }

    pub(crate) fn apply_details_success(&mut self) {
        self.details_load.succeed();
    }

    pub(crate) fn apply_files_success(&mut self) {
        self.files_load.succeed();
    }

    pub(crate) fn apply_checks_success(&mut self) {
        self.checks_load.succeed();
    }

    pub(crate) fn apply_workflows_success(&mut self) {
        self.workflows_load.succeed();
    }

    pub(crate) fn apply_details_failure(&mut self, error: impl Into<String>) {
        self.details_load.fail(error);
    }

    pub(crate) fn apply_files_failure(&mut self, error: impl Into<String>) {
        self.files_load.fail(error);
    }

    pub(crate) fn apply_checks_failure(&mut self, error: impl Into<String>) {
        self.checks_load.fail(error);
    }

    pub(crate) fn apply_workflows_failure(&mut self, error: impl Into<String>) {
        self.workflows_load.fail(error);
    }

    pub(crate) fn details_loading(&self) -> bool {
        self.details_load.is_loading()
    }

    pub(crate) fn details_loaded(&self) -> bool {
        self.details_load.is_loaded()
    }

    pub(crate) fn files_loading(&self) -> bool {
        self.files_load.is_loading()
    }

    pub(crate) fn checks_loading(&self) -> bool {
        self.checks_load.is_loading()
    }

    pub(crate) fn workflows_loading(&self) -> bool {
        self.workflows_load.is_loading()
    }

    pub(crate) fn details_error(&self) -> Option<&str> {
        self.details_load.error()
    }

    pub(crate) fn files_error(&self) -> Option<&str> {
        self.files_load.error()
    }

    pub(crate) fn checks_error(&self) -> Option<&str> {
        self.checks_load.error()
    }

    pub(crate) fn workflows_error(&self) -> Option<&str> {
        self.workflows_load.error()
    }
}

impl ReviewRuntimeState {
    pub(crate) fn new(
        pull_request_reviews: Vec<PullRequestReview>,
        review_threads: Vec<ReviewThread>,
        review_composer_state: ReviewComposerState,
    ) -> Self {
        Self {
            pull_request_reviews,
            review_threads,
            review_composer_state,
            pending_review: None,
            is_submitting_review_comment: false,
            is_submitting_review_thread_reply: false,
            is_submitting_review_comment_edit: false,
            is_submitting_pending_review: false,
            review_thread_action_thread_id: None,
            review_comment_action_comment_id: None,
            review_reaction_action: None,
            review_thread_state_overrides: HashMap::new(),
            review_reaction_overrides: HashMap::new(),
            reviews_load: LoadStatus::Idle,
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
        }
    }

    pub(crate) fn reset_reviews_load(&mut self) {
        self.reviews_load.reset();
    }

    pub(crate) fn mark_reviews_stale(&mut self) {
        self.reviews_load.reset();
    }

    pub(crate) fn start_reviews_load(&mut self) {
        self.reviews_load.start();
    }

    pub(crate) fn apply_reviews_success(&mut self) {
        self.reviews_load.succeed();
    }

    pub(crate) fn apply_reviews_failure(&mut self, error: impl Into<String>) {
        self.reviews_load.fail(error);
    }

    pub(crate) fn reviews_loading(&self) -> bool {
        self.reviews_load.is_loading()
    }

    pub(crate) fn reviews_finished(&self) -> bool {
        self.reviews_load.is_finished()
    }

    pub(crate) fn should_load_reviews(&self) -> bool {
        !self.reviews_load.is_loading() && !self.reviews_load.is_finished()
    }

    pub(crate) fn reviews_error(&self) -> Option<&str> {
        self.reviews_load.error()
    }

    pub(crate) fn clear_reviews_error(&mut self) {
        self.reviews_load.clear_error();
    }

    pub(crate) fn pending_review(&self) -> Option<&PendingReviewSession> {
        self.pending_review.as_ref()
    }

    pub(crate) fn pending_review_cloned(&self) -> Option<PendingReviewSession> {
        self.pending_review.clone()
    }

    pub(crate) fn has_pending_review(&self) -> bool {
        self.pending_review.is_some()
    }

    pub(crate) fn clear_pending_review(&mut self) {
        self.pending_review = None;
    }

    pub(crate) fn increment_pending_review_comment_count(&mut self) {
        increment_pending_review_comment_count(&mut self.pending_review);
    }

    pub(crate) fn is_submitting_review_comment(&self) -> bool {
        self.is_submitting_review_comment
    }

    pub(crate) fn start_review_comment_submission(&mut self, show_submitting: bool) {
        self.is_submitting_review_comment = show_submitting;
        self.review_composer_state.clear();
        self.review_comment_error = None;
    }

    pub(crate) fn finish_review_comment_submission(&mut self) {
        self.is_submitting_review_comment = false;
    }

    pub(crate) fn review_comment_error(&self) -> Option<&str> {
        self.review_comment_error.as_deref()
    }

    pub(crate) fn set_review_comment_error(&mut self, error: impl Into<String>) {
        self.review_comment_error = Some(error.into());
    }

    pub(crate) fn clear_review_comment_error(&mut self) {
        self.review_comment_error = None;
    }

    pub(crate) fn is_submitting_review_thread_reply(&self) -> bool {
        self.is_submitting_review_thread_reply
    }

    pub(crate) fn finish_review_thread_reply_submission(&mut self) {
        self.is_submitting_review_thread_reply = false;
    }

    pub(crate) fn review_thread_reply_error(&self) -> Option<&ReviewThreadUiError> {
        self.review_thread_reply_error.as_ref()
    }

    pub(crate) fn set_review_thread_reply_error(
        &mut self,
        thread_id: impl Into<String>,
        message: impl Into<String>,
    ) {
        self.review_thread_reply_error = Some(ReviewThreadUiError {
            thread_id: thread_id.into(),
            message: message.into(),
        });
    }

    pub(crate) fn clear_review_thread_reply_error(&mut self) {
        self.review_thread_reply_error = None;
    }

    pub(crate) fn is_submitting_review_comment_edit(&self) -> bool {
        self.is_submitting_review_comment_edit
    }

    pub(crate) fn start_review_comment_edit_submission(&mut self, comment_id: String) {
        self.is_submitting_review_comment_edit = true;
        self.review_composer_state.open_comment_edit(comment_id);
        self.review_comment_edit_error = None;
    }

    pub(crate) fn finish_review_comment_edit_submission(&mut self) {
        self.is_submitting_review_comment_edit = false;
    }

    pub(crate) fn review_comment_edit_error(&self) -> Option<&ReviewCommentUiError> {
        self.review_comment_edit_error.as_ref()
    }

    pub(crate) fn set_review_comment_edit_error(
        &mut self,
        comment_id: impl Into<String>,
        message: impl Into<String>,
    ) {
        self.review_comment_edit_error = Some(ReviewCommentUiError {
            comment_id: comment_id.into(),
            message: message.into(),
        });
    }

    pub(crate) fn clear_review_comment_edit_error(&mut self) {
        self.review_comment_edit_error = None;
    }

    pub(crate) fn comment_action_running(&self) -> bool {
        self.review_comment_action_comment_id.is_some()
    }

    pub(crate) fn review_comment_action_comment_id(&self) -> Option<&str> {
        self.review_comment_action_comment_id.as_deref()
    }

    pub(crate) fn start_review_comment_action(&mut self, comment_id: String) {
        self.review_comment_action_comment_id = Some(comment_id);
        self.review_comment_action_error = None;
    }

    pub(crate) fn finish_review_comment_action(&mut self) {
        self.review_comment_action_comment_id = None;
    }

    pub(crate) fn review_comment_action_error(&self) -> Option<&ReviewCommentUiError> {
        self.review_comment_action_error.as_ref()
    }

    pub(crate) fn set_review_comment_action_error(
        &mut self,
        comment_id: impl Into<String>,
        message: impl Into<String>,
    ) {
        self.review_comment_action_error = Some(ReviewCommentUiError {
            comment_id: comment_id.into(),
            message: message.into(),
        });
    }

    pub(crate) fn clear_review_comment_action_error(&mut self) {
        self.review_comment_action_error = None;
    }

    pub(crate) fn thread_action_running(&self) -> bool {
        self.review_thread_action_thread_id.is_some()
    }

    pub(crate) fn review_thread_action_thread_id(&self) -> Option<&str> {
        self.review_thread_action_thread_id.as_deref()
    }

    pub(crate) fn start_review_thread_action(&mut self, thread_id: String) {
        self.review_thread_action_thread_id = Some(thread_id);
        self.review_thread_action_error = None;
    }

    pub(crate) fn finish_review_thread_action(&mut self) {
        self.review_thread_action_thread_id = None;
    }

    pub(crate) fn review_thread_action_error(&self) -> Option<&ReviewThreadUiError> {
        self.review_thread_action_error.as_ref()
    }

    pub(crate) fn set_review_thread_action_error(
        &mut self,
        thread_id: impl Into<String>,
        message: impl Into<String>,
    ) {
        self.review_thread_action_error = Some(ReviewThreadUiError {
            thread_id: thread_id.into(),
            message: message.into(),
        });
    }

    pub(crate) fn clear_review_thread_action_error(&mut self) {
        self.review_thread_action_error = None;
    }

    pub(crate) fn set_review_thread_state_override(
        &mut self,
        thread_id: String,
        state: ReviewThreadState,
    ) {
        self.review_thread_state_overrides.insert(thread_id, state);
    }

    pub(crate) fn remove_review_thread_state_override(&mut self, thread_id: &str) {
        self.review_thread_state_overrides.remove(thread_id);
    }

    pub(crate) fn reaction_action_running(&self) -> bool {
        self.review_reaction_action.is_some()
    }

    pub(crate) fn review_reaction_action(&self) -> Option<&ReviewReactionAction> {
        self.review_reaction_action.as_ref()
    }

    pub(crate) fn start_review_reaction_action(&mut self, action: ReviewReactionAction) {
        self.review_reaction_action = Some(action);
        self.review_reaction_error = None;
    }

    pub(crate) fn finish_review_reaction_action(&mut self) {
        self.review_reaction_action = None;
    }

    pub(crate) fn review_reaction_error(&self) -> Option<&ReviewCommentUiError> {
        self.review_reaction_error.as_ref()
    }

    pub(crate) fn set_review_reaction_error(
        &mut self,
        comment_id: impl Into<String>,
        message: impl Into<String>,
    ) {
        self.review_reaction_error = Some(ReviewCommentUiError {
            comment_id: comment_id.into(),
            message: message.into(),
        });
    }

    pub(crate) fn clear_review_reaction_error(&mut self) {
        self.review_reaction_error = None;
    }

    pub(crate) fn set_review_reaction_override(
        &mut self,
        key: ReviewReactionKey,
        viewer_has_reacted: bool,
    ) {
        self.review_reaction_overrides
            .insert(key, viewer_has_reacted);
    }

    pub(crate) fn remove_review_reaction_override(&mut self, key: &ReviewReactionKey) {
        self.review_reaction_overrides.remove(key);
    }

    pub(crate) fn is_submitting_pending_review(&self) -> bool {
        self.is_submitting_pending_review
    }

    pub(crate) fn start_pending_review_submission(&mut self) {
        self.is_submitting_pending_review = true;
        self.pending_review_error = None;
    }

    pub(crate) fn finish_pending_review_submission(&mut self) {
        self.is_submitting_pending_review = false;
    }

    pub(crate) fn pending_review_error(&self) -> Option<&str> {
        self.pending_review_error.as_deref()
    }

    pub(crate) fn set_pending_review_error(&mut self, error: impl Into<String>) {
        self.pending_review_error = Some(error.into());
    }

    pub(crate) fn clear_pending_review_error(&mut self) {
        self.pending_review_error = None;
    }

    pub(crate) fn next_review_data_generation(&mut self) -> u64 {
        self.review_data_generation = self.review_data_generation.saturating_add(1);
        self.review_data_generation
    }

    pub(crate) fn review_data_generation(&self) -> u64 {
        self.review_data_generation
    }

    pub(crate) fn clear_composer_and_action_state(&mut self) {
        self.review_composer_state.clear();
        self.review_comment_error = None;
        self.review_thread_reply_error = None;
        self.review_comment_edit_error = None;
        self.review_comment_action_comment_id = None;
        self.review_comment_action_error = None;
        self.review_reaction_action = None;
        self.review_reaction_error = None;
    }

    pub(crate) fn clear_submission_errors(&mut self) {
        self.review_comment_error = None;
        self.pending_review_error = None;
    }

    pub(crate) fn clear_review_data(&mut self) {
        self.pull_request_reviews.clear();
        self.review_threads.clear();
        self.clear_composer_and_action_state();
        self.pending_review = None;
    }

    pub(crate) fn restore_review_snapshot(
        &mut self,
        pull_request_reviews: Vec<PullRequestReview>,
        review_threads: Vec<ReviewThread>,
        pending_review: Option<PendingReviewSession>,
        current_user_login: Option<String>,
        reviews_loaded: bool,
    ) {
        self.pull_request_reviews = pull_request_reviews;
        self.review_threads = review_threads;
        self.pending_review = pending_review;
        self.current_user_login = current_user_login;
        if reviews_loaded {
            self.apply_reviews_success();
        } else {
            self.reset_reviews_load();
        }
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
        self.apply_loaded_review_threads(review_threads)
    }

    pub(crate) fn replace_loaded_review_threads(
        &mut self,
        review_threads: Vec<ReviewThread>,
    ) -> usize {
        self.apply_loaded_review_threads(review_threads)
    }

    pub(crate) fn replace_reviews_and_loaded_threads(
        &mut self,
        reviews: Vec<PullRequestReview>,
        review_threads: Vec<ReviewThread>,
    ) -> usize {
        self.pull_request_reviews = reviews;
        self.apply_loaded_review_threads(review_threads)
    }

    pub(crate) fn clear_pull_request_reviews(&mut self) {
        self.pull_request_reviews.clear();
    }

    fn apply_loaded_review_threads(&mut self, mut review_threads: Vec<ReviewThread>) -> usize {
        let settled_thread_state_overrides = apply_review_thread_state_overrides(
            &mut review_threads,
            &self.review_thread_state_overrides,
        );
        let settled_reaction_overrides =
            apply_review_reaction_overrides(&mut review_threads, &self.review_reaction_overrides);
        self.remove_review_thread_state_overrides(settled_thread_state_overrides);
        self.remove_review_reaction_overrides(settled_reaction_overrides);
        self.review_threads = merge_optimistic_review_threads(review_threads, &self.review_threads);
        self.unresolved_thread_count()
    }

    pub(crate) fn unresolved_thread_count(&self) -> usize {
        unresolved_review_thread_count(&self.review_threads)
    }

    pub(crate) fn set_review_thread_state(&mut self, thread_id: &str, state: ReviewThreadState) {
        if let Some(thread) = self
            .review_threads
            .iter_mut()
            .find(|thread| thread.id == thread_id)
        {
            thread.state = state;
        }
    }

    pub(crate) fn review_comment(&self, comment_id: &str) -> Option<&ReviewComment> {
        self.review_threads
            .iter()
            .flat_map(|thread| thread.comments.iter())
            .find(|comment| comment.id == comment_id)
    }

    pub(crate) fn review_comment_mut(&mut self, comment_id: &str) -> Option<&mut ReviewComment> {
        self.review_threads
            .iter_mut()
            .flat_map(|thread| thread.comments.iter_mut())
            .find(|comment| comment.id == comment_id)
    }

    pub(crate) fn remove_review_comment(&mut self, comment_id: &str) {
        remove_review_comment_from_threads(&mut self.review_threads, comment_id);
    }

    pub(crate) fn rollback_pending_review_comment_count(
        &mut self,
        previous_pending_review: Option<&PendingReviewSession>,
    ) {
        rollback_pending_review_comment_count(&mut self.pending_review, previous_pending_review);
    }

    pub(crate) fn set_pending_review(&mut self, pending_review: PendingReviewSession) {
        self.pending_review = Some(pending_review);
    }

    pub(crate) fn set_review_comment_reaction(
        &mut self,
        comment_id: &str,
        content: ReactionContent,
        viewer_has_reacted: bool,
    ) {
        if let Some(comment) = self.review_comment_mut(comment_id) {
            set_review_comment_reaction_state(comment, content, viewer_has_reacted);
        }
    }

    pub(crate) fn insert_optimistic_review_thread(
        &mut self,
        range: ReviewCommentRange,
        body: String,
    ) -> OptimisticReviewCommentHandle {
        let sequence = self.next_local_review_comment_sequence();
        let comment_id = format!("{LOCAL_REVIEW_COMMENT_ID_PREFIX}{sequence}");
        let comment = self.optimistic_review_comment(
            comment_id.clone(),
            Some(review_position_from_range(&range)),
            body,
        );

        self.review_threads.push(ReviewThread {
            id: format!("{LOCAL_REVIEW_THREAD_ID_PREFIX}{sequence}"),
            path: range.path.clone(),
            range: Some(range),
            state: ReviewThreadState::Unresolved,
            comments: vec![comment],
        });

        OptimisticReviewCommentHandle { comment_id }
    }

    pub(crate) fn append_optimistic_review_reply(
        &mut self,
        thread_id: &str,
        body: String,
    ) -> Option<OptimisticReviewCommentHandle> {
        let thread_index = self
            .review_threads
            .iter()
            .position(|thread| thread.id == thread_id)?;

        let position = self.review_threads[thread_index]
            .range
            .as_ref()
            .map(review_position_from_range)
            .or_else(|| {
                self.review_threads[thread_index]
                    .comments
                    .iter()
                    .find_map(|comment| comment.position.clone())
            });
        let sequence = self.next_local_review_comment_sequence();
        let comment_id = format!("{LOCAL_REVIEW_COMMENT_ID_PREFIX}{sequence}");
        let comment = self.optimistic_review_comment(comment_id.clone(), position, body);

        self.review_threads[thread_index].comments.push(comment);

        Some(OptimisticReviewCommentHandle { comment_id })
    }

    fn optimistic_review_comment(
        &self,
        id: String,
        position: Option<ReviewCommentPosition>,
        body: String,
    ) -> ReviewComment {
        ReviewComment {
            id,
            author: self
                .current_user_login
                .clone()
                .unwrap_or_else(|| "you".to_string()),
            author_avatar_url: None,
            body,
            created_at: Utc::now(),
            updated_at: None,
            position,
            viewer_did_author: true,
            viewer_can_update: false,
            viewer_can_delete: false,
            viewer_can_react: false,
            reactions: Vec::new(),
        }
    }

    fn next_local_review_comment_sequence(&mut self) -> u64 {
        self.local_review_comment_sequence = self.local_review_comment_sequence.saturating_add(1);
        self.local_review_comment_sequence
    }

    fn remove_review_reaction_overrides(&mut self, keys: Vec<ReviewReactionKey>) {
        for key in keys {
            self.review_reaction_overrides.remove(&key);
        }
    }

    fn remove_review_thread_state_overrides(&mut self, thread_ids: Vec<String>) {
        for thread_id in thread_ids {
            self.review_thread_state_overrides.remove(&thread_id);
        }
    }
}

impl ReviewComposerState {
    pub(crate) fn new(
        comment_input: Entity<InputState>,
        thread_reply_input: Entity<InputState>,
        comment_edit_input: Entity<InputState>,
        pending_review_body_input: Entity<InputState>,
    ) -> Self {
        Self {
            mode: ReviewComposerMode::Idle,
            comment_input,
            thread_reply_input,
            comment_edit_input,
            pending_review_body_input,
        }
    }

    pub(crate) fn start_line_selection(&mut self, target: ReviewLineTarget) {
        self.mode = ReviewComposerMode::Selecting {
            line_selection: ReviewLineSelection {
                anchor: target.clone(),
                current: target,
            },
        };
    }

    pub(crate) fn extend_line_selection(&mut self, target: ReviewLineTarget) {
        if let ReviewComposerMode::Selecting { line_selection } = &mut self.mode {
            line_selection.current = target;
        }
    }

    pub(crate) fn take_line_selection(&mut self) -> Option<ReviewLineSelection> {
        let ReviewComposerMode::Selecting { line_selection } =
            std::mem::replace(&mut self.mode, ReviewComposerMode::Idle)
        else {
            return None;
        };

        Some(line_selection)
    }

    pub(crate) fn open_inline(
        &mut self,
        composer: ReviewComposer,
        line_selection: ReviewLineSelection,
    ) {
        self.mode = ReviewComposerMode::Inline {
            composer,
            line_selection,
        };
    }

    pub(crate) fn open_thread_reply(&mut self, thread_id: String) {
        self.mode = ReviewComposerMode::ThreadReply { thread_id };
    }

    pub(crate) fn open_comment_edit(&mut self, comment_id: String) {
        self.mode = ReviewComposerMode::CommentEdit { comment_id };
    }

    pub(crate) fn clear(&mut self) {
        self.mode = ReviewComposerMode::Idle;
    }

    pub(crate) fn inline_composer(&self) -> Option<&ReviewComposer> {
        match &self.mode {
            ReviewComposerMode::Inline { composer, .. } => Some(composer),
            ReviewComposerMode::Idle
            | ReviewComposerMode::Selecting { .. }
            | ReviewComposerMode::ThreadReply { .. }
            | ReviewComposerMode::CommentEdit { .. } => None,
        }
    }

    pub(crate) fn line_selection(&self) -> Option<&ReviewLineSelection> {
        match &self.mode {
            ReviewComposerMode::Selecting { line_selection }
            | ReviewComposerMode::Inline { line_selection, .. } => Some(line_selection),
            ReviewComposerMode::Idle
            | ReviewComposerMode::ThreadReply { .. }
            | ReviewComposerMode::CommentEdit { .. } => None,
        }
    }

    pub(crate) fn active_thread_reply(&self) -> Option<&str> {
        match &self.mode {
            ReviewComposerMode::ThreadReply { thread_id } => Some(thread_id.as_str()),
            ReviewComposerMode::Idle
            | ReviewComposerMode::Selecting { .. }
            | ReviewComposerMode::Inline { .. }
            | ReviewComposerMode::CommentEdit { .. } => None,
        }
    }

    pub(crate) fn active_comment_edit(&self) -> Option<&str> {
        match &self.mode {
            ReviewComposerMode::CommentEdit { comment_id } => Some(comment_id.as_str()),
            ReviewComposerMode::Idle
            | ReviewComposerMode::Selecting { .. }
            | ReviewComposerMode::Inline { .. }
            | ReviewComposerMode::ThreadReply { .. } => None,
        }
    }

    pub(crate) fn take_active_comment_edit_if(
        &mut self,
        predicate: impl FnOnce(&str) -> bool,
    ) -> Option<String> {
        let ReviewComposerMode::CommentEdit { comment_id } = &self.mode else {
            return None;
        };
        if !predicate(comment_id) {
            return None;
        }
        let ReviewComposerMode::CommentEdit { comment_id } =
            std::mem::replace(&mut self.mode, ReviewComposerMode::Idle)
        else {
            return None;
        };
        Some(comment_id)
    }
}

fn load_status_from_loaded(loaded: bool) -> LoadStatus {
    if loaded {
        LoadStatus::Loaded
    } else {
        LoadStatus::Idle
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use harbor_domain::{ReviewCommentRange, ReviewSide};

    #[test]
    fn pull_request_detail_state_tracks_section_load_transitions() {
        let mut state =
            PullRequestDetailUiState::new(Vec::new(), Vec::new(), WorkflowLogState::new());

        assert!(state.should_load_details());
        state.start_details_load();
        assert!(state.details_loading());
        assert!(!state.should_load_details());

        state.apply_details_failure("metadata failed");
        assert_eq!(state.details_error(), Some("metadata failed"));
        assert!(!state.should_load_details());

        state.reset_for_selection();
        assert!(state.should_load_details());

        state.restore_loaded_sections(PullRequestDetailLoadedState {
            details: true,
            files: false,
            checks: true,
            workflows: false,
            reviews: true,
        });
        assert!(state.details_loaded());
        assert!(!state.should_load_checks());
        assert_eq!(
            state.loaded_sections(true),
            PullRequestDetailLoadedState {
                details: true,
                files: false,
                checks: true,
                workflows: false,
                reviews: true,
            }
        );
    }

    #[test]
    fn pull_request_selection_state_restores_indexes_with_bounds() {
        let mut state = PullRequestSelectionState::default();

        state.restore_pull_request_index(4, 2);
        assert_eq!(state.pull_request_index(), 1);

        state.set_diff_position(2, 3);
        assert_eq!(state.file_index(), 2);
        assert_eq!(state.hunk_index(), 3);

        state.restore_diff_position(5, 8, 3);
        assert_eq!(state.file_index(), 2);
        assert_eq!(state.hunk_index(), 8);

        state.select_file_index(1);
        assert_eq!(state.file_index(), 1);
        assert_eq!(state.hunk_index(), 0);

        state.reset_pull_request_index();
        state.reset_diff_selection();
        assert_eq!(state, PullRequestSelectionState::default());
    }

    #[test]
    fn review_composer_modes_are_mutually_exclusive() {
        let target = ReviewLineTarget {
            hunk_index: 0,
            line_index: 1,
            range: ReviewCommentRange {
                path: "src/lib.rs".to_string(),
                line: 10,
                side: ReviewSide::Right,
                start_line: None,
                start_side: None,
            },
        };
        let selection = ReviewLineSelection {
            anchor: target.clone(),
            current: target.clone(),
        };
        let composer = ReviewComposer {
            anchor: target,
            range: selection.current.range.clone(),
        };

        let modes = [
            ReviewComposerMode::Idle,
            ReviewComposerMode::Selecting {
                line_selection: selection.clone(),
            },
            ReviewComposerMode::Inline {
                composer,
                line_selection: selection,
            },
            ReviewComposerMode::ThreadReply {
                thread_id: "thread-1".to_string(),
            },
            ReviewComposerMode::CommentEdit {
                comment_id: "comment-1".to_string(),
            },
        ];

        for mode in modes {
            let active_count = [
                matches!(mode, ReviewComposerMode::Selecting { .. }),
                matches!(mode, ReviewComposerMode::Inline { .. }),
                matches!(mode, ReviewComposerMode::ThreadReply { .. }),
                matches!(mode, ReviewComposerMode::CommentEdit { .. }),
            ]
            .into_iter()
            .filter(|active| *active)
            .count();
            assert!(active_count <= 1);
        }
    }
}
