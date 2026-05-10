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

use crate::workspace::{AppView, ReviewCommentSubmission, ReviewComposer, ReviewThreadUiError};

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
    pub(super) anchor_label: Option<String>,
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

    div()
        .min_h(px(DIFF_ROW_HEIGHT))
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

pub(super) fn render_review_thread_inline(
    state: ReviewThreadRenderState<'_>,
) -> impl IntoElement + use<> {
    let ReviewThreadRenderState {
        thread,
        anchor_label,
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
    let hidden_comment_count = hidden_inline_review_comment_count(thread.comments.len());
    let visible_reply_start_index = visible_inline_review_reply_start_index(thread.comments.len());

    div()
        .min_h(px(DIFF_ROW_HEIGHT))
        .w_full()
        .flex()
        .items_start()
        .bg(rgb(0x0c0f12))
        .text_color(rgb(0xcbd5e1))
        .font_family(".SystemUIFont")
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
                            anchor_label,
                            comment_count: thread.comments.len(),
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
                                .children(thread.comments.iter().take(1).enumerate().map(
                                    |(index, comment)| {
                                        render_review_comment_inline(ReviewCommentRenderState::new(
                                            comment,
                                            index > 0,
                                            is_resolved,
                                            &comments,
                                        ))
                                    },
                                ))
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
                                            render_review_comment_inline(
                                                ReviewCommentRenderState::new(
                                                    comment,
                                                    index > 0,
                                                    is_resolved,
                                                    &comments,
                                                ),
                                            )
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
            rgb(0x213040)
        } else {
            rgb(0x263241)
        })
        .pl_2()
        .py_1()
        .text_xs()
        .text_color(if is_resolved {
            rgb(0x697789)
        } else {
            rgb(0x93a4b8)
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
        rgb(0xfbbf24)
    } else {
        rgb(0x64748b)
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
        .text_color(rgb(0x93c5fd))
        .child("")
}

#[cfg(test)]
mod tests {
    use gpui::{
        Context, Entity, IntoElement, Modifiers, Render, TestAppContext, VisualTestContext, Window,
    };
    use gpui_component::{Root, Theme, ThemeMode, input::InputState};

    use crate::test_fixtures::review_thread as test_review_thread;
    use crate::workspace::{
        AppView, ReviewCommentUiError, ReviewReactionAction, ReviewThreadUiError,
    };

    use super::*;

    #[test]
    fn compacts_large_inline_review_threads() {
        assert_eq!(hidden_inline_review_comment_count(21), 0);
        assert_eq!(visible_inline_review_reply_start_index(21), 1);
        assert_eq!(hidden_inline_review_comment_count(125), 104);
        assert_eq!(visible_inline_review_reply_start_index(125), 105);
    }

    #[gpui::test]
    async fn renders_comment_actions_only_when_available(cx: &mut TestAppContext) {
        let (_, harness_entity, cx) = init_visual_review_test(cx);

        render_inline_review_harness(cx);
        assert!(
            cx.debug_bounds("inline-review-comment-actions-comment-1")
                .is_none()
        );

        harness_entity.update(cx, |harness, cx| {
            harness.thread.comments[0].viewer_can_update = true;
            cx.notify();
        });
        render_inline_review_harness(cx);
        assert!(
            cx.debug_bounds("inline-review-comment-actions-comment-1")
                .is_some()
        );
    }

    #[gpui::test]
    async fn reply_button_opens_thread_reply_mode(cx: &mut TestAppContext) {
        let (view_entity, _, cx) = init_visual_review_test(cx);

        render_inline_review_harness(cx);
        let reply_bounds = cx
            .debug_bounds("inline-review-reply-thread-1")
            .expect("reply button should render");
        cx.simulate_click(reply_bounds.center(), Modifiers::none());

        assert_eq!(
            view_entity.read_with(cx, |view, _| view
                .review_composer_state
                .thread_reply_thread_id
                .clone()),
            Some("thread-1".to_string())
        );
    }

    #[gpui::test]
    async fn comment_edit_cancel_exits_edit_mode(cx: &mut TestAppContext) {
        let (view_entity, harness_entity, cx) = init_visual_review_test(cx);
        harness_entity.update(cx, |harness, cx| {
            harness.thread.comments[0].viewer_can_update = true;
            cx.notify();
        });

        cx.update(|window, app| {
            view_entity.update(app, |view, cx| {
                view.open_review_comment_edit(
                    "comment-1".to_string(),
                    "Please check this line.".to_string(),
                    window,
                    cx,
                );
            });
        });
        render_inline_review_harness(cx);
        let cancel_bounds = cx
            .debug_bounds("inline-review-comment-edit-cancel-comment-1")
            .expect("edit cancel button should render");
        cx.simulate_click(cancel_bounds.center(), Modifiers::none());

        assert!(view_entity.read_with(cx, |view, _| {
            view.review_composer_state.comment_edit_comment_id.is_none()
        }));
    }

    fn init_visual_review_test(
        cx: &mut TestAppContext,
    ) -> (
        Entity<AppView>,
        Entity<InlineReviewThreadHarness>,
        &mut VisualTestContext,
    ) {
        cx.update(|cx| {
            gpui_component::init(cx);
            Theme::change(ThemeMode::Dark, None, cx);
        });

        let mut view_entity = None;
        let mut harness_entity = None;
        let (_, cx) = cx.add_window_view(|window, cx| {
            let view = cx.new(|cx| AppView::new_without_startup_tasks(window, cx));
            let harness = cx.new(|_| InlineReviewThreadHarness {
                view_entity: view.clone(),
                thread: review_thread(),
            });
            view_entity = Some(view.clone());
            harness_entity = Some(harness.clone());
            Root::new(harness, window, cx)
        });

        (
            view_entity.expect("test AppView should be created"),
            harness_entity.expect("test inline review harness should be created"),
            cx,
        )
    }

    fn render_inline_review_harness(cx: &mut VisualTestContext) {
        cx.refresh().expect("test window should refresh");
        cx.run_until_parked();
    }

    struct InlineReviewThreadHarness {
        view_entity: Entity<AppView>,
        thread: ReviewThread,
    }

    impl Render for InlineReviewThreadHarness {
        fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            let render_state = self
                .view_entity
                .read_with(cx, |view, app| ReviewThreadTestState {
                    active_reply_thread_id: view
                        .review_composer_state
                        .thread_reply_thread_id
                        .clone(),
                    reply_input: view.review_composer_state.thread_reply_input.clone(),
                    reply_body_empty: view
                        .review_composer_state
                        .thread_reply_input
                        .read(app)
                        .value()
                        .trim()
                        .is_empty(),
                    is_submitting_reply: view.is_submitting_review_thread_reply,
                    review_thread_reply_error: view.review_thread_reply_error.as_ref().cloned(),
                    action_thread_id: view.review_thread_action_thread_id.clone(),
                    action_error: view.review_thread_action_error.as_ref().cloned(),
                    active_comment_edit_id: view
                        .review_composer_state
                        .comment_edit_comment_id
                        .clone(),
                    comment_edit_input: view.review_composer_state.comment_edit_input.clone(),
                    edit_body_empty: view
                        .review_composer_state
                        .comment_edit_input
                        .read(app)
                        .value()
                        .trim()
                        .is_empty(),
                    is_submitting_edit: view.is_submitting_review_comment_edit,
                    review_comment_edit_error: view.review_comment_edit_error.as_ref().cloned(),
                    action_comment_id: view.review_comment_action_comment_id.clone(),
                    comment_action_error: view.review_comment_action_error.as_ref().cloned(),
                    reaction_action: view.review_reaction_action.clone(),
                    reaction_error: view.review_reaction_error.as_ref().cloned(),
                });
            let active_reply_thread_id = render_state.active_reply_thread_id.as_deref();
            let action_thread_id = render_state.action_thread_id.as_deref();
            let active_comment_edit_id = render_state.active_comment_edit_id.as_deref();
            let action_comment_id = render_state.action_comment_id.as_deref();
            let comments = ReviewCommentListRenderState {
                active_review_comment_edit: active_comment_edit_id,
                review_comment_edit_input: render_state.comment_edit_input.clone(),
                edit_body_empty: render_state.edit_body_empty,
                is_submitting_edit: render_state.is_submitting_edit,
                edit_error: render_state.review_comment_edit_error.as_ref(),
                action_comment_id,
                comment_action_error: render_state.comment_action_error.as_ref(),
                reaction_action: render_state.reaction_action.as_ref(),
                reaction_error: render_state.reaction_error.as_ref(),
                view_entity: self.view_entity.clone(),
            };

            render_review_thread_inline(ReviewThreadRenderState {
                thread: &self.thread,
                anchor_label: Some("new line 12 in src/lib.rs".to_string()),
                line_number_width: 44.0,
                active_review_thread_reply: active_reply_thread_id,
                review_thread_reply_input: render_state.reply_input.clone(),
                reply_body_empty: render_state.reply_body_empty,
                is_submitting_reply: render_state.is_submitting_reply,
                reply_error: render_state.review_thread_reply_error.as_ref(),
                action_thread_id,
                action_error: render_state.action_error.as_ref(),
                comments: comments.clone(),
                view_entity: self.view_entity.clone(),
            })
            .into_element()
        }
    }

    struct ReviewThreadTestState {
        active_reply_thread_id: Option<String>,
        reply_input: Entity<InputState>,
        reply_body_empty: bool,
        is_submitting_reply: bool,
        review_thread_reply_error: Option<ReviewThreadUiError>,
        action_thread_id: Option<String>,
        action_error: Option<ReviewThreadUiError>,
        active_comment_edit_id: Option<String>,
        comment_edit_input: Entity<InputState>,
        edit_body_empty: bool,
        is_submitting_edit: bool,
        review_comment_edit_error: Option<ReviewCommentUiError>,
        action_comment_id: Option<String>,
        comment_action_error: Option<ReviewCommentUiError>,
        reaction_action: Option<ReviewReactionAction>,
        reaction_error: Option<ReviewCommentUiError>,
    }

    fn review_thread() -> ReviewThread {
        let mut thread = test_review_thread(ReviewThreadState::Unresolved);
        thread.comments[0].viewer_can_update = false;
        thread
    }
}
