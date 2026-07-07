use gpui::{AnyElement, Entity, SharedString, div, prelude::*, px};
use gpui_component::input::InputState;
use gpui_component::{StyledExt, tooltip::Tooltip};
use harbor_domain::{ReviewComment, ReviewThread};

use crate::{
    visual::{Tone, color, opacity},
    workspace::{AppView, ReviewThreadUiError},
};

use super::{
    render_status_pill,
    review::{
        ReviewDiffPreview, render_review_author_link, render_review_avatar,
        render_review_diff_preview, review_comment_time_label, review_comment_time_tooltip,
        review_thread_location, review_thread_state_tone,
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
    let use_resolved_low_emphasis = is_resolved && !ui_state.active_reply && action_error.is_none();
    let thread_id = thread.id.clone();
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
                .when(use_resolved_low_emphasis, |element| {
                    element
                        .opacity(opacity::DEEMPHASIZED_ITEM)
                        .hover(|element| element.opacity(opacity::DEEMPHASIZED_ITEM_HOVER))
                })
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
                                .flex_col()
                                .gap_1()
                                .child(
                                    div()
                                        .font_medium()
                                        .text_color(path_color)
                                        .child(thread.path.clone()),
                                )
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(metadata_color)
                                        .child(review_thread_metadata(thread, &location)),
                                ),
                        )
                        .child(
                            div()
                                .when(is_resolved, |element| {
                                    element.opacity(opacity::DEEMPHASIZED_ITEM)
                                })
                                .child(render_status_pill(
                                    label,
                                    if is_resolved { Tone::Neutral } else { tone },
                                )),
                        ),
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
                    is_resolved,
                ))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .justify_end()
                        .gap_2()
                        .border_t_1()
                        .border_color(color::border_subtle())
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
    is_resolved: bool,
) -> impl IntoElement {
    let thread_rail_color = if is_resolved {
        color::border_subtle()
    } else {
        color::border()
    };
    let author_color = if is_resolved {
        color::text_muted()
    } else {
        color::text_primary()
    };

    div()
        .flex()
        .flex_col()
        .gap_2()
        .px_3()
        .py_3()
        .children(thread.comments.iter().enumerate().map(|(index, comment)| {
            render_review_thread_comment(
                comment,
                index > 0,
                index + 1 < thread.comments.len(),
                thread_rail_color,
                author_color,
                metadata_color,
                comment_color,
            )
        }))
}

fn render_review_thread_comment(
    comment: &ReviewComment,
    is_reply: bool,
    show_thread_rail: bool,
    rail_color: gpui::Rgba,
    author_color: gpui::Rgba,
    metadata_color: gpui::Rgba,
    comment_color: gpui::Rgba,
) -> impl IntoElement {
    let author_id = if is_reply {
        format!("review-thread-reply-author-link-{}", comment.id)
    } else {
        format!("review-thread-comment-author-link-{}", comment.id)
    };
    let time_id = if is_reply {
        format!("review-thread-reply-time-{}", comment.id)
    } else {
        format!("review-thread-comment-time-{}", comment.id)
    };
    let body_id = if is_reply {
        format!("review-thread-reply-body-{}", comment.id)
    } else {
        format!("review-thread-comment-body-{}", comment.id)
    };

    div()
        .min_w_0()
        .flex()
        .gap_1()
        .child(render_review_thread_comment_gutter(
            comment,
            show_thread_rail,
            rail_color,
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
                        .items_center()
                        .gap_2()
                        .text_xs()
                        .child(render_review_author_link(
                            author_id,
                            comment.author.clone(),
                            author_color,
                        ))
                        .child(render_time_metadata(
                            time_id,
                            review_comment_time_label(comment),
                            Some(review_comment_time_tooltip(comment)),
                            metadata_color,
                        )),
                )
                .child(
                    div()
                        .text_sm()
                        .text_color(comment_color)
                        .child(render_review_markdown_body(body_id, &comment.body)),
                ),
        )
}

fn render_review_thread_comment_gutter(
    comment: &ReviewComment,
    show_thread_rail: bool,
    rail_color: gpui::Rgba,
) -> impl IntoElement {
    div()
        .relative()
        .min_h(px(28.0))
        .w(px(20.0))
        .flex_none()
        .child(render_review_avatar(
            &comment.author,
            comment.author_avatar_url.as_deref(),
            20.0,
        ))
        .when(show_thread_rail, |element| {
            element.child(
                div()
                    .absolute()
                    .top(px(24.0))
                    .bottom(px(-12.0))
                    .left(px(9.5))
                    .w(px(1.0))
                    .bg(rail_color),
            )
        })
}

fn render_time_metadata(
    id: String,
    label: String,
    tooltip: Option<String>,
    text_color: gpui::Rgba,
) -> impl IntoElement {
    div()
        .id(id)
        .text_xs()
        .text_color(text_color)
        .when_some(tooltip, |element, tooltip| {
            element.tooltip(move |window, cx| Tooltip::new(tooltip.clone()).build(window, cx))
        })
        .child(label)
}

fn review_thread_metadata(thread: &ReviewThread, location: &str) -> String {
    if thread.comments.len() > 1 {
        format!("{}  {} comments", location, thread.comments.len())
    } else {
        location.to_string()
    }
}
