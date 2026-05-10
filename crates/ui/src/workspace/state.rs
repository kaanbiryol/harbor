use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
    sync::Arc,
};

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
    ReviewComposer, ReviewLineSelection, ReviewReactionAction, ReviewReactionKey,
    ReviewThreadUiError, notifications::NotificationSink,
};
use crate::diff::ParsedDiff;

#[derive(Default)]
pub(crate) struct WorkspaceTasks {
    pub(crate) pr_list_task: Option<Task<()>>,
    pub(crate) pr_detail_tasks: Vec<Task<()>>,
    pub(crate) repository_task: Option<Task<()>>,
    pub(crate) local_task: Option<Task<()>>,
    pub(crate) external_app_availability_task: Option<Task<()>>,
    pub(crate) sync_task: Option<Task<()>>,
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

pub(crate) struct PullRequestDetailUiState {
    pub(crate) files: Vec<DiffFile>,
    pub(crate) diffs: Vec<Option<ParsedDiff>>,
    pub(crate) check_runs: Vec<CheckRun>,
    pub(crate) workflow_runs: Vec<WorkflowRun>,
    pub(crate) workflow_jobs: Vec<WorkflowJob>,
    pub(crate) pull_request_detail_cache:
        HashMap<PullRequestDetailCacheKey, PullRequestDetailSnapshot>,
    pub(crate) detail_loaded: PullRequestDetailLoadedState,
    pub(crate) detail_loading: PullRequestDetailLoadingState,
    pub(crate) log_state: WorkflowLogState,
    pub(crate) details_error: Option<String>,
    pub(crate) files_error: Option<String>,
    pub(crate) checks_error: Option<String>,
    pub(crate) workflows_error: Option<String>,
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
    pub(crate) reviews_error: Option<String>,
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

#[derive(Default)]
pub(crate) struct PullRequestDetailLoadingState {
    pub(crate) details: bool,
    pub(crate) files: bool,
    pub(crate) checks: bool,
    pub(crate) workflows: bool,
    pub(crate) reviews: bool,
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
    pub(crate) composer: Option<ReviewComposer>,
    pub(crate) line_selection: Option<ReviewLineSelection>,
    pub(crate) comment_input: Entity<InputState>,
    pub(crate) thread_reply_thread_id: Option<String>,
    pub(crate) thread_reply_input: Entity<InputState>,
    pub(crate) comment_edit_comment_id: Option<String>,
    pub(crate) comment_edit_input: Entity<InputState>,
    pub(crate) pending_review_body_input: Entity<InputState>,
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
}
