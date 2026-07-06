use gpui::{AnyElement, Entity, SharedString, div, prelude::*, px};
use gpui_component::StyledExt;
use gpui_component::input::InputState;
use harbor_domain::{ReviewComment, ReviewThread};

use crate::{
    visual::color,
    workspace::{AppView, ReviewThreadUiError},
};

use super::{
    render_status_pill,
    review::{
        ReviewDiffPreview, render_review_author_link, render_review_avatar,
        render_review_diff_preview, review_comment_time_label, review_thread_location,
        review_thread_state_tone,
    },
    review_markdown::render_review_markdown_body,
    review_thread_chrome::{
        ReviewThreadActionIds, ReviewThreadActionsState, ReviewThreadReplyComposerChrome,
        ReviewThreadReplyComposerIds, ReviewThreadReplyComposerState, render_review_thread_actions,
        render_review_thread_reply_composer, review_thread_ui_state,
    },
};

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
    pub(crate) diff_preview: Option<ReviewDiffPreview>,
    pub(crate) mono_font_family: SharedString,
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
        diff_preview,
        mono_font_family,
        view_entity,
    } = state;
    let (label, tone) = review_thread_state_tone(thread.state);
    let latest_comment = thread.comments.last();
    let location = review_thread_location(thread);
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
    let comment_color = if is_resolved {
        color::text_muted()
    } else {
        color::text_secondary()
    };
    let reply_error = reply_error
        .filter(|error| error.thread_id == thread.id)
        .map(|error| error.message.clone());
    let action_error = action_error
        .filter(|error| error.thread_id == thread.id)
        .map(|error| error.message.clone());
    let thread_id = thread.id.clone();
    let header_author =
        latest_comment.map_or(thread.path.as_str(), |comment| comment.author.as_str());
    let header_avatar_url = latest_comment.and_then(|comment| comment.author_avatar_url.as_deref());
    let header_time = latest_comment.map(review_comment_time_label);

    div()
        .id(("review-thread-row", index))
        .w_full()
        .min_w_0()
        .flex_initial()
        .py_1()
        .child(
            div()
                .w_full()
                .min_w_0()
                .flex()
                .flex_col()
                .border_1()
                .border_color(row_border_color)
                .bg(row_bg_color)
                .child(
                    div()
                        .flex()
                        .items_center()
                        .justify_between()
                        .gap_3()
                        .px_3()
                        .py_2()
                        .border_b_1()
                        .border_color(color::border_subtle())
                        .child(
                            div()
                                .min_w_0()
                                .flex_1()
                                .flex()
                                .items_start()
                                .gap_2()
                                .child(render_review_avatar(header_author, header_avatar_url, 24.0))
                                .child(
                                    div()
                                        .min_w_0()
                                        .flex_1()
                                        .flex()
                                        .flex_col()
                                        .gap_1()
                                        .child(
                                            div()
                                                .flex()
                                                .items_baseline()
                                                .gap_2()
                                                .child({
                                                    if latest_comment.is_some() {
                                                        render_review_author_link(
                                                            format!(
                                                                "review-thread-author-link-{}",
                                                                thread.id
                                                            ),
                                                            header_author.to_string(),
                                                            path_color,
                                                        )
                                                        .into_any_element()
                                                    } else {
                                                        div()
                                                            .font_medium()
                                                            .text_color(path_color)
                                                            .child(header_author.to_string())
                                                            .into_any_element()
                                                    }
                                                })
                                                .child(
                                                    div()
                                                        .text_xs()
                                                        .text_color(metadata_color)
                                                        .child(header_time.map_or_else(
                                                            || "commented".to_string(),
                                                            |time| format!("commented {time}"),
                                                        )),
                                                ),
                                        )
                                        .child(
                                            div()
                                                .text_xs()
                                                .text_color(metadata_color)
                                                .child(review_thread_metadata(thread, &location)),
                                        ),
                                ),
                        )
                        .child(render_status_pill(label, tone)),
                )
                .when_some(diff_preview, move |element, preview| {
                    element.child(div().px_3().pt_2().child(render_review_diff_preview(
                        preview,
                        mono_font_family.clone(),
                    )))
                })
                .child(render_review_thread_comments(
                    thread,
                    metadata_color,
                    comment_color,
                ))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .justify_end()
                        .gap_2()
                        .px_3()
                        .py_2()
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
                        element.child(div().px_3().pb_3().child(
                            render_review_thread_reply_composer(ReviewThreadReplyComposerState {
                                ids: ReviewThreadReplyComposerIds::review_panel(&thread_id),
                                thread_id: thread_id.clone(),
                                input: review_thread_reply_input.clone(),
                                input_height: px(48.),
                                disabled: ui_state.reply_disabled,
                                submitting: ui_state.reply_submitting,
                                error: reply_error.clone(),
                                chrome: ReviewThreadReplyComposerChrome::Panel,
                                view_entity: view_entity.clone(),
                            }),
                        ))
                    }
                })
                .when_some(action_error, |element, error| {
                    element.child(
                        div()
                            .px_3()
                            .pb_3()
                            .text_xs()
                            .text_color(color::danger())
                            .child(error),
                    )
                }),
        )
        .into_any_element()
}

fn render_review_thread_comments(
    thread: &ReviewThread,
    metadata_color: gpui::Rgba,
    comment_color: gpui::Rgba,
) -> impl IntoElement {
    let first_comment = thread.comments.first();

    div()
        .flex()
        .flex_col()
        .gap_3()
        .px_3()
        .py_3()
        .when_some(first_comment, |element, comment| {
            element.child(div().text_sm().text_color(comment_color).child(
                render_review_markdown_body(
                    format!("review-thread-comment-body-{}", comment.id),
                    &comment.body,
                ),
            ))
        })
        .when(thread.comments.len() > 1, |element| {
            element.child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .pt_3()
                    .border_t_1()
                    .border_color(color::border_subtle())
                    .children(thread.comments.iter().skip(1).map(|comment| {
                        render_review_thread_reply(comment, metadata_color, comment_color)
                    })),
            )
        })
}

fn render_review_thread_reply(
    comment: &ReviewComment,
    metadata_color: gpui::Rgba,
    comment_color: gpui::Rgba,
) -> impl IntoElement {
    div()
        .min_w_0()
        .flex()
        .items_start()
        .gap_2()
        .border_1()
        .border_color(color::border_subtle())
        .bg(color::content_background())
        .px_2()
        .py_2()
        .child(render_review_avatar(
            &comment.author,
            comment.author_avatar_url.as_deref(),
            20.0,
        ))
        .child(
            div()
                .min_w_0()
                .flex_1()
                .flex()
                .flex_col()
                .gap_1()
                .child(
                    div()
                        .flex()
                        .items_baseline()
                        .gap_2()
                        .text_xs()
                        .child(render_review_author_link(
                            format!("review-thread-reply-author-link-{}", comment.id),
                            comment.author.clone(),
                            color::text_primary(),
                        ))
                        .child(
                            div()
                                .text_color(metadata_color)
                                .child(review_comment_time_label(comment)),
                        ),
                )
                .child(div().text_sm().text_color(comment_color).child(
                    render_review_markdown_body(
                        format!("review-thread-reply-body-{}", comment.id),
                        &comment.body,
                    ),
                )),
        )
}

fn review_thread_metadata(thread: &ReviewThread, location: &str) -> String {
    if thread.comments.len() > 1 {
        format!(
            "{}  {}  {} comments",
            thread.path,
            location,
            thread.comments.len()
        )
    } else {
        format!("{}  {}", thread.path, location)
    }
}
