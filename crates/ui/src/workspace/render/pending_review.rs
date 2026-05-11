use gpui::{Context, IntoElement, div, prelude::*, rgb};
use gpui_component::{Disableable, Sizable, StyledExt, button::Button, input::Input};
use harbor_github::SubmitPullRequestReviewEvent;

use crate::workspace::{AppView, PendingReviewSession};

fn pending_review_comment_count_label(comment_count: usize) -> String {
    match comment_count {
        0 => "pending comments".to_string(),
        1 => "1 pending comment".to_string(),
        count => format!("{count} pending comments"),
    }
}

impl AppView {
    pub(super) fn render_pending_review_bar(
        &self,
        pending_review: PendingReviewSession,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let body_input = self
            .review_state
            .review_composer_state
            .pending_review_body_input
            .clone();
        let submitting = self.review_state.is_submitting_pending_review();
        let body_empty = self
            .review_state
            .review_composer_state
            .pending_review_body_input
            .read(cx)
            .value()
            .trim()
            .is_empty();
        let comment_submit_disabled =
            submitting || (pending_review.comment_count == 0 && body_empty);

        div().pt_3().child(
            div()
                .border_1()
                .border_color(rgb(0x355071))
                .bg(rgb(0x172033))
                .px_3()
                .py_2()
                .child(
                    div()
                        .flex()
                        .items_center()
                        .justify_between()
                        .gap_2()
                        .text_xs()
                        .child(
                            div()
                                .font_medium()
                                .text_color(rgb(0xe6e8eb))
                                .child("pending review"),
                        )
                        .child(div().text_color(rgb(0x93c5fd)).child(
                            pending_review_comment_count_label(pending_review.comment_count),
                        )),
                )
                .child(
                    div().pt_2().child(
                        Input::new(&body_input)
                            .small()
                            .w_full()
                            .appearance(false)
                            .bordered(true)
                            .focus_bordered(true),
                    ),
                )
                .child(
                    div()
                        .pt_2()
                        .flex()
                        .items_center()
                        .gap_2()
                        .child(
                            Button::new("submit-pending-approve")
                                .label("Approve")
                                .small()
                                .outline()
                                .loading(submitting)
                                .disabled(submitting)
                                .on_click(cx.listener(|view, _, window, cx| {
                                    view.submit_pending_pull_request_review(
                                        SubmitPullRequestReviewEvent::Approve,
                                        window,
                                        cx,
                                    );
                                })),
                        )
                        .child(
                            Button::new("submit-pending-comment")
                                .label("Comment")
                                .small()
                                .outline()
                                .loading(submitting)
                                .disabled(comment_submit_disabled)
                                .on_click(cx.listener(|view, _, window, cx| {
                                    view.submit_pending_pull_request_review(
                                        SubmitPullRequestReviewEvent::Comment,
                                        window,
                                        cx,
                                    );
                                })),
                        )
                        .child(
                            Button::new("submit-pending-request-changes")
                                .label("Request changes")
                                .small()
                                .outline()
                                .loading(submitting)
                                .disabled(submitting)
                                .on_click(cx.listener(|view, _, window, cx| {
                                    view.submit_pending_pull_request_review(
                                        SubmitPullRequestReviewEvent::RequestChanges,
                                        window,
                                        cx,
                                    );
                                })),
                        ),
                )
                .when_some(
                    self.review_state.pending_review_error().map(str::to_string),
                    |element, error| {
                        element.child(
                            div()
                                .pt_2()
                                .text_xs()
                                .text_color(rgb(0xf87171))
                                .child(error),
                        )
                    },
                ),
        )
    }
}
