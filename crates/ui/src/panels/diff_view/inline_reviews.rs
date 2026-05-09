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

use gpui::{Entity, IntoElement, div, prelude::*, px, rgb};
use gpui_component::{
    Disableable, Sizable, StyledExt,
    button::{Button, ButtonVariants},
    input::{Input, InputState},
};
use harbor_domain::{ReviewThread, ReviewThreadState};

use crate::{
    diff_reviews::review_thread_inline_rows,
    workspace::{AppView, ReviewCommentSubmission, ReviewComposer, ReviewThreadUiError},
};

use super::{
    DIFF_ROW_HEIGHT, REVIEW_COMPOSER_MAX_WIDTH, REVIEW_MARKER_WIDTH,
    layout::review_comment_range_label, render_line_number,
};
#[cfg(test)]
pub(crate) use avatars::{github_avatar_url_for_login, review_comment_avatar_url};
#[cfg(test)]
pub(crate) use comment_actions::review_comment_action_visibility;
pub(super) use comments::ReviewCommentListRenderState;
use comments::{ReviewCommentRenderState, render_review_comment_inline};
#[cfg(test)]
pub(crate) use comments::{review_comment_body_markdown, review_comment_ui_state};
#[cfg(test)]
pub(crate) use reactions::{
    review_reaction_button_label, review_reaction_emoji, visible_review_reaction_contents,
};
#[cfg(test)]
pub(crate) use threads::review_thread_ui_state;
use threads::{
    ReviewThreadHeaderState, ReviewThreadReplyComposerState, render_review_thread_header,
    render_review_thread_reply_composer,
};

pub(super) struct ReviewComposerRenderState {
    pub(super) composer: ReviewComposer,
    pub(super) has_pending_review: bool,
    pub(super) input: Entity<InputState>,
    pub(super) body_empty: bool,
    pub(super) is_submitting: bool,
    pub(super) error: Option<String>,
    pub(super) row_count: usize,
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
        row_count,
        line_number_width,
        review_marker_width,
        view_entity,
    } = state;
    let target_label = review_comment_range_label(&composer.range);
    let submit_disabled = body_empty || is_submitting;
    let height = row_count as f32 * DIFF_ROW_HEIGHT;

    div()
        .h(px(height))
        .w_full()
        .flex()
        .items_start()
        .bg(rgb(0x0c0f12))
        .text_color(rgb(0xcbd5e1))
        .font_family(".SystemUIFont")
        .child(render_line_number(None, line_number_width))
        .child(render_line_number(None, line_number_width))
        .child(render_review_menu_marker(review_marker_width))
        .child(
            div()
                .min_w_0()
                .flex_1()
                .flex()
                .flex_col()
                .gap_2()
                .py_1()
                .pr_3()
                .child(
                    div()
                        .w_full()
                        .max_w(px(REVIEW_COMPOSER_MAX_WIDTH))
                        .border_1()
                        .border_color(rgb(0x2c3745))
                        .bg(rgb(0x121923))
                        .px_3()
                        .py_2()
                        .child(
                            div()
                                .pb_2()
                                .text_xs()
                                .font_medium()
                                .text_color(rgb(0x9fc7ff))
                                .child(format!("Comment on {target_label}")),
                        )
                        .child(
                            div()
                                .w_full()
                                .border_1()
                                .border_color(rgb(0x354252))
                                .bg(rgb(0x0b1118))
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
                                    .text_color(rgb(0xf87171))
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
                ),
        )
}

pub(super) fn render_review_composer_spacer() -> impl IntoElement {
    div().h(px(DIFF_ROW_HEIGHT)).w_full()
}

pub(super) fn render_review_thread_inline(state: ReviewThreadRenderState<'_>) -> impl IntoElement {
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
    let height = review_thread_inline_rows(thread) as f32 * DIFF_ROW_HEIGHT;
    let is_resolved = ui_state.is_resolved;
    let card_border_color = if is_resolved {
        rgb(0x223142)
    } else {
        rgb(0x2c3745)
    };
    let card_bg_color = if is_resolved {
        rgb(0x0f151d)
    } else {
        rgb(0x121923)
    };
    let reply_error = reply_error
        .filter(|error| error.thread_id == thread.id)
        .map(|error| error.message.clone());
    let action_error = action_error
        .filter(|error| error.thread_id == thread.id)
        .map(|error| error.message.clone());
    let thread_id = thread.id.clone();

    div()
        .h(px(height))
        .w_full()
        .flex()
        .items_start()
        .bg(rgb(0x0c0f12))
        .text_color(rgb(0xcbd5e1))
        .font_family(".SystemUIFont")
        .whitespace_nowrap()
        .child(render_line_number(None, line_number_width))
        .child(render_line_number(None, line_number_width))
        .child(render_review_marker(
            1,
            thread.state == ReviewThreadState::Unresolved,
            REVIEW_MARKER_WIDTH,
        ))
        .child(
            div()
                .min_w_0()
                .flex_1()
                .flex()
                .flex_col()
                .gap_2()
                .py_1()
                .pr_3()
                .child(
                    div()
                        .w_full()
                        .border_1()
                        .border_color(card_border_color)
                        .bg(card_bg_color)
                        .rounded_xs()
                        .overflow_hidden()
                        .child(render_review_thread_header(ReviewThreadHeaderState {
                            thread_id: thread_id.clone(),
                            thread_state: thread.state,
                            comment_count: thread.comments.len(),
                            active_reply: ui_state.active_reply,
                            reply_button_disabled: ui_state.reply_button_disabled,
                            action_running: ui_state.action_running,
                            can_toggle_resolution: ui_state.can_toggle_resolution,
                            view_entity: view_entity.clone(),
                        }))
                        .child(div().px_2().pb_2().children(
                            thread.comments.iter().enumerate().map(|(index, comment)| {
                                render_review_comment_inline(ReviewCommentRenderState::new(
                                    comment,
                                    index > 0,
                                    is_resolved,
                                    &comments,
                                ))
                            }),
                        ))
                        .when(thread.comments.is_empty(), |element| {
                            element.child(
                                div()
                                    .px_2()
                                    .pb_2()
                                    .text_xs()
                                    .text_color(if is_resolved {
                                        rgb(0x697789)
                                    } else {
                                        rgb(0x9aa4b2)
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
                                    .text_color(rgb(0xf87171))
                                    .child(error),
                            )
                        }),
                ),
        )
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
        rgb(0xfbbf24)
    } else {
        rgb(0x64748b)
    };

    div()
        .w(px(width))
        .flex_none()
        .text_center()
        .text_color(color)
        .child(marker)
}

fn render_review_menu_marker(width: f32) -> impl IntoElement {
    div()
        .w(px(width))
        .flex_none()
        .text_center()
        .text_color(rgb(0x93c5fd))
        .child("")
}
