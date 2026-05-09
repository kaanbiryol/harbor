use gpui::{
    AnyElement, Context, Entity, IntoElement, UniformListScrollHandle, div, prelude::*, px, rgb,
    uniform_list,
};
use gpui_component::input::InputState;
use harbor_domain::{PullRequestReview, PullRequestReviewState, ReviewThread, ReviewThreadState};

use crate::workspace::{AppView, ReviewThreadUiError};

use super::review_thread_chrome::{
    ReviewThreadActionIds, ReviewThreadActionsState, ReviewThreadReplyComposerChrome,
    ReviewThreadReplyComposerIds, ReviewThreadReplyComposerState, render_review_thread_actions,
    render_review_thread_reply_composer, review_thread_ui_state,
};

const REVIEW_THREAD_ROW_HEIGHT: f32 = 224.0;

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
    pub(crate) view_entity: Entity<AppView>,
}

pub(crate) fn render_review_panel(
    reviews: &[PullRequestReview],
    threads: &[ReviewThread],
    is_loading: bool,
    error: Option<&str>,
    scroll_handle: UniformListScrollHandle,
    cx: &mut Context<AppView>,
) -> impl IntoElement {
    let (unresolved, resolved, outdated) = review_thread_counts(threads);
    let view_entity = cx.entity().clone();

    div()
        .id("review-panel")
        .flex()
        .flex_col()
        .flex_1()
        .min_h_0()
        .gap_2()
        .child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .gap_3()
                .child("Review")
                .child(div().text_xs().text_color(rgb(0x9aa4b2)).child(format!(
                    "{} reviews  {} threads",
                    reviews.len(),
                    threads.len()
                ))),
        )
        .child(
            div()
                .flex()
                .gap_3()
                .text_xs()
                .text_color(rgb(0x9aa4b2))
                .child(format!("unresolved {unresolved}"))
                .child(format!("resolved {resolved}"))
                .child(format!("outdated {outdated}")),
        )
        .when(!reviews.is_empty(), |element| {
            element
                .child(
                    div()
                        .pt_1()
                        .text_xs()
                        .text_color(rgb(0x9aa4b2))
                        .child("latest reviews"),
                )
                .children(reviews.iter().rev().take(3).map(render_pull_request_review))
        })
        .when(is_loading, |element| {
            element.child(
                div()
                    .border_1()
                    .border_color(rgb(0x242a31))
                    .bg(rgb(0x0c0f12))
                    .p_3()
                    .text_color(rgb(0x9aa4b2))
                    .child("Loading review threads..."),
            )
        })
        .when_some(error.map(str::to_string), |element, error| {
            element.child(
                div()
                    .border_1()
                    .border_color(rgb(0x7f1d1d))
                    .bg(rgb(0x2a1212))
                    .p_3()
                    .text_color(rgb(0xf87171))
                    .child(error),
            )
        })
        .when(
            !is_loading && error.is_none() && threads.is_empty(),
            |element| {
                element.child(
                    div()
                        .border_1()
                        .border_color(rgb(0x242a31))
                        .bg(rgb(0x0c0f12))
                        .p_3()
                        .text_color(rgb(0x9aa4b2))
                        .child("No review threads found for this pull request"),
                )
            },
        )
        .when(!threads.is_empty(), |element| {
            element.child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_h_0()
                    .min_w_0()
                    .border_1()
                    .border_color(rgb(0x242a31))
                    .bg(rgb(0x0c0f12))
                    .overflow_hidden()
                    .child(
                        uniform_list(
                            "review-thread-list",
                            threads.len(),
                            cx.processor(
                                move |view, range: std::ops::Range<usize>, _window, _cx| {
                                    let mut rows = Vec::with_capacity(range.len());

                                    for index in range {
                                        let Some(thread) = view.review_threads.get(index) else {
                                            continue;
                                        };
                                        rows.push(render_review_thread_row(
                                            ReviewThreadRowRenderState {
                                                index,
                                                thread,
                                                active_review_thread_reply: view
                                                    .review_thread_reply_thread_id
                                                    .as_deref(),
                                                review_thread_reply_input: view
                                                    .review_thread_reply_input
                                                    .clone(),
                                                reply_body_empty: view
                                                    .review_thread_reply_input
                                                    .read(_cx)
                                                    .value()
                                                    .trim()
                                                    .is_empty(),
                                                is_submitting_reply: view
                                                    .is_submitting_review_thread_reply,
                                                reply_error: view
                                                    .review_thread_reply_error
                                                    .as_ref(),
                                                action_thread_id: view
                                                    .review_thread_action_thread_id
                                                    .as_deref(),
                                                action_error: view
                                                    .review_thread_action_error
                                                    .as_ref(),
                                                view_entity: view_entity.clone(),
                                            },
                                        ));
                                    }

                                    rows
                                },
                            ),
                        )
                        .track_scroll(&scroll_handle)
                        .flex_1()
                        .min_h_0()
                        .min_w_0(),
                    ),
            )
        })
}

pub(crate) fn render_pull_request_review(review: &PullRequestReview) -> impl IntoElement {
    let (label, color) = pull_request_review_state_label(review.state);

    div()
        .flex()
        .items_center()
        .justify_between()
        .gap_3()
        .border_1()
        .border_color(rgb(0x242a31))
        .bg(rgb(0x0c0f12))
        .px_3()
        .py_2()
        .child(
            div()
                .min_w_0()
                .flex_1()
                .child(format!("{} by {}", label, review.author))
                .when_some(
                    review.body.as_ref().map(|body| single_line(body)),
                    |element, body| {
                        element.child(
                            div()
                                .pt_1()
                                .text_xs()
                                .text_color(rgb(0x9aa4b2))
                                .truncate()
                                .child(body),
                        )
                    },
                ),
        )
        .child(
            div()
                .text_xs()
                .text_color(color)
                .child(review_time_label(review)),
        )
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
        view_entity,
    } = state;
    let (label, color) = review_thread_state_label(thread.state);
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
        rgb(0x1f2d3a)
    } else {
        rgb(0x20252b)
    };
    let row_bg_color = if is_resolved {
        rgb(0x0f151d)
    } else {
        rgb(0x101214)
    };
    let row_hover_bg_color = if is_resolved {
        rgb(0x17212c)
    } else {
        rgb(0x202a35)
    };
    let path_color = if is_resolved {
        rgb(0xb7c0cd)
    } else {
        rgb(0xe6e8eb)
    };
    let metadata_color = if is_resolved {
        rgb(0x637186)
    } else {
        rgb(0x9aa4b2)
    };
    let reply_error = reply_error
        .filter(|error| error.thread_id == thread.id)
        .map(|error| error.message.clone());
    let action_error = action_error
        .filter(|error| error.thread_id == thread.id)
        .map(|error| error.message.clone());
    let thread_id = thread.id.clone();

    div()
        .id(("review-thread-row", index))
        .h(px(REVIEW_THREAD_ROW_HEIGHT))
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
                .child(div().text_xs().text_color(color).child(label)),
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
                    show_toggle_icon: false,
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
            element.child(div().text_xs().text_color(rgb(0xf87171)).child(error))
        })
        .into_any_element()
}

pub(crate) fn review_thread_counts(threads: &[ReviewThread]) -> (usize, usize, usize) {
    let mut unresolved = 0;
    let mut resolved = 0;
    let mut outdated = 0;

    for thread in threads {
        match thread.state {
            ReviewThreadState::Unresolved => unresolved += 1,
            ReviewThreadState::Resolved => resolved += 1,
            ReviewThreadState::Outdated => outdated += 1,
        }
    }

    (unresolved, resolved, outdated)
}

pub(crate) fn review_thread_location(thread: &ReviewThread) -> String {
    thread
        .comments
        .iter()
        .find_map(|comment| comment.position.as_ref())
        .and_then(|position| position.line.or(position.original_line))
        .map_or_else(|| "file".to_string(), |line| format!("line {line}"))
}

pub(crate) fn single_line(value: &str) -> String {
    value
        .lines()
        .find(|line| !line.trim().is_empty())
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .unwrap_or("empty comment")
        .to_string()
}

pub(crate) fn pull_request_review_state_label(
    state: PullRequestReviewState,
) -> (&'static str, gpui::Hsla) {
    match state {
        PullRequestReviewState::Pending => ("pending", rgb(0xfbbf24).into()),
        PullRequestReviewState::Commented => ("commented", rgb(0x93c5fd).into()),
        PullRequestReviewState::Approved => ("approved", rgb(0x34d399).into()),
        PullRequestReviewState::ChangesRequested => ("changes requested", rgb(0xf87171).into()),
        PullRequestReviewState::Dismissed => ("dismissed", rgb(0x9aa4b2).into()),
    }
}

pub(crate) fn review_thread_state_label(state: ReviewThreadState) -> (&'static str, gpui::Hsla) {
    match state {
        ReviewThreadState::Unresolved => ("unresolved", rgb(0xfbbf24).into()),
        ReviewThreadState::Resolved => ("resolved", rgb(0x34d399).into()),
        ReviewThreadState::Outdated => ("outdated", rgb(0x9aa4b2).into()),
    }
}

pub(crate) fn review_time_label(review: &PullRequestReview) -> String {
    review
        .submitted_at
        .map(|submitted_at| submitted_at.format("%Y-%m-%d %H:%M").to_string())
        .unwrap_or_else(|| "not submitted".to_string())
}
