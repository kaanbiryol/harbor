use gpui::{App, Entity};
use gpui_component::input::InputState;
use harbor_domain::ReviewThread;

use crate::workspace::{
    AppView, PendingReviewSession, ReviewCommentUiError, ReviewComposer, ReviewLineSelection,
    ReviewReactionAction, ReviewThreadUiError,
};

pub(super) struct DiffRowRenderState<'a> {
    pub(super) review_threads: &'a [ReviewThread],
    pub(super) review_composer: Option<&'a ReviewComposer>,
    pub(super) review_line_selection: Option<&'a ReviewLineSelection>,
    pub(super) pending_review: Option<&'a PendingReviewSession>,
    pub(super) review_comment_input: Entity<InputState>,
    pub(super) review_comment_body_empty: bool,
    pub(super) is_submitting_review_comment: bool,
    pub(super) review_comment_error: Option<&'a str>,
    pub(super) active_review_thread_reply: Option<&'a str>,
    pub(super) review_thread_reply_input: Entity<InputState>,
    pub(super) review_thread_reply_body_empty: bool,
    pub(super) is_submitting_review_thread_reply: bool,
    pub(super) review_thread_reply_error: Option<&'a ReviewThreadUiError>,
    pub(super) review_thread_action_thread_id: Option<&'a str>,
    pub(super) review_thread_action_error: Option<&'a ReviewThreadUiError>,
    pub(super) active_review_comment_edit: Option<&'a str>,
    pub(super) review_comment_edit_input: Entity<InputState>,
    pub(super) review_comment_edit_body_empty: bool,
    pub(super) is_submitting_review_comment_edit: bool,
    pub(super) review_comment_edit_error: Option<&'a ReviewCommentUiError>,
    pub(super) review_comment_action_comment_id: Option<&'a str>,
    pub(super) review_comment_action_error: Option<&'a ReviewCommentUiError>,
    pub(super) review_reaction_action: Option<&'a ReviewReactionAction>,
    pub(super) review_reaction_error: Option<&'a ReviewCommentUiError>,
    pub(super) active_file: usize,
    pub(super) view_entity: Entity<AppView>,
}

impl<'a> DiffRowRenderState<'a> {
    pub(super) fn from_view(view: &'a AppView, cx: &App, view_entity: Entity<AppView>) -> Self {
        Self {
            review_threads: &view.review_state.review_threads,
            review_composer: view.review_state.review_composer_state.inline_composer(),
            review_line_selection: view.review_state.review_composer_state.line_selection(),
            pending_review: view.review_state.pending_review(),
            review_comment_input: view
                .review_state
                .review_composer_state
                .comment_input
                .clone(),
            review_comment_body_empty: view
                .review_state
                .review_composer_state
                .comment_input
                .read(cx)
                .value()
                .trim()
                .is_empty(),
            is_submitting_review_comment: view.review_state.is_submitting_review_comment(),
            review_comment_error: view.review_state.review_comment_error(),
            active_review_thread_reply: view
                .review_state
                .review_composer_state
                .active_thread_reply(),
            review_thread_reply_input: view
                .review_state
                .review_composer_state
                .thread_reply_input
                .clone(),
            review_thread_reply_body_empty: view
                .review_state
                .review_composer_state
                .thread_reply_input
                .read(cx)
                .value()
                .trim()
                .is_empty(),
            is_submitting_review_thread_reply: view
                .review_state
                .is_submitting_review_thread_reply(),
            review_thread_reply_error: view.review_state.review_thread_reply_error(),
            review_thread_action_thread_id: view.review_state.review_thread_action_thread_id(),
            review_thread_action_error: view.review_state.review_thread_action_error(),
            active_review_comment_edit: view
                .review_state
                .review_composer_state
                .active_comment_edit(),
            review_comment_edit_input: view
                .review_state
                .review_composer_state
                .comment_edit_input
                .clone(),
            review_comment_edit_body_empty: view
                .review_state
                .review_composer_state
                .comment_edit_input
                .read(cx)
                .value()
                .trim()
                .is_empty(),
            is_submitting_review_comment_edit: view
                .review_state
                .is_submitting_review_comment_edit(),
            review_comment_edit_error: view.review_state.review_comment_edit_error(),
            review_comment_action_comment_id: view.review_state.review_comment_action_comment_id(),
            review_comment_action_error: view.review_state.review_comment_action_error(),
            review_reaction_action: view.review_state.review_reaction_action(),
            review_reaction_error: view.review_state.review_reaction_error(),
            active_file: view.active_file_index(),
            view_entity,
        }
    }
}
