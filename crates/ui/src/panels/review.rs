use gpui::{Context, IntoElement, UniformListScrollHandle, div, prelude::*, uniform_list};
use gpui_component::StyledExt;
use harbor_domain::{PullRequestReview, PullRequestReviewState, ReviewThread, ReviewThreadState};

use crate::{
    visual::{Tone, color, tone_text},
    workspace::AppView,
};

use super::review_thread_rows::{ReviewThreadRowRenderState, render_review_thread_row};
use super::{
    render_empty_panel_card, render_error_panel_card, render_metric_pill, render_panel_card,
    render_panel_header, render_status_pill,
};

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
        .child(render_panel_header(
            "Review",
            Some(format!(
                "{} reviews  {} threads",
                reviews.len(),
                threads.len()
            )),
        ))
        .child(
            div()
                .flex()
                .items_center()
                .gap_2()
                .flex_wrap()
                .child(render_metric_pill("unresolved", unresolved, Tone::Warning))
                .child(render_metric_pill("resolved", resolved, Tone::Success))
                .child(render_metric_pill("outdated", outdated, Tone::Neutral)),
        )
        .when(!reviews.is_empty(), |element| {
            element
                .child(
                    div()
                        .pt_1()
                        .text_xs()
                        .font_medium()
                        .text_color(color::text_secondary())
                        .child("latest reviews"),
                )
                .children(reviews.iter().rev().take(3).map(render_pull_request_review))
        })
        .when(is_loading, |element| {
            element.child(render_empty_panel_card("Loading review threads..."))
        })
        .when_some(error.map(str::to_string), |element, error| {
            element.child(render_error_panel_card(error))
        })
        .when(
            !is_loading && error.is_none() && threads.is_empty(),
            |element| {
                element.child(render_empty_panel_card(
                    "No review threads found for this pull request",
                ))
            },
        )
        .when(!threads.is_empty(), |element| {
            element.child(
                render_panel_card()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_h_0()
                    .min_w_0()
                    .overflow_hidden()
                    .child(
                        uniform_list(
                            "review-thread-list",
                            threads.len(),
                            cx.processor(
                                move |view, range: std::ops::Range<usize>, _window, _cx| {
                                    let mut rows = Vec::with_capacity(range.len());

                                    for index in range {
                                        let Some(thread) =
                                            view.review_state.review_threads.get(index)
                                        else {
                                            continue;
                                        };
                                        rows.push(render_review_thread_row(
                                            ReviewThreadRowRenderState {
                                                index,
                                                thread,
                                                active_review_thread_reply: view
                                                    .review_state
                                                    .review_composer_state
                                                    .active_thread_reply(),
                                                review_thread_reply_input: view
                                                    .review_state
                                                    .review_composer_state
                                                    .thread_reply_input
                                                    .clone(),
                                                reply_body_empty: view
                                                    .review_state
                                                    .review_composer_state
                                                    .thread_reply_input
                                                    .read(_cx)
                                                    .value()
                                                    .trim()
                                                    .is_empty(),
                                                is_submitting_reply: view
                                                    .review_state
                                                    .is_submitting_review_thread_reply(),
                                                reply_error: view
                                                    .review_state
                                                    .review_thread_reply_error(),
                                                action_thread_id: view
                                                    .review_state
                                                    .review_thread_action_thread_id(),
                                                action_error: view
                                                    .review_state
                                                    .review_thread_action_error(),
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
    let (label, tone) = pull_request_review_state_tone(review.state);

    render_panel_card()
        .flex()
        .items_center()
        .justify_between()
        .gap_3()
        .px_3()
        .py_2()
        .child(
            div()
                .min_w_0()
                .flex_1()
                .font_medium()
                .text_color(color::text_primary())
                .child(review.author.clone())
                .when_some(
                    review.body.as_ref().map(|body| single_line(body)),
                    |element, body| {
                        element.child(
                            div()
                                .pt_1()
                                .text_xs()
                                .text_color(color::text_muted())
                                .truncate()
                                .child(body),
                        )
                    },
                ),
        )
        .child(
            div()
                .flex()
                .items_center()
                .gap_2()
                .text_xs()
                .child(
                    div()
                        .text_color(color::text_muted())
                        .child(review_time_label(review)),
                )
                .child(render_status_pill(label, tone)),
        )
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

fn pull_request_review_state_tone(state: PullRequestReviewState) -> (&'static str, Tone) {
    match state {
        PullRequestReviewState::Pending => ("pending", Tone::Warning),
        PullRequestReviewState::Commented => ("commented", Tone::Info),
        PullRequestReviewState::Approved => ("approved", Tone::Success),
        PullRequestReviewState::ChangesRequested => ("changes requested", Tone::Danger),
        PullRequestReviewState::Dismissed => ("dismissed", Tone::Neutral),
    }
}

pub(crate) fn review_thread_state_label(state: ReviewThreadState) -> (&'static str, gpui::Hsla) {
    let (label, tone) = review_thread_state_tone(state);

    (label, tone_text(tone).into())
}

pub(super) fn review_thread_state_tone(state: ReviewThreadState) -> (&'static str, Tone) {
    match state {
        ReviewThreadState::Unresolved => ("unresolved", Tone::Warning),
        ReviewThreadState::Resolved => ("resolved", Tone::Success),
        ReviewThreadState::Outdated => ("outdated", Tone::Neutral),
    }
}

pub(crate) fn review_time_label(review: &PullRequestReview) -> String {
    review
        .submitted_at
        .map(|submitted_at| submitted_at.format("%Y-%m-%d %H:%M").to_string())
        .unwrap_or_else(|| "not submitted".to_string())
}

#[cfg(test)]
mod tests {
    use gpui::{
        Context, Entity, IntoElement, Modifiers, Render, TestAppContext, VisualTestContext, Window,
    };
    use gpui_component::{Root, Theme, ThemeMode, input::InputState};

    use super::*;
    use crate::{
        test_fixtures::review_thread as test_review_thread, workspace::ReviewThreadUiError,
    };

    #[test]
    fn counts_review_threads_by_state() {
        let threads = vec![
            review_thread_with_state(ReviewThreadState::Unresolved),
            review_thread_with_state(ReviewThreadState::Resolved),
            review_thread_with_state(ReviewThreadState::Outdated),
            review_thread_with_state(ReviewThreadState::Unresolved),
        ];

        assert_eq!(review_thread_counts(&threads), (2, 1, 1));
    }

    #[gpui::test]
    async fn review_panel_reply_button_opens_and_cancel_clears_reply_mode(cx: &mut TestAppContext) {
        let (view_entity, cx) = init_visual_review_panel_test(cx);

        render_review_panel_row_harness(cx);
        let reply_bounds = cx
            .debug_bounds("review-panel-reply-thread-thread-1")
            .expect("review panel reply button should render");
        cx.simulate_click(reply_bounds.center(), Modifiers::none());

        assert_eq!(
            view_entity.read_with(cx, |view, _| view
                .review_state
                .review_composer_state
                .active_thread_reply()
                .map(str::to_string)),
            Some("thread-1".to_string())
        );

        render_review_panel_row_harness(cx);
        let cancel_bounds = cx
            .debug_bounds("review-panel-cancel-thread-reply-thread-1")
            .expect("review panel reply cancel button should render");
        cx.simulate_click(cancel_bounds.center(), Modifiers::none());

        assert!(view_entity.read_with(cx, |view, _| {
            view.review_state
                .review_composer_state
                .active_thread_reply()
                .is_none()
        }));
    }

    #[gpui::test]
    async fn review_panel_toggle_reports_missing_selected_pull_request(cx: &mut TestAppContext) {
        let (view_entity, cx) = init_visual_review_panel_test(cx);

        render_review_panel_row_harness(cx);
        let toggle_bounds = cx
            .debug_bounds("review-panel-toggle-thread-thread-1")
            .expect("review panel toggle button should render");
        cx.simulate_click(toggle_bounds.center(), Modifiers::none());

        assert_eq!(
            view_entity.read_with(cx, |view, _| {
                view.review_state
                    .review_thread_action_error()
                    .map(|error| (error.thread_id.clone(), error.message.clone()))
            }),
            Some((
                "thread-1".to_string(),
                "Select a pull request before updating a thread".to_string()
            ))
        );
    }

    fn init_visual_review_panel_test(
        cx: &mut TestAppContext,
    ) -> (Entity<AppView>, &mut VisualTestContext) {
        cx.update(|cx| {
            gpui_component::init(cx);
            Theme::change(ThemeMode::Dark, None, cx);
        });

        let mut view_entity = None;
        let (_, cx) = cx.add_window_view(|window, cx| {
            let view = cx.new(|cx| AppView::new_without_startup_tasks(window, cx));
            let harness = cx.new(|_| ReviewPanelRowHarness {
                view_entity: view.clone(),
                thread: review_thread(),
            });
            view_entity = Some(view);
            Root::new(harness, window, cx)
        });

        (view_entity.expect("test AppView should be created"), cx)
    }

    fn render_review_panel_row_harness(cx: &mut VisualTestContext) {
        cx.refresh().expect("test window should refresh");
        cx.run_until_parked();
    }

    struct ReviewPanelRowHarness {
        view_entity: Entity<AppView>,
        thread: ReviewThread,
    }

    impl Render for ReviewPanelRowHarness {
        fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            let render_state =
                self.view_entity
                    .read_with(cx, |view, app| ReviewPanelRowTestState {
                        active_reply_thread_id: view
                            .review_state
                            .review_composer_state
                            .active_thread_reply()
                            .map(str::to_string),
                        reply_input: view
                            .review_state
                            .review_composer_state
                            .thread_reply_input
                            .clone(),
                        reply_body_empty: view
                            .review_state
                            .review_composer_state
                            .thread_reply_input
                            .read(app)
                            .value()
                            .trim()
                            .is_empty(),
                        is_submitting_reply: view.review_state.is_submitting_review_thread_reply(),
                        reply_error: view.review_state.review_thread_reply_error().cloned(),
                        action_thread_id: view
                            .review_state
                            .review_thread_action_thread_id()
                            .map(str::to_string),
                        action_error: view.review_state.review_thread_action_error().cloned(),
                    });

            render_review_thread_row(ReviewThreadRowRenderState {
                index: 0,
                thread: &self.thread,
                active_review_thread_reply: render_state.active_reply_thread_id.as_deref(),
                review_thread_reply_input: render_state.reply_input.clone(),
                reply_body_empty: render_state.reply_body_empty,
                is_submitting_reply: render_state.is_submitting_reply,
                reply_error: render_state.reply_error.as_ref(),
                action_thread_id: render_state.action_thread_id.as_deref(),
                action_error: render_state.action_error.as_ref(),
                view_entity: self.view_entity.clone(),
            })
        }
    }

    struct ReviewPanelRowTestState {
        active_reply_thread_id: Option<String>,
        reply_input: Entity<InputState>,
        reply_body_empty: bool,
        is_submitting_reply: bool,
        reply_error: Option<ReviewThreadUiError>,
        action_thread_id: Option<String>,
        action_error: Option<ReviewThreadUiError>,
    }

    fn review_thread() -> ReviewThread {
        review_thread_with_state(ReviewThreadState::Unresolved)
    }

    fn review_thread_with_state(state: ReviewThreadState) -> ReviewThread {
        test_review_thread(state)
    }
}
