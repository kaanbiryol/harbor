use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
    sync::Arc,
};

use chrono::Utc;
use gpui::{Entity, Task, UniformListScrollHandle};
use gpui_component::input::InputState;
use harbor_domain::{
    CheckRun, DiffFile, PullRequestReview, RepoId, ReviewThread, ReviewThreadState, WorkflowJob,
    WorkflowRun,
};
use harbor_logs::LogChunk;
use harbor_storage::SqliteStore;
use harbor_sync::{ActivityState, SyncPolicy, SyncState, SyncTarget};

use super::{
    PendingReviewSession, PullRequestDetailCacheKey, PullRequestDetailSnapshot,
    PullRequestInboxCacheKey, PullRequestInboxMode, PullRequestInboxSnapshot, ReviewCommentUiError,
    ReviewComposer, ReviewLineSelection, ReviewLineTarget, ReviewReactionAction, ReviewReactionKey,
    ReviewThreadUiError, notifications::NotificationSink,
};
use crate::diff::ParsedDiff;

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) enum LoadStatus {
    #[default]
    Idle,
    Loading,
    Loaded,
    Failed(String),
}

impl LoadStatus {
    pub(crate) fn start(&mut self) {
        *self = Self::Loading;
    }

    pub(crate) fn succeed(&mut self) {
        *self = Self::Loaded;
    }

    pub(crate) fn fail(&mut self, error: impl Into<String>) {
        *self = Self::Failed(error.into());
    }

    pub(crate) fn reset(&mut self) {
        *self = Self::Idle;
    }

    pub(crate) fn is_loading(&self) -> bool {
        matches!(self, Self::Loading)
    }

    pub(crate) fn is_loaded(&self) -> bool {
        matches!(self, Self::Loaded)
    }

    pub(crate) fn is_finished(&self) -> bool {
        matches!(self, Self::Loaded | Self::Failed(_))
    }

    pub(crate) fn error(&self) -> Option<&str> {
        match self {
            Self::Failed(error) => Some(error),
            Self::Idle | Self::Loading | Self::Loaded => None,
        }
    }

    pub(crate) fn clear_error(&mut self) {
        if matches!(self, Self::Failed(_)) {
            self.reset();
        }
    }
}

#[derive(Default)]
pub(crate) struct WorkspaceTasks {
    pub(crate) pr_list_task: Option<Task<()>>,
    pub(crate) pr_detail_tasks: Vec<Task<()>>,
    pub(crate) repository_task: Option<Task<()>>,
    pub(crate) local_task: Option<Task<()>>,
    pub(crate) external_app_availability_task: Option<Task<()>>,
    pub(crate) sync_task: Option<Task<()>>,
}

impl WorkspaceTasks {
    pub(crate) fn clear_pull_request_detail_tasks(&mut self) {
        self.pr_detail_tasks.clear();
    }

    pub(crate) fn set_pull_request_list_task(&mut self, task: Task<()>) {
        self.pr_list_task = Some(task);
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
}

pub(crate) struct RepositoryUiState {
    pub(crate) repositories: Vec<RepoId>,
    pub(crate) repository_switcher_open: bool,
    pub(crate) repository_switcher_selection: usize,
    pub(crate) repository_search_input: Entity<InputState>,
    pub(crate) configured_repo: Option<RepoId>,
    pub(crate) repository_store: Option<SqliteStore>,
    pub(crate) repository_local_paths: HashMap<RepoId, PathBuf>,
    pub(crate) is_loading_repositories: bool,
    pub(crate) repository_error: Option<String>,
}

impl RepositoryUiState {
    pub(crate) fn start_loading(&mut self) {
        self.is_loading_repositories = true;
    }

    pub(crate) fn finish_loading(&mut self) {
        self.is_loading_repositories = false;
    }

    pub(crate) fn set_store(&mut self, store: SqliteStore) {
        self.repository_store = Some(store);
        self.repository_error = None;
    }

    pub(crate) fn clear_store_with_error(&mut self, error: impl Into<String>) {
        self.repository_store = None;
        self.is_loading_repositories = false;
        self.repository_error = Some(error.into());
    }

    pub(crate) fn set_error(&mut self, error: impl Into<String>) {
        self.repository_error = Some(error.into());
    }

    pub(crate) fn clear_error(&mut self) {
        self.repository_error = None;
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
    pub(crate) pull_request_detail_cache:
        HashMap<PullRequestDetailCacheKey, PullRequestDetailSnapshot>,
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
    pub(crate) pending_review: Option<PendingReviewSession>,
    pub(crate) is_submitting_review_comment: bool,
    pub(crate) is_submitting_review_thread_reply: bool,
    pub(crate) is_submitting_review_comment_edit: bool,
    pub(crate) is_submitting_pending_review: bool,
    pub(crate) review_thread_action_thread_id: Option<String>,
    pub(crate) review_comment_action_comment_id: Option<String>,
    pub(crate) review_reaction_action: Option<ReviewReactionAction>,
    pub(crate) review_thread_state_overrides: HashMap<String, ReviewThreadState>,
    pub(crate) review_reaction_overrides: HashMap<ReviewReactionKey, bool>,
    reviews_load: LoadStatus,
    pub(crate) review_comment_error: Option<String>,
    pub(crate) review_thread_reply_error: Option<ReviewThreadUiError>,
    pub(crate) review_thread_action_error: Option<ReviewThreadUiError>,
    pub(crate) review_comment_edit_error: Option<ReviewCommentUiError>,
    pub(crate) review_comment_action_error: Option<ReviewCommentUiError>,
    pub(crate) review_reaction_error: Option<ReviewCommentUiError>,
    pub(crate) pending_review_error: Option<String>,
    pub(crate) current_user_login: Option<String>,
    pub(crate) local_review_comment_sequence: u64,
    pub(crate) review_data_generation: u64,
}

pub(crate) struct NotificationState {
    pub(crate) notification_sink: Arc<dyn NotificationSink>,
    pub(crate) notification_dedupe: HashSet<String>,
    pub(crate) notifications_enabled: bool,
}

pub(crate) struct SyncRuntimeState {
    pub(crate) activity_state: ActivityState,
    pub(crate) sync_policy: SyncPolicy,
    pub(crate) sync_states: HashMap<SyncTarget, SyncState>,
    pub(crate) did_focus: bool,
}

impl SyncRuntimeState {
    pub(crate) fn set_activity(&mut self, activity_state: ActivityState) {
        self.activity_state = activity_state;
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
}

#[derive(Default)]
pub(crate) struct PullRequestInboxState {
    pub(crate) visible: bool,
    pub(crate) mode: PullRequestInboxMode,
    pub(crate) cache: HashMap<PullRequestInboxCacheKey, PullRequestInboxSnapshot>,
}

impl PullRequestInboxState {
    pub(crate) fn visible_by_default() -> Self {
        Self {
            visible: true,
            ..Self::default()
        }
    }
}

#[derive(Default)]
pub(crate) struct DiffSelectionState {
    pub(crate) file_index: usize,
    pub(crate) hunk_index: usize,
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
    pub(crate) chunk: Option<LogChunk>,
    pub(crate) task: Option<Task<()>>,
    pub(crate) list_scroll: UniformListScrollHandle,
    pub(crate) is_loading: bool,
    pub(crate) error: Option<String>,
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
    fn load_status_transitions_and_error_access() {
        let mut status = LoadStatus::default();
        assert!(!status.is_loading());
        assert!(!status.is_loaded());
        assert_eq!(status.error(), None);

        status.start();
        assert!(status.is_loading());
        assert!(!status.is_finished());

        status.fail("network");
        assert!(!status.is_loading());
        assert!(status.is_finished());
        assert_eq!(status.error(), Some("network"));

        status.succeed();
        assert!(status.is_loaded());
        assert_eq!(status.error(), None);

        status.reset();
        assert_eq!(status, LoadStatus::Idle);
    }

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
