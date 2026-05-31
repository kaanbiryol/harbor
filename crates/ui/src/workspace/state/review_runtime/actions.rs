use crate::workspace::{
    ReviewCommentUiError, ReviewReactionAction, ReviewRuntimeState, ReviewThreadUiError,
};

impl ReviewRuntimeState {
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
}
