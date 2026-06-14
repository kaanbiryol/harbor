use gpui::{AnyElement, Entity, div, prelude::*, px};
use gpui_component::input::InputState;
use harbor_domain::ReviewThread;

use crate::{
    visual::color,
    workspace::{AppView, ReviewThreadUiError},
};

use super::{
    render_status_pill,
    review::{review_thread_location, review_thread_state_tone, single_line},
    review_thread_chrome::{
        ReviewThreadActionIds, ReviewThreadActionsState, ReviewThreadReplyComposerChrome,
        ReviewThreadReplyComposerIds, ReviewThreadReplyComposerState, render_review_thread_actions,
        render_review_thread_reply_composer, review_thread_ui_state,
    },
};

const REVIEW_THREAD_ROW_HEIGHT: f32 = 144.0;
const EXPANDED_REVIEW_THREAD_ROW_HEIGHT: f32 = 224.0;

pub(crate) struct ReviewThreadRowRenderState<'a> {
    pub(crate) index: usize,
    pub(crate) thread: &'a ReviewThread,
    pub(crate) active_review_thread_reply: Option<&'a str>,
    pub(crate) review_thread_reply_input: Entity<InputState>,
    pub(crate) reply_body_empty: bool,
    pub(crate) is_submitting_reply: bool,
    pub(crate) reply_error: Option<&'a ReviewThreadUiError>,
    pub(crate) action_thread_id: Option<&'a str>,
    pub(crate) action_error: Option<&'a ReviewThreadUiError>,
    pub(crate) use_expanded_rows: bool,
    pub(crate) view_entity: Entity<AppView>,
}

pub(crate) fn render_review_thread_row(state: ReviewThreadRowRenderState<'_>) -> AnyElement {
    let ReviewThreadRowRenderState {
        index,
        thread,
        active_review_thread_reply,
        review_thread_reply_input,
        reply_body_empty,
        is_submitting_reply,
        reply_error,
        action_thread_id,
        action_error,
        use_expanded_rows,
        view_entity,
    } = state;
    let (label, tone) = review_thread_state_tone(thread.state);
    let latest_comment = thread.comments.last();
    let location = review_thread_location(thread);
    let preview = latest_comment
        .map(|comment| single_line(&comment.body))
        .unwrap_or_else(|| "No comments in this thread".to_string());
    let ui_state = review_thread_ui_state(
        thread,
        active_review_thread_reply,
        reply_body_empty,
        is_submitting_reply,
        action_thread_id,
    );
    let is_resolved = ui_state.is_resolved;
    let row_border_color = if is_resolved {
        color::border_subtle()
    } else {
        color::border()
    };
    let row_bg_color = if is_resolved {
        color::content_background()
    } else {
        color::app_background()
    };
    let row_hover_bg_color = if is_resolved {
        color::row_selected_subtle()
    } else {
        color::row_hover()
    };
    let path_color = if is_resolved {
        color::text_secondary()
    } else {
        color::text_primary()
    };
    let metadata_color = if is_resolved {
        color::text_disabled()
    } else {
        color::text_muted()
    };
    let reply_error = reply_error
        .filter(|error| error.thread_id == thread.id)
        .map(|error| error.message.clone());
    let action_error = action_error
        .filter(|error| error.thread_id == thread.id)
        .map(|error| error.message.clone());
    let thread_id = thread.id.clone();
    let row_height = if use_expanded_rows {
        EXPANDED_REVIEW_THREAD_ROW_HEIGHT
    } else {
        REVIEW_THREAD_ROW_HEIGHT
    };

    div()
        .id(("review-thread-row", index))
        .h(px(row_height))
        .w_full()
        .min_w_0()
        .flex()
        .flex_col()
        .gap_2()
        .px_3()
        .py_2()
        .border_1()
        .border_color(row_border_color)
        .bg(row_bg_color)
        .hover(move |style| style.bg(row_hover_bg_color))
        .child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .gap_3()
                .child(
                    div()
                        .min_w_0()
                        .flex_1()
                        .truncate()
                        .text_color(path_color)
                        .child(thread.path.clone()),
                )
                .child(render_status_pill(label, tone)),
        )
        .child(div().text_xs().text_color(metadata_color).child(format!(
            "{}  {} comments",
            location,
            thread.comments.len()
        )))
        .when_some(latest_comment, |element, comment| {
            element.child(
                div()
                    .text_xs()
                    .text_color(metadata_color)
                    .truncate()
                    .child(format!("{}: {}", comment.author, preview)),
            )
        })
        .child(
            div()
                .flex()
                .items_center()
                .justify_end()
                .child(render_review_thread_actions(ReviewThreadActionsState {
                    ids: ReviewThreadActionIds::review_panel(&thread_id),
                    thread_id: thread_id.clone(),
                    active_reply: ui_state.active_reply,
                    reply_button_disabled: ui_state.reply_button_disabled,
                    is_resolved,
                    action_running: ui_state.action_running,
                    can_toggle_resolution: ui_state.can_toggle_resolution,
                    show_toggle_icon: true,
                    view_entity: view_entity.clone(),
                })),
        )
        .when(ui_state.active_reply, {
            let view_entity = view_entity.clone();
            let thread_id = thread_id.clone();
            move |element| {
                element.child(render_review_thread_reply_composer(
                    ReviewThreadReplyComposerState {
                        ids: ReviewThreadReplyComposerIds::review_panel(&thread_id),
                        thread_id: thread_id.clone(),
                        input: review_thread_reply_input.clone(),
                        input_height: px(48.),
                        disabled: ui_state.reply_disabled,
                        submitting: ui_state.reply_submitting,
                        error: reply_error.clone(),
                        chrome: ReviewThreadReplyComposerChrome::Panel,
                        view_entity: view_entity.clone(),
                    },
                ))
            }
        })
        .when_some(action_error, |element, error| {
            element.child(div().text_xs().text_color(color::danger()).child(error))
        })
        .into_any_element()
}
