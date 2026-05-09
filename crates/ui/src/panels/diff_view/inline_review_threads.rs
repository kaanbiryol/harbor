use gpui::{Entity, IntoElement, div, prelude::*, px, rgb};
use gpui_component::{
    Disableable, IconName, Sizable, StyledExt,
    button::{Button, ButtonVariants},
    input::{Input, InputState},
};
use harbor_domain::{ReviewThread, ReviewThreadState};

use crate::{panels::review::review_thread_state_label, workspace::AppView};

use super::super::DIFF_ROW_HEIGHT;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ReviewThreadUiState {
    pub(crate) active_reply: bool,
    pub(crate) action_running: bool,
    pub(crate) reply_submitting: bool,
    pub(crate) reply_disabled: bool,
    pub(crate) reply_button_disabled: bool,
    pub(crate) is_resolved: bool,
    pub(crate) can_toggle_resolution: bool,
}

pub(super) struct ReviewThreadHeaderState {
    pub(super) thread_id: String,
    pub(super) thread_state: ReviewThreadState,
    pub(super) comment_count: usize,
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

pub(crate) fn review_thread_ui_state(
    thread: &ReviewThread,
    active_reply_thread_id: Option<&str>,
    reply_body_empty: bool,
    is_submitting_reply: bool,
    action_thread_id: Option<&str>,
) -> ReviewThreadUiState {
    let active_reply = active_reply_thread_id == Some(thread.id.as_str());
    let reply_submitting = active_reply && is_submitting_reply;

    ReviewThreadUiState {
        active_reply,
        action_running: action_thread_id == Some(thread.id.as_str()),
        reply_submitting,
        reply_disabled: reply_body_empty || reply_submitting,
        reply_button_disabled: is_submitting_reply,
        is_resolved: thread.state == ReviewThreadState::Resolved,
        can_toggle_resolution: thread.state != ReviewThreadState::Outdated,
    }
}

pub(super) fn render_review_thread_header(state: ReviewThreadHeaderState) -> impl IntoElement {
    let ReviewThreadHeaderState {
        thread_id,
        thread_state,
        comment_count,
        active_reply,
        reply_button_disabled,
        action_running,
        can_toggle_resolution,
        view_entity,
    } = state;
    let (label, color) = review_thread_state_label(thread_state);
    let is_resolved = thread_state == ReviewThreadState::Resolved;
    let comment_count_color = if is_resolved {
        rgb(0x56657a)
    } else {
        rgb(0x64748b)
    };
    let toggle_label = if is_resolved { "Reopen" } else { "Resolve" };

    div()
        .border_b_1()
        .border_color(if is_resolved {
            rgb(0x203040)
        } else {
            rgb(0x263241)
        })
        .bg(if is_resolved {
            rgb(0x121a24)
        } else {
            rgb(0x151e29)
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
                .child(render_review_thread_status_pill(label, color))
                .child(
                    div()
                        .text_xs()
                        .text_color(comment_count_color)
                        .child(review_comment_count_label(comment_count)),
                ),
        )
        .child(
            div()
                .flex()
                .items_center()
                .gap_2()
                .child(
                    Button::new(format!("reply-thread-{thread_id}"))
                        .label(if active_reply { "Replying" } else { "Reply" })
                        .xsmall()
                        .outline()
                        .disabled(reply_button_disabled)
                        .on_click({
                            let view_entity = view_entity.clone();
                            let thread_id = thread_id.clone();
                            move |_, window, cx| {
                                view_entity.update(cx, |view, cx| {
                                    view.open_review_thread_reply(thread_id.clone(), window, cx);
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
                        .loading(action_running)
                        .disabled(!can_toggle_resolution || action_running)
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
                    Input::new(&input)
                        .w_full()
                        .small()
                        .h(px(DIFF_ROW_HEIGHT * 2.0))
                        .appearance(false)
                        .bordered(false)
                        .focus_bordered(false),
                ),
        )
        .when_some(error, |element, error| {
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
                        .disabled(submitting)
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
                        .loading(submitting)
                        .disabled(disabled)
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

pub(crate) fn review_comment_count_label(comment_count: usize) -> String {
    if comment_count == 1 {
        "1 comment".to_string()
    } else {
        format!("{comment_count} comments")
    }
}
