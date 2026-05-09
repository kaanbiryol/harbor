#![expect(
    clippy::too_many_arguments,
    reason = "inline review render helpers pass explicit interaction state for each virtualized row"
)]

#[path = "inline_review_avatars.rs"]
mod avatars;
#[path = "inline_review_comment_actions.rs"]
mod comment_actions;
#[path = "inline_review_comments.rs"]
mod comments;
#[path = "inline_review_reactions.rs"]
mod reactions;

use gpui::{Entity, IntoElement, div, prelude::*, px, rgb};
use gpui_component::{
    Disableable, IconName, Sizable, StyledExt,
    button::{Button, ButtonVariants},
    input::{Input, InputState},
};
use harbor_domain::{ReviewThread, ReviewThreadState};

use crate::{
    diff_reviews::review_thread_inline_rows,
    workspace::{
        AppView, PendingReviewSession, ReviewCommentSubmission, ReviewCommentUiError,
        ReviewComposer, ReviewReactionAction, ReviewThreadUiError,
    },
};

use super::{
    DIFF_ROW_HEIGHT, REVIEW_COMPOSER_MAX_WIDTH, REVIEW_MARKER_WIDTH,
    layout::review_comment_range_label, render_line_number,
};
use crate::panels::review::review_thread_state_label;
#[cfg(test)]
pub(crate) use avatars::{github_avatar_url_for_login, review_comment_avatar_url};
#[cfg(test)]
pub(crate) use comment_actions::review_comment_action_visibility;
use comments::render_review_comment_inline;
#[cfg(test)]
pub(crate) use reactions::{
    review_reaction_button_label, review_reaction_emoji, visible_review_reaction_contents,
};

pub(super) fn render_review_composer_inline(
    composer: ReviewComposer,
    pending_review: Option<PendingReviewSession>,
    review_comment_input: Entity<InputState>,
    body_empty: bool,
    is_submitting: bool,
    error: Option<&str>,
    row_count: usize,
    line_number_width: f32,
    review_marker_width: f32,
    view_entity: Entity<AppView>,
) -> impl IntoElement {
    let target_label = review_comment_range_label(&composer.range);
    let submit_disabled = body_empty || is_submitting;
    let has_pending_review = pending_review.is_some();
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
                                    Input::new(&review_comment_input)
                                        .w_full()
                                        .small()
                                        .h(px(DIFF_ROW_HEIGHT * 3.0))
                                        .appearance(false)
                                        .bordered(false)
                                        .focus_bordered(false),
                                ),
                        )
                        .when_some(error.map(ToString::to_string), |element, error| {
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
                                .when_some(pending_review, {
                                    let view_entity = view_entity.clone();
                                    move |element, _pending_review| {
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

pub(super) fn render_review_thread_inline(
    thread: &ReviewThread,
    line_number_width: f32,
    active_review_thread_reply: Option<&str>,
    review_thread_reply_input: Entity<InputState>,
    reply_body_empty: bool,
    is_submitting_reply: bool,
    reply_error: Option<&ReviewThreadUiError>,
    action_thread_id: Option<&str>,
    action_error: Option<&ReviewThreadUiError>,
    active_review_comment_edit: Option<&str>,
    review_comment_edit_input: Entity<InputState>,
    edit_body_empty: bool,
    is_submitting_edit: bool,
    edit_error: Option<&ReviewCommentUiError>,
    action_comment_id: Option<&str>,
    comment_action_error: Option<&ReviewCommentUiError>,
    reaction_action: Option<&ReviewReactionAction>,
    reaction_error: Option<&ReviewCommentUiError>,
    view_entity: Entity<AppView>,
) -> impl IntoElement {
    let (label, color) = review_thread_state_label(thread.state);
    let height = review_thread_inline_rows(thread) as f32 * DIFF_ROW_HEIGHT;
    let active_reply = active_review_thread_reply == Some(thread.id.as_str());
    let thread_action_running = action_thread_id == Some(thread.id.as_str());
    let thread_reply_submitting = active_reply && is_submitting_reply;
    let reply_disabled = reply_body_empty || thread_reply_submitting;
    let is_resolved = thread.state == ReviewThreadState::Resolved;
    let can_toggle_resolution = thread.state != ReviewThreadState::Outdated;
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
    let card_header_bg_color = if is_resolved {
        rgb(0x121a24)
    } else {
        rgb(0x151e29)
    };
    let card_header_border_color = if is_resolved {
        rgb(0x203040)
    } else {
        rgb(0x263241)
    };
    let comment_count_color = if is_resolved {
        rgb(0x56657a)
    } else {
        rgb(0x64748b)
    };
    let reply_error = reply_error
        .filter(|error| error.thread_id == thread.id)
        .map(|error| error.message.clone());
    let action_error = action_error
        .filter(|error| error.thread_id == thread.id)
        .map(|error| error.message.clone());
    let thread_id = thread.id.clone();
    let toggle_label = if is_resolved { "Reopen" } else { "Resolve" };

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
                        .child(
                            div()
                                .border_b_1()
                                .border_color(card_header_border_color)
                                .bg(card_header_bg_color)
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
                                        .child(render_review_thread_status_pill(label, color))
                                        .child(
                                            div().text_xs().text_color(comment_count_color).child(
                                                review_comment_count_label(thread.comments.len()),
                                            ),
                                        ),
                                )
                                .child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .gap_2()
                                        .child(
                                            Button::new(format!("reply-thread-{thread_id}"))
                                                .label(if active_reply {
                                                    "Replying"
                                                } else {
                                                    "Reply"
                                                })
                                                .xsmall()
                                                .outline()
                                                .disabled(is_submitting_reply)
                                                .on_click({
                                                    let view_entity = view_entity.clone();
                                                    let thread_id = thread_id.clone();
                                                    move |_, window, cx| {
                                                        view_entity.update(cx, |view, cx| {
                                                            view.open_review_thread_reply(
                                                                thread_id.clone(),
                                                                window,
                                                                cx,
                                                            );
                                                        });
                                                    }
                                                }),
                                        )
                                        .child(
                                            Button::new(format!("toggle-thread-{thread_id}"))
                                                .icon(if is_resolved {
                                                    IconName::Undo2
                                                } else {
                                                    IconName::CircleCheck
                                                })
                                                .label(toggle_label)
                                                .xsmall()
                                                .ghost()
                                                .loading(thread_action_running)
                                                .disabled(
                                                    !can_toggle_resolution || thread_action_running,
                                                )
                                                .on_click({
                                                    let view_entity = view_entity.clone();
                                                    let thread_id = thread_id.clone();
                                                    move |_, _, cx| {
                                                        view_entity.update(cx, |view, cx| {
                                                            view.set_review_thread_resolved(
                                                                thread_id.clone(),
                                                                !is_resolved,
                                                                cx,
                                                            );
                                                        });
                                                    }
                                                }),
                                        ),
                                ),
                        )
                        .child(div().px_2().pb_2().children(
                            thread.comments.iter().enumerate().map(|(index, comment)| {
                                render_review_comment_inline(
                                    comment,
                                    index > 0,
                                    active_review_comment_edit,
                                    review_comment_edit_input.clone(),
                                    edit_body_empty,
                                    is_submitting_edit,
                                    edit_error,
                                    action_comment_id,
                                    comment_action_error,
                                    reaction_action,
                                    reaction_error,
                                    is_resolved,
                                    view_entity.clone(),
                                )
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
                        .when(active_reply, {
                            let view_entity = view_entity.clone();
                            let thread_id = thread_id.clone();
                            move |element| {
                                element.child(render_review_thread_reply_composer(
                                    thread_id.clone(),
                                    review_thread_reply_input.clone(),
                                    reply_disabled,
                                    thread_reply_submitting,
                                    reply_error.clone(),
                                    view_entity.clone(),
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

fn render_review_thread_status_pill(label: &str, color: gpui::Hsla) -> impl IntoElement {
    div()
        .rounded_xs()
        .border_1()
        .border_color(rgb(0x334155))
        .bg(rgb(0x0f1720))
        .px_1()
        .py_0p5()
        .text_xs()
        .font_medium()
        .text_color(color)
        .child(label.to_string())
}

fn render_review_thread_reply_composer(
    thread_id: String,
    review_thread_reply_input: Entity<InputState>,
    reply_disabled: bool,
    thread_reply_submitting: bool,
    reply_error: Option<String>,
    view_entity: Entity<AppView>,
) -> impl IntoElement {
    div()
        .border_t_1()
        .border_color(rgb(0x263241))
        .bg(rgb(0x101720))
        .px_2()
        .py_2()
        .child(
            div()
                .w_full()
                .border_1()
                .border_color(rgb(0x354252))
                .bg(rgb(0x0b1118))
                .px_2()
                .py_1()
                .child(
                    Input::new(&review_thread_reply_input)
                        .w_full()
                        .small()
                        .h(px(DIFF_ROW_HEIGHT * 2.0))
                        .appearance(false)
                        .bordered(false)
                        .focus_bordered(false),
                ),
        )
        .when_some(reply_error, |element, error| {
            element.child(
                div()
                    .pt_1()
                    .text_xs()
                    .text_color(rgb(0xf87171))
                    .child(error),
            )
        })
        .child(
            div()
                .pt_1()
                .flex()
                .items_center()
                .justify_end()
                .gap_2()
                .child(
                    Button::new(format!("cancel-thread-reply-{thread_id}"))
                        .label("Cancel")
                        .xsmall()
                        .ghost()
                        .disabled(thread_reply_submitting)
                        .on_click({
                            let view_entity = view_entity.clone();
                            move |_, window, cx| {
                                view_entity.update(cx, |view, cx| {
                                    view.cancel_review_thread_reply(window, cx);
                                });
                            }
                        }),
                )
                .child(
                    Button::new(format!("submit-thread-reply-{thread_id}"))
                        .label("Send reply")
                        .xsmall()
                        .primary()
                        .loading(thread_reply_submitting)
                        .disabled(reply_disabled)
                        .on_click({
                            let view_entity = view_entity.clone();
                            let thread_id = thread_id.clone();
                            move |_, _, cx| {
                                view_entity.update(cx, |view, cx| {
                                    view.submit_review_thread_reply(thread_id.clone(), cx);
                                });
                            }
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

fn review_comment_count_label(comment_count: usize) -> String {
    if comment_count == 1 {
        "1 comment".to_string()
    } else {
        format!("{comment_count} comments")
    }
}
