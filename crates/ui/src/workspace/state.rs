use std::collections::HashMap;

use gpui::{Entity, Task, UniformListScrollHandle};
use gpui_component::input::InputState;
use harbor_logs::LogChunk;

use super::{
    PullRequestInboxCacheKey, PullRequestInboxMode, PullRequestInboxSnapshot, ReviewComposer,
    ReviewLineSelection,
};

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
