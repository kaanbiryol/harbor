#![expect(
    clippy::too_many_arguments,
    reason = "inline review render helpers pass explicit interaction state for each virtualized row"
)]

use gpui::{Anchor, AnyElement, Entity, IntoElement, div, img, prelude::*, px, rgb};
use gpui_component::{
    Disableable, IconName, Sizable, StyledExt,
    button::{Button, ButtonVariants},
    input::{Input, InputState},
    popover::{Popover, PopoverState},
};
use harbor_domain::{ReactionContent, ReviewComment, ReviewThread, ReviewThreadState};

use crate::{
    diff_reviews::review_thread_inline_rows,
    workspace::{
        AppView, PendingReviewSession, ReviewCommentSubmission, ReviewCommentUiError,
        ReviewComposer, ReviewReactionAction, ReviewThreadUiError, review_comment_pending_sync,
        review_reaction,
    },
};

use super::{
    DIFF_ROW_HEIGHT, REVIEW_COMPOSER_MAX_WIDTH, REVIEW_MARKER_WIDTH,
    layout::review_comment_range_label, render_line_number,
};
use crate::panels::review::review_thread_state_label;

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

fn render_review_comment_inline(
    comment: &ReviewComment,
    separated: bool,
    active_review_comment_edit: Option<&str>,
    review_comment_edit_input: Entity<InputState>,
    edit_body_empty: bool,
    is_submitting_edit: bool,
    edit_error: Option<&ReviewCommentUiError>,
    action_comment_id: Option<&str>,
    comment_action_error: Option<&ReviewCommentUiError>,
    reaction_action: Option<&ReviewReactionAction>,
    reaction_error: Option<&ReviewCommentUiError>,
    thread_resolved: bool,
    view_entity: Entity<AppView>,
) -> AnyElement {
    let comment_id = comment.id.clone();
    let comment_body = comment.body.clone();
    let active_edit = active_review_comment_edit == Some(comment.id.as_str());
    let edit_submitting = active_edit && is_submitting_edit;
    let action_running = action_comment_id == Some(comment.id.as_str());
    let edit_error = edit_error
        .filter(|error| error.comment_id == comment.id)
        .map(|error| error.message.clone());
    let action_error = comment_action_error
        .filter(|error| error.comment_id == comment.id)
        .map(|error| error.message.clone());
    let reaction_error = reaction_error
        .filter(|error| error.comment_id == comment.id)
        .map(|error| error.message.clone());
    let (can_update, can_delete) = review_comment_action_visibility(comment);
    let author_color = if thread_resolved {
        rgb(0xb7c0cd)
    } else {
        rgb(0xe5edf7)
    };
    let metadata_color = if thread_resolved {
        rgb(0x526176)
    } else {
        rgb(0x64748b)
    };
    let body_color = if thread_resolved {
        rgb(0x8996a8)
    } else {
        rgb(0xcbd5e1)
    };
    let separator_color = if thread_resolved {
        rgb(0x213040)
    } else {
        rgb(0x263241)
    };

    div()
        .pt_2()
        .when(separated, |element| {
            element.mt_2().border_t_1().border_color(separator_color)
        })
        .flex()
        .items_start()
        .gap_2()
        .child(render_review_comment_avatar(comment))
        .child(
            div()
                .min_w_0()
                .flex_1()
                .child(
                    div()
                        .flex()
                        .items_center()
                        .justify_between()
                        .gap_2()
                        .child(
                            div()
                                .min_w_0()
                                .flex()
                                .items_center()
                                .gap_2()
                                .text_xs()
                                .child(
                                    div()
                                        .font_medium()
                                        .text_color(author_color)
                                        .child(comment.author.clone()),
                                )
                                .child(
                                    div()
                                        .text_color(metadata_color)
                                        .child(review_comment_time_label(comment)),
                                )
                                .when(review_comment_pending_sync(comment), |element| {
                                    element.child(
                                        div()
                                            .rounded_xs()
                                            .border_1()
                                            .border_color(rgb(0x355071))
                                            .bg(rgb(0x101b2a))
                                            .px_1()
                                            .text_color(rgb(0x93c5fd))
                                            .child("syncing"),
                                    )
                                }),
                        )
                        .when(can_update || can_delete, {
                            let view_entity = view_entity.clone();
                            let comment_id = comment_id.clone();
                            let comment_body = comment_body.clone();
                            move |element| {
                                element.child(render_review_comment_actions_menu(
                                    comment_id.clone(),
                                    comment_body.clone(),
                                    can_update,
                                    can_delete,
                                    active_edit,
                                    edit_submitting,
                                    action_running,
                                    view_entity.clone(),
                                ))
                            }
                        }),
                )
                .when(!active_edit, |element| {
                    element.child(render_review_comment_body(&comment.body, body_color))
                })
                .when(active_edit, {
                    let view_entity = view_entity.clone();
                    let comment_id = comment_id.clone();
                    move |element| {
                        element.child(render_review_comment_edit_composer(
                            comment_id.clone(),
                            review_comment_edit_input.clone(),
                            edit_body_empty,
                            edit_submitting,
                            edit_error.clone(),
                            view_entity.clone(),
                        ))
                    }
                })
                .child(render_review_reactions(
                    comment,
                    reaction_action,
                    view_entity.clone(),
                ))
                .when_some(action_error, |element, error| {
                    element.child(
                        div()
                            .pt_1()
                            .text_xs()
                            .text_color(rgb(0xf87171))
                            .child(error),
                    )
                })
                .when_some(reaction_error, |element, error| {
                    element.child(
                        div()
                            .pt_1()
                            .text_xs()
                            .text_color(rgb(0xf87171))
                            .child(error),
                    )
                }),
        )
        .into_any_element()
}

fn render_review_comment_avatar(comment: &ReviewComment) -> impl IntoElement {
    let initial = author_initial(&comment.author);
    let avatar = div()
        .mt(px(1.0))
        .w(px(20.0))
        .h(px(20.0))
        .flex_none()
        .rounded_xs()
        .border_1()
        .border_color(rgb(0x334155))
        .bg(rgb(0x1d2734))
        .flex()
        .items_center()
        .justify_center()
        .text_xs()
        .font_medium()
        .text_color(rgb(0xcbd5e1));

    if let Some(avatar_url) = review_comment_avatar_url(comment) {
        let loading_initial = initial.clone();
        let fallback_initial = initial.clone();
        avatar
            .overflow_hidden()
            .child(
                img(avatar_url)
                    .w(px(20.0))
                    .h(px(20.0))
                    .with_loading(move || render_review_comment_avatar_initial(&loading_initial))
                    .with_fallback(move || render_review_comment_avatar_initial(&fallback_initial)),
            )
            .into_any_element()
    } else {
        avatar.child(initial).into_any_element()
    }
}

fn render_review_comment_avatar_initial(initial: &str) -> AnyElement {
    div()
        .w(px(20.0))
        .h(px(20.0))
        .flex()
        .items_center()
        .justify_center()
        .text_xs()
        .font_medium()
        .text_color(rgb(0xcbd5e1))
        .child(initial.to_string())
        .into_any_element()
}

pub(crate) fn review_comment_avatar_url(comment: &ReviewComment) -> Option<String> {
    comment
        .author_avatar_url
        .clone()
        .or_else(|| github_avatar_url_for_login(&comment.author))
}

pub(crate) fn github_avatar_url_for_login(login: &str) -> Option<String> {
    let login = login.trim();

    if login.is_empty()
        || login.eq_ignore_ascii_case("ghost")
        || login.eq_ignore_ascii_case("you")
        || login.chars().any(char::is_whitespace)
    {
        None
    } else {
        Some(format!("https://github.com/{login}.png?size=48"))
    }
}

fn render_review_comment_actions_menu(
    comment_id: String,
    comment_body: String,
    can_update: bool,
    can_delete: bool,
    active_edit: bool,
    edit_submitting: bool,
    action_running: bool,
    view_entity: Entity<AppView>,
) -> impl IntoElement {
    Popover::new(format!("comment-actions-{comment_id}"))
        .appearance(false)
        .anchor(Anchor::TopRight)
        .trigger(
            Button::new(format!("comment-actions-trigger-{comment_id}"))
                .icon(IconName::Ellipsis)
                .xsmall()
                .compact()
                .ghost()
                .tooltip("Comment actions"),
        )
        .content(move |_, _window, _popover_cx| {
            div()
                .w(px(160.0))
                .border_1()
                .border_color(rgb(0x343b44))
                .bg(rgb(0x171b20))
                .p_1()
                .shadow_lg()
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_1()
                        .when(can_update, {
                            let view_entity = view_entity.clone();
                            let comment_id = comment_id.clone();
                            let comment_body = comment_body.clone();
                            move |element| {
                                element.child(
                                    Button::new(format!("edit-comment-{comment_id}"))
                                        .icon(IconName::ALargeSmall)
                                        .label(if active_edit { "Editing" } else { "Edit" })
                                        .small()
                                        .ghost()
                                        .disabled(edit_submitting || action_running)
                                        .on_click({
                                            let view_entity = view_entity.clone();
                                            let comment_id = comment_id.clone();
                                            let comment_body = comment_body.clone();
                                            move |_, window, cx| {
                                                view_entity.update(cx, |view, cx| {
                                                    view.open_review_comment_edit(
                                                        comment_id.clone(),
                                                        comment_body.clone(),
                                                        window,
                                                        cx,
                                                    );
                                                });
                                            }
                                        }),
                                )
                            }
                        })
                        .when(can_delete, {
                            let view_entity = view_entity.clone();
                            let comment_id = comment_id.clone();
                            move |element| {
                                element.child(
                                    Button::new(format!("delete-comment-{comment_id}"))
                                        .icon(IconName::Delete)
                                        .label("Delete")
                                        .small()
                                        .ghost()
                                        .loading(action_running)
                                        .disabled(action_running || edit_submitting)
                                        .on_click({
                                            let view_entity = view_entity.clone();
                                            let comment_id = comment_id.clone();
                                            move |_, _, cx| {
                                                view_entity.update(cx, |view, cx| {
                                                    view.delete_review_comment(
                                                        comment_id.clone(),
                                                        cx,
                                                    );
                                                });
                                            }
                                        }),
                                )
                            }
                        }),
                )
        })
}

fn render_review_comment_edit_composer(
    comment_id: String,
    review_comment_edit_input: Entity<InputState>,
    edit_body_empty: bool,
    edit_submitting: bool,
    edit_error: Option<String>,
    view_entity: Entity<AppView>,
) -> impl IntoElement {
    div()
        .child(
            div()
                .mt_2()
                .w_full()
                .border_1()
                .border_color(rgb(0x354252))
                .bg(rgb(0x0b1118))
                .px_2()
                .py_1()
                .child(
                    Input::new(&review_comment_edit_input)
                        .w_full()
                        .small()
                        .appearance(false)
                        .bordered(false)
                        .focus_bordered(false),
                ),
        )
        .when_some(edit_error, |element, error| {
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
                    Button::new(format!("cancel-comment-edit-{comment_id}"))
                        .label("Cancel")
                        .xsmall()
                        .ghost()
                        .disabled(edit_submitting)
                        .on_click({
                            let view_entity = view_entity.clone();
                            move |_, window, cx| {
                                view_entity.update(cx, |view, cx| {
                                    view.cancel_review_comment_edit(window, cx);
                                });
                            }
                        }),
                )
                .child(
                    Button::new(format!("save-comment-edit-{comment_id}"))
                        .label("Save")
                        .xsmall()
                        .primary()
                        .loading(edit_submitting)
                        .disabled(edit_body_empty || edit_submitting)
                        .on_click({
                            let view_entity = view_entity.clone();
                            let comment_id = comment_id.clone();
                            move |_, _, cx| {
                                view_entity.update(cx, |view, cx| {
                                    view.submit_review_comment_edit(comment_id.clone(), cx);
                                });
                            }
                        }),
                ),
        )
}

fn author_initial(author: &str) -> String {
    author
        .chars()
        .find(|character| character.is_alphanumeric())
        .map(|character| character.to_uppercase().to_string())
        .unwrap_or_else(|| "?".to_string())
}

fn render_review_comment_body(body: &str, color: gpui::Rgba) -> impl IntoElement {
    let lines: Vec<String> = body.lines().map(str::to_string).collect::<Vec<_>>();
    let lines = if lines.is_empty() {
        vec!["empty comment".to_string()]
    } else {
        lines
    };

    div()
        .pt_2()
        .text_xs()
        .text_color(color)
        .children(lines.into_iter().map(|line| {
            div().min_h(px(16.0)).child(if line.is_empty() {
                " ".to_string()
            } else {
                line
            })
        }))
}

fn render_review_reactions(
    comment: &ReviewComment,
    reaction_action: Option<&ReviewReactionAction>,
    view_entity: Entity<AppView>,
) -> impl IntoElement {
    let visible_reactions = visible_review_reaction_contents(comment);
    let has_visible_reactions = !visible_reactions.is_empty();
    let can_add_reaction = comment.viewer_can_react;

    div().when(has_visible_reactions || can_add_reaction, |element| {
        element
            .pt_2()
            .flex()
            .items_center()
            .gap_1()
            .children(visible_reactions.into_iter().map(|content| {
                render_review_reaction_button(
                    comment,
                    content,
                    reaction_action,
                    view_entity.clone(),
                )
            }))
            .when(can_add_reaction, |element| {
                element.child(render_add_reaction_popover(comment, view_entity.clone()))
            })
    })
}

fn render_review_reaction_button(
    comment: &ReviewComment,
    content: ReactionContent,
    reaction_action: Option<&ReviewReactionAction>,
    view_entity: Entity<AppView>,
) -> AnyElement {
    let reaction = review_reaction(comment, content);
    let count = reaction.map_or(0, |reaction| reaction.count);
    let viewer_has_reacted = reaction.is_some_and(|reaction| reaction.viewer_has_reacted);
    let running = reaction_action
        .is_some_and(|action| action.comment_id == comment.id && action.content == content);
    let comment_id = comment.id.clone();
    let label = review_reaction_button_label(content, count);
    let button = Button::new(format!("reaction-{comment_id}-{}", content.label()))
        .label(label)
        .xsmall()
        .disabled(!comment.viewer_can_react || running)
        .on_click({
            let view_entity = view_entity.clone();
            let comment_id = comment_id.clone();
            move |_, _, cx| {
                view_entity.update(cx, |view, cx| {
                    view.toggle_review_comment_reaction(comment_id.clone(), content, cx);
                });
            }
        });

    if viewer_has_reacted {
        button.primary().into_any_element()
    } else {
        button.ghost().into_any_element()
    }
}

fn render_add_reaction_popover(
    comment: &ReviewComment,
    view_entity: Entity<AppView>,
) -> impl IntoElement {
    let comment_id = comment.id.clone();

    Popover::new(format!("add-reaction-{comment_id}"))
        .appearance(false)
        .anchor(Anchor::TopRight)
        .trigger(
            Button::new(format!("add-reaction-trigger-{comment_id}"))
                .icon(IconName::Plus)
                .xsmall()
                .compact()
                .ghost()
                .tooltip("Add reaction"),
        )
        .content({
            let view_entity = view_entity.clone();
            move |_, _window, popover_cx| {
                let popover = popover_cx.entity().clone();
                let (comment, reaction_action) = {
                    let view = view_entity.read(popover_cx);
                    (
                        view.review_comment(&comment_id).cloned(),
                        view.review_reaction_action.clone(),
                    )
                };
                let Some(comment) = comment else {
                    return div()
                        .w(px(256.0))
                        .border_1()
                        .border_color(rgb(0x343b44))
                        .bg(rgb(0x171b20))
                        .p_2()
                        .text_xs()
                        .text_color(rgb(0x9aa4b2))
                        .child("Comment is no longer loaded")
                        .into_any_element();
                };

                div()
                    .w(px(256.0))
                    .border_1()
                    .border_color(rgb(0x343b44))
                    .bg(rgb(0x171b20))
                    .p_2()
                    .shadow_lg()
                    .child(div().grid().grid_cols(4).gap_1().children(
                        ReactionContent::ALL.into_iter().map(|content| {
                            render_review_reaction_picker_button(
                                &comment,
                                content,
                                reaction_action.as_ref(),
                                popover.clone(),
                                view_entity.clone(),
                            )
                        }),
                    ))
                    .into_any_element()
            }
        })
}

fn render_review_reaction_picker_button(
    comment: &ReviewComment,
    content: ReactionContent,
    reaction_action: Option<&ReviewReactionAction>,
    popover: Entity<PopoverState>,
    view_entity: Entity<AppView>,
) -> AnyElement {
    let reaction = review_reaction(comment, content);
    let viewer_has_reacted = reaction.is_some_and(|reaction| reaction.viewer_has_reacted);
    let running = reaction_action
        .is_some_and(|action| action.comment_id == comment.id && action.content == content);
    let comment_id = comment.id.clone();
    let button = Button::new(format!("reaction-picker-{comment_id}-{}", content.label()))
        .label(review_reaction_emoji(content))
        .xsmall()
        .disabled(!comment.viewer_can_react || running)
        .on_click({
            let view_entity = view_entity.clone();
            let comment_id = comment_id.clone();
            let popover = popover.clone();
            move |_, window, cx| {
                view_entity.update(cx, |view, cx| {
                    view.toggle_review_comment_reaction(comment_id.clone(), content, cx);
                });
                popover.update(cx, |popover, cx| {
                    popover.dismiss(window, cx);
                });
            }
        });

    if viewer_has_reacted {
        button.primary().into_any_element()
    } else {
        button.ghost().into_any_element()
    }
}

pub(crate) fn review_comment_action_visibility(comment: &ReviewComment) -> (bool, bool) {
    (comment.viewer_can_update, comment.viewer_can_delete)
}

pub(crate) fn visible_review_reaction_contents(comment: &ReviewComment) -> Vec<ReactionContent> {
    ReactionContent::ALL
        .into_iter()
        .filter(|content| {
            review_reaction(comment, *content)
                .is_some_and(|reaction| reaction.count > 0 || reaction.viewer_has_reacted)
        })
        .collect()
}

pub(crate) fn review_reaction_button_label(content: ReactionContent, count: usize) -> String {
    if count == 0 {
        review_reaction_emoji(content).to_string()
    } else {
        format!("{} {count}", review_reaction_emoji(content))
    }
}

pub(crate) fn review_reaction_emoji(content: ReactionContent) -> &'static str {
    match content {
        ReactionContent::ThumbsUp => "👍",
        ReactionContent::ThumbsDown => "👎",
        ReactionContent::Laugh => "😄",
        ReactionContent::Confused => "😕",
        ReactionContent::Heart => "❤️",
        ReactionContent::Hooray => "🎉",
        ReactionContent::Rocket => "🚀",
        ReactionContent::Eyes => "👀",
    }
}

fn review_comment_time_label(comment: &ReviewComment) -> String {
    let mut label = comment.created_at.format("%Y-%m-%d %H:%M").to_string();

    if comment
        .updated_at
        .is_some_and(|updated_at| updated_at != comment.created_at)
    {
        label.push_str(" edited");
    }

    label
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
