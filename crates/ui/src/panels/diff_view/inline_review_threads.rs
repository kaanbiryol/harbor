use gpui::{Entity, IntoElement, div, prelude::*, px};
use gpui_component::input::InputState;
use harbor_domain::ReviewThreadState;

use crate::{
    panels::{
        review::review_thread_state_label,
        review_thread_chrome::{
            ReviewThreadActionIds, ReviewThreadActionsChrome, ReviewThreadActionsState,
            ReviewThreadReplyComposerChrome, ReviewThreadReplyComposerIds,
            ReviewThreadReplyComposerState as SharedReviewThreadReplyComposerState,
            render_review_thread_actions,
            render_review_thread_reply_composer as render_shared_review_thread_reply_composer,
            render_review_thread_status_pill,
        },
    },
    visual::color,
    workspace::AppView,
};

pub(crate) use crate::panels::review_thread_chrome::review_thread_ui_state;

use super::super::DIFF_ROW_HEIGHT;

pub(super) struct ReviewThreadHeaderState {
    pub(super) thread_id: String,
    pub(super) thread_state: ReviewThreadState,
    pub(super) active_reply: bool,
    pub(super) reply_button_disabled: bool,
    pub(super) action_running: bool,
    pub(super) can_toggle_resolution: bool,
    pub(super) view_entity: Entity<AppView>,
}

pub(super) struct ReviewThreadReplyComposerState {
    pub(super) thread_id: String,
    pub(super) input: Entity<InputState>,
    pub(super) disabled: bool,
    pub(super) submitting: bool,
    pub(super) error: Option<String>,
    pub(super) view_entity: Entity<AppView>,
}

pub(super) fn render_review_thread_header(state: ReviewThreadHeaderState) -> impl IntoElement {
    let ReviewThreadHeaderState {
        thread_id,
        thread_state,
        active_reply,
        reply_button_disabled,
        action_running,
        can_toggle_resolution,
        view_entity,
    } = state;
    let (label, color) = review_thread_state_label(thread_state);
    let is_resolved = thread_state == ReviewThreadState::Resolved;

    div()
        .border_b_1()
        .border_color(if is_resolved {
            color::border_subtle()
        } else {
            color::border()
        })
        .bg(if is_resolved {
            color::content_background()
        } else {
            color::elevated_background()
        })
        .px_2()
        .py_1()
        .flex()
        .items_center()
        .justify_between()
        .gap_3()
        .child(
            div()
                .min_w_0()
                .flex_1()
                .flex()
                .items_center()
                .gap_2()
                .child(render_review_thread_status_pill(label, color)),
        )
        .child(render_review_thread_actions(ReviewThreadActionsState {
            ids: ReviewThreadActionIds::inline(&thread_id),
            thread_id,
            active_reply,
            reply_button_disabled,
            is_resolved,
            action_running,
            can_toggle_resolution,
            show_reply_button: true,
            show_toggle_icon: true,
            chrome: ReviewThreadActionsChrome::Inline,
            view_entity,
        }))
}

pub(super) fn render_review_thread_reply_composer(
    state: ReviewThreadReplyComposerState,
) -> impl IntoElement {
    let ReviewThreadReplyComposerState {
        thread_id,
        input,
        disabled,
        submitting,
        error,
        view_entity,
    } = state;

    render_shared_review_thread_reply_composer(SharedReviewThreadReplyComposerState {
        ids: ReviewThreadReplyComposerIds::inline(&thread_id),
        thread_id,
        input,
        input_height: px(DIFF_ROW_HEIGHT * 2.0),
        disabled,
        submitting,
        error,
        chrome: ReviewThreadReplyComposerChrome::Inline,
        view_entity,
    })
}
