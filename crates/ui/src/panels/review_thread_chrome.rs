use gpui::{Entity, IntoElement, Pixels, div, prelude::*};
use gpui_component::{
    Disableable, IconName, Sizable, StyledExt,
    button::{Button, ButtonVariants},
    input::{Input, InputState},
};
use harbor_domain::{ReviewThread, ReviewThreadState};

use crate::{
    visual::{Tone, color, tone_colors},
    workspace::AppView,
};

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

#[derive(Clone)]
pub(crate) struct ReviewThreadActionIds {
    pub(crate) reply_button: String,
    pub(crate) reply_debug_selector: String,
    pub(crate) toggle_button: String,
    pub(crate) toggle_debug_selector: String,
}

#[derive(Clone)]
pub(crate) struct ReviewThreadReplyComposerIds {
    pub(crate) cancel_button: String,
    pub(crate) cancel_debug_selector: String,
    pub(crate) submit_button: String,
    pub(crate) submit_debug_selector: String,
}

#[derive(Clone, Copy)]
pub(crate) enum ReviewThreadReplyComposerChrome {
    Inline,
    Panel,
}

pub(crate) struct ReviewThreadActionsState {
    pub(crate) ids: ReviewThreadActionIds,
    pub(crate) thread_id: String,
    pub(crate) active_reply: bool,
    pub(crate) reply_button_disabled: bool,
    pub(crate) is_resolved: bool,
    pub(crate) action_running: bool,
    pub(crate) can_toggle_resolution: bool,
    pub(crate) show_toggle_icon: bool,
    pub(crate) view_entity: Entity<AppView>,
}

pub(crate) struct ReviewThreadReplyComposerState {
    pub(crate) ids: ReviewThreadReplyComposerIds,
    pub(crate) thread_id: String,
    pub(crate) input: Entity<InputState>,
    pub(crate) input_height: Pixels,
    pub(crate) disabled: bool,
    pub(crate) submitting: bool,
    pub(crate) error: Option<String>,
    pub(crate) chrome: ReviewThreadReplyComposerChrome,
    pub(crate) view_entity: Entity<AppView>,
}

impl ReviewThreadActionIds {
    pub(crate) fn inline(thread_id: &str) -> Self {
        Self {
            reply_button: format!("reply-thread-{thread_id}"),
            reply_debug_selector: format!("inline-review-reply-{thread_id}"),
            toggle_button: format!("toggle-thread-{thread_id}"),
            toggle_debug_selector: format!("inline-review-toggle-{thread_id}"),
        }
    }

    pub(crate) fn review_panel(thread_id: &str) -> Self {
        Self {
            reply_button: format!("review-panel-reply-thread-{thread_id}"),
            reply_debug_selector: format!("review-panel-reply-thread-{thread_id}"),
            toggle_button: format!("review-panel-toggle-thread-{thread_id}"),
            toggle_debug_selector: format!("review-panel-toggle-thread-{thread_id}"),
        }
    }
}

impl ReviewThreadReplyComposerIds {
    pub(crate) fn inline(thread_id: &str) -> Self {
        Self {
            cancel_button: format!("cancel-thread-reply-{thread_id}"),
            cancel_debug_selector: format!("inline-review-reply-cancel-{thread_id}"),
            submit_button: format!("submit-thread-reply-{thread_id}"),
            submit_debug_selector: format!("inline-review-reply-submit-{thread_id}"),
        }
    }

    pub(crate) fn review_panel(thread_id: &str) -> Self {
        Self {
            cancel_button: format!("review-panel-cancel-thread-reply-{thread_id}"),
            cancel_debug_selector: format!("review-panel-cancel-thread-reply-{thread_id}"),
            submit_button: format!("review-panel-submit-thread-reply-{thread_id}"),
            submit_debug_selector: format!("review-panel-submit-thread-reply-{thread_id}"),
        }
    }
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

pub(crate) fn render_review_thread_actions(state: ReviewThreadActionsState) -> impl IntoElement {
    let ReviewThreadActionsState {
        ids,
        thread_id,
        active_reply,
        reply_button_disabled,
        is_resolved,
        action_running,
        can_toggle_resolution,
        show_toggle_icon,
        view_entity,
    } = state;
    let toggle_label = if is_resolved { "Reopen" } else { "Resolve" };

    div()
        .flex()
        .items_center()
        .gap_2()
        .child(
            Button::new(ids.reply_button)
                .label(if active_reply { "Replying" } else { "Reply" })
                .xsmall()
                .outline()
                .disabled(reply_button_disabled)
                .debug_selector({
                    let selector = ids.reply_debug_selector.clone();
                    move || selector.clone()
                })
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
        .child({
            let button = Button::new(ids.toggle_button)
                .label(toggle_label)
                .xsmall()
                .ghost()
                .loading(action_running)
                .disabled(!can_toggle_resolution || action_running);
            let button = if show_toggle_icon {
                button.icon(if is_resolved {
                    IconName::Undo2
                } else {
                    IconName::CircleCheck
                })
            } else {
                button
            };

            button
                .debug_selector({
                    let selector = ids.toggle_debug_selector.clone();
                    move || selector.clone()
                })
                .on_click({
                    let view_entity = view_entity.clone();
                    let thread_id = thread_id.clone();
                    move |_, _, cx| {
                        view_entity.update(cx, |view, cx| {
                            view.set_review_thread_resolved(thread_id.clone(), !is_resolved, cx);
                        });
                    }
                })
        })
}

pub(crate) fn render_review_thread_status_pill(
    label: &str,
    text_color: gpui::Hsla,
) -> impl IntoElement {
    let tone = match label {
        "unresolved" => Tone::Warning,
        "resolved" => Tone::Success,
        "outdated" => Tone::Neutral,
        _ => Tone::Info,
    };
    let colors = tone_colors(tone);

    div()
        .rounded_xs()
        .border_1()
        .border_color(colors.border)
        .bg(colors.background)
        .px_1()
        .py_0p5()
        .text_xs()
        .font_medium()
        .text_color(text_color)
        .child(label.to_string())
}

pub(crate) fn render_review_thread_reply_composer(
    state: ReviewThreadReplyComposerState,
) -> impl IntoElement {
    let ReviewThreadReplyComposerState {
        ids,
        thread_id,
        input,
        input_height,
        disabled,
        submitting,
        error,
        chrome,
        view_entity,
    } = state;

    div()
        .when(
            matches!(chrome, ReviewThreadReplyComposerChrome::Inline),
            |element| {
                element
                    .border_t_1()
                    .border_color(color::border())
                    .bg(color::content_background())
                    .px_2()
                    .py_2()
            },
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
                        .h(input_height)
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
                    .text_color(color::danger())
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
                    Button::new(ids.cancel_button)
                        .label("Cancel")
                        .xsmall()
                        .ghost()
                        .disabled(submitting)
                        .debug_selector({
                            let selector = ids.cancel_debug_selector.clone();
                            move || selector.clone()
                        })
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
                    Button::new(ids.submit_button)
                        .label("Send reply")
                        .xsmall()
                        .primary()
                        .loading(submitting)
                        .disabled(disabled)
                        .debug_selector({
                            let selector = ids.submit_debug_selector.clone();
                            move || selector.clone()
                        })
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn derives_inline_review_thread_reply_ui_state() {
        let thread = review_thread(ReviewThreadState::Unresolved);
        let state = review_thread_ui_state(&thread, Some("thread"), false, true, Some("thread"));

        assert!(state.active_reply);
        assert!(state.action_running);
        assert!(state.reply_submitting);
        assert!(state.reply_disabled);
        assert!(state.reply_button_disabled);
        assert!(!state.is_resolved);
        assert!(state.can_toggle_resolution);

        let outdated_thread = review_thread(ReviewThreadState::Outdated);
        let state = review_thread_ui_state(&outdated_thread, Some("other"), true, true, None);

        assert!(!state.active_reply);
        assert!(!state.reply_submitting);
        assert!(state.reply_disabled);
        assert!(state.reply_button_disabled);
        assert!(!state.can_toggle_resolution);
    }

    fn review_thread(state: ReviewThreadState) -> ReviewThread {
        ReviewThread {
            id: "thread".to_string(),
            path: "src/app.rs".to_string(),
            range: None,
            state,
            comments: Vec::new(),
        }
    }
}
