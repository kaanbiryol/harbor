use gpui::Entity;
use gpui_component::input::InputState;

use crate::workspace::{ReviewComposer, ReviewLineSelection, ReviewLineTarget};

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
