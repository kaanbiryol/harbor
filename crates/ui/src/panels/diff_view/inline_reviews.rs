#[path = "inline_review_avatars.rs"]
mod avatars;
#[path = "inline_review_comment_actions.rs"]
mod comment_actions;
#[path = "inline_review_comments.rs"]
mod comments;
#[path = "inline_review_reactions.rs"]
mod reactions;
#[path = "inline_review_threads.rs"]
mod threads;

use gpui::{Entity, IntoElement, div, prelude::*, px};
use gpui_component::{
    Disableable, Sizable, StyledExt,
    button::{Button, ButtonVariants},
    input::{Input, InputState},
};
use harbor_domain::{ReviewThread, ReviewThreadState};

use crate::{
    visual::{color, font, opacity},
    workspace::{AppView, ReviewCommentSubmission, ReviewComposer, ReviewThreadUiError},
};

use super::{
    DIFF_ROW_HEIGHT, REVIEW_COMPOSER_MAX_WIDTH, REVIEW_MARKER_WIDTH,
    inline_review_layout::review_comment_range_label, render_line_number,
};
pub(super) use comments::ReviewCommentListRenderState;
use comments::{ReviewCommentRenderState, render_review_comment_inline};
use threads::{
    ReviewThreadHeaderState, ReviewThreadReplyComposerState, render_review_thread_header,
    render_review_thread_reply_composer,
};

const INLINE_REVIEW_THREAD_RECENT_REPLY_LIMIT: usize = 20;

pub(super) struct ReviewComposerRenderState {
    pub(super) composer: ReviewComposer,
    pub(super) has_pending_review: bool,
    pub(super) input: Entity<InputState>,
    pub(super) body_empty: bool,
    pub(super) is_submitting: bool,
    pub(super) error: Option<String>,
    pub(super) line_number_width: f32,
    pub(super) review_marker_width: f32,
    pub(super) view_entity: Entity<AppView>,
}

pub(super) struct ReviewThreadRenderState<'a> {
    pub(super) thread: &'a ReviewThread,
    pub(super) line_number_width: f32,
    pub(super) active_review_thread_reply: Option<&'a str>,
    pub(super) review_thread_reply_input: Entity<InputState>,
    pub(super) reply_body_empty: bool,
    pub(super) is_submitting_reply: bool,
    pub(super) reply_error: Option<&'a ReviewThreadUiError>,
    pub(super) action_thread_id: Option<&'a str>,
    pub(super) action_error: Option<&'a ReviewThreadUiError>,
    pub(super) comments: ReviewCommentListRenderState<'a>,
    pub(super) view_entity: Entity<AppView>,
}

pub(super) fn render_review_composer_inline(state: ReviewComposerRenderState) -> impl IntoElement {
    let ReviewComposerRenderState {
        composer,
        has_pending_review,
        input,
        body_empty,
        is_submitting,
        error,
        line_number_width,
        review_marker_width,
        view_entity,
    } = state;
    let target_label = review_comment_range_label(&composer.range);
    let submit_disabled = body_empty || is_submitting;

    render_inline_review_row(
        line_number_width,
        render_review_menu_marker(review_marker_width),
        div()
            .w_full()
            .max_w(px(REVIEW_COMPOSER_MAX_WIDTH))
            .border_1()
            .border_color(color::border_strong())
            .bg(color::panel_background())
            .px_3()
            .py_2()
            .child(
                div()
                    .pb_2()
                    .text_xs()
                    .font_medium()
                    .text_color(color::accent())
                    .child(format!("Comment on {target_label}")),
            )
            .child(
                div()
                    .w_full()
                    .border_1()
                    .border_color(color::border_strong())
                    .bg(color::input_background())
                    .px_2()
                    .py_1()
                    .child(
                        Input::new(&input)
                            .w_full()
                            .small()
                            .h(px(DIFF_ROW_HEIGHT * 3.0))
                            .appearance(false)
                            .bordered(false)
                            .focus_bordered(false),
                    ),
            )
            .when_some(error, |element, error| {
                element.child(
                    div()
                        .pt_2()
                        .text_xs()
                        .text_color(color::danger())
                        .child(error),
                )
            })
            .child(
                div()
                    .pt_2()
                    .flex()
                    .items_center()
                    .justify_end()
                    .gap_2()
                    .child(
                        Button::new("cancel-review-comment")
                            .label("Cancel")
                            .xsmall()
                            .ghost()
                            .disabled(is_submitting)
                            .on_click({
                                let view_entity = view_entity.clone();
                                move |_, window, cx| {
                                    view_entity.update(cx, |view, cx| {
                                        view.cancel_review_composer(window, cx);
                                    });
                                }
                            }),
                    )
                    .when(has_pending_review, {
                        let view_entity = view_entity.clone();
                        move |element| {
                            element.child(
                                Button::new("add-review-comment")
                                    .label("Add review comment")
                                    .xsmall()
                                    .primary()
                                    .loading(is_submitting)
                                    .disabled(submit_disabled)
                                    .on_click(move |_, _, cx| {
                                        view_entity.update(cx, |view, cx| {
                                            view.submit_review_comment(
                                                ReviewCommentSubmission::AddToReview,
                                                cx,
                                            );
                                        });
                                    }),
                            )
                        }
                    })
                    .when(!has_pending_review, {
                        let view_entity = view_entity.clone();
                        move |element| {
                            element
                                .child(
                                    Button::new("add-single-comment")
                                        .label("Add single comment")
                                        .xsmall()
                                        .outline()
                                        .loading(is_submitting)
                                        .disabled(submit_disabled)
                                        .on_click({
                                            let view_entity = view_entity.clone();
                                            move |_, _, cx| {
                                                view_entity.update(cx, |view, cx| {
                                                    view.submit_review_comment(
                                                        ReviewCommentSubmission::SingleComment,
                                                        cx,
                                                    );
                                                });
                                            }
                                        }),
                                )
                                .child(
                                    Button::new("start-review-comment")
                                        .label("Start review")
                                        .xsmall()
                                        .primary()
                                        .loading(is_submitting)
                                        .disabled(submit_disabled)
                                        .on_click(move |_, _, cx| {
                                            view_entity.update(cx, |view, cx| {
                                                view.submit_review_comment(
                                                    ReviewCommentSubmission::StartReview,
                                                    cx,
                                                );
                                            });
                                        }),
                                )
                        }
                    }),
            ),
    )
}

pub(super) fn render_review_thread_inline(
    state: ReviewThreadRenderState<'_>,
) -> impl IntoElement + use<> {
    let ReviewThreadRenderState {
        thread,
        line_number_width,
        active_review_thread_reply,
        review_thread_reply_input,
        reply_body_empty,
        is_submitting_reply,
        reply_error,
        action_thread_id,
        action_error,
        comments,
        view_entity,
    } = state;
    let ui_state = threads::review_thread_ui_state(
        thread,
        active_review_thread_reply,
        reply_body_empty,
        is_submitting_reply,
        action_thread_id,
    );
    let is_resolved = ui_state.is_resolved;
    let card_border_color = if is_resolved {
        color::border()
    } else {
        color::border_strong()
    };
    let card_bg_color = if is_resolved {
        color::content_background()
    } else {
        color::panel_background()
    };
    let reply_error = reply_error
        .filter(|error| error.thread_id == thread.id)
        .map(|error| error.message.clone());
    let action_error = action_error
        .filter(|error| error.thread_id == thread.id)
        .map(|error| error.message.clone());
    let use_resolved_low_emphasis = is_resolved && !ui_state.active_reply && action_error.is_none();
    let thread_id = thread.id.clone();
    let hidden_comment_count = hidden_inline_review_comment_count(thread.comments.len());
    let visible_reply_start_index = visible_inline_review_reply_start_index(thread.comments.len());

    render_inline_review_row(
        line_number_width,
        render_review_marker(
            1,
            thread.state == ReviewThreadState::Unresolved,
            REVIEW_MARKER_WIDTH,
        ),
        div()
            .w_full()
            .border_1()
            .border_color(card_border_color)
            .bg(card_bg_color)
            .rounded_xs()
            .overflow_hidden()
            .when(use_resolved_low_emphasis, |element| {
                element
                    .opacity(opacity::DEEMPHASIZED_ITEM)
                    .hover(|element| element.opacity(opacity::DEEMPHASIZED_ITEM_HOVER))
            })
            .child(render_review_thread_header(ReviewThreadHeaderState {
                thread_id: thread_id.clone(),
                thread_state: thread.state,
                active_reply: ui_state.active_reply,
                reply_button_disabled: ui_state.reply_button_disabled,
                action_running: ui_state.action_running,
                can_toggle_resolution: ui_state.can_toggle_resolution,
                view_entity: view_entity.clone(),
            }))
            .child(
                div()
                    .px_2()
                    .pb_2()
                    .children(
                        thread
                            .comments
                            .iter()
                            .take(1)
                            .enumerate()
                            .map(|(index, comment)| {
                                render_review_comment_inline(ReviewCommentRenderState::new(
                                    comment,
                                    index > 0,
                                    is_resolved,
                                    &comments,
                                ))
                            }),
                    )
                    .when(hidden_comment_count > 0, |element| {
                        element.child(render_hidden_review_comments_notice(
                            hidden_comment_count,
                            is_resolved,
                        ))
                    })
                    .children(
                        thread
                            .comments
                            .iter()
                            .enumerate()
                            .skip(visible_reply_start_index)
                            .map(|(index, comment)| {
                                render_review_comment_inline(ReviewCommentRenderState::new(
                                    comment,
                                    index > 0,
                                    is_resolved,
                                    &comments,
                                ))
                            }),
                    ),
            )
            .when(thread.comments.is_empty(), |element| {
                element.child(
                    div()
                        .px_2()
                        .pb_2()
                        .text_xs()
                        .text_color(if is_resolved {
                            color::text_disabled()
                        } else {
                            color::text_muted()
                        })
                        .child("No comments in this thread"),
                )
            })
            .when(ui_state.active_reply, {
                let view_entity = view_entity.clone();
                let thread_id = thread_id.clone();
                move |element| {
                    element.child(render_review_thread_reply_composer(
                        ReviewThreadReplyComposerState {
                            thread_id: thread_id.clone(),
                            input: review_thread_reply_input.clone(),
                            disabled: ui_state.reply_disabled,
                            submitting: ui_state.reply_submitting,
                            error: reply_error.clone(),
                            view_entity: view_entity.clone(),
                        },
                    ))
                }
            })
            .when_some(action_error, |element, error| {
                element.child(
                    div()
                        .px_2()
                        .pb_2()
                        .text_xs()
                        .text_color(color::danger())
                        .child(error),
                )
            }),
    )
}

fn render_inline_review_row(
    line_number_width: f32,
    marker: impl IntoElement,
    content: impl IntoElement,
) -> impl IntoElement {
    div()
        .min_h(px(DIFF_ROW_HEIGHT))
        .w_full()
        .flex()
        .items_start()
        .bg(color::content_background())
        .text_color(color::text_secondary())
        .font_family(font::UI)
        .child(render_line_number(None, line_number_width))
        .child(render_line_number(None, line_number_width))
        .child(marker)
        .child(
            div()
                .min_w_0()
                .flex_1()
                .flex()
                .flex_col()
                .gap_2()
                .py_1()
                .pr_3()
                .child(content),
        )
}

fn hidden_inline_review_comment_count(comment_count: usize) -> usize {
    comment_count.saturating_sub(INLINE_REVIEW_THREAD_RECENT_REPLY_LIMIT + 1)
}

fn visible_inline_review_reply_start_index(comment_count: usize) -> usize {
    if hidden_inline_review_comment_count(comment_count) > 0 {
        comment_count - INLINE_REVIEW_THREAD_RECENT_REPLY_LIMIT
    } else {
        1
    }
}

fn render_hidden_review_comments_notice(
    hidden_comment_count: usize,
    is_resolved: bool,
) -> impl IntoElement {
    let label = if hidden_comment_count == 1 {
        "1 older reply hidden in diff view".to_string()
    } else {
        format!("{hidden_comment_count} older replies hidden in diff view")
    };

    div()
        .mt_2()
        .ml(px(28.0))
        .border_l_1()
        .border_color(if is_resolved {
            color::border_subtle()
        } else {
            color::border()
        })
        .pl_2()
        .py_1()
        .text_xs()
        .text_color(if is_resolved {
            color::text_disabled()
        } else {
            color::text_secondary()
        })
        .child(label)
}

pub(super) fn render_review_marker(
    thread_count: usize,
    has_unresolved_thread: bool,
    width: f32,
) -> impl IntoElement {
    let marker = match thread_count {
        0 => String::new(),
        1 => "R".to_string(),
        count => format!("R{count}"),
    };
    let color = if has_unresolved_thread {
        color::warning()
    } else {
        color::text_muted()
    };

    div()
        .w(px(width))
        .flex_none()
        .text_center()
        .whitespace_nowrap()
        .overflow_hidden()
        .text_color(color)
        .child(marker)
}

fn render_review_menu_marker(width: f32) -> impl IntoElement {
    div()
        .w(px(width))
        .flex_none()
        .text_center()
        .whitespace_nowrap()
        .overflow_hidden()
        .text_color(color::accent())
        .child("")
}

#[cfg(test)]
#[path = "inline_reviews/tests.rs"]
mod tests;
