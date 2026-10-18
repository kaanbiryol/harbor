use crate::gpui_component::{
    Icon, Sizable, StyledExt,
    button::{Button, ButtonVariants},
    list::ListItem,
};
use gpui::{AnyElement, Context, Div, SharedString, div, prelude::*, px};
use harbor_domain::{MergeState, PullRequest, ReviewDecision};

use crate::panels::{PullRequestMergeQueueStatus, pull_request_merge_queue_status};
use crate::{
    icons::Octicon,
    visual::{Tone, color, tone_colors},
    workspace::AppView,
};

use super::sidebar::render_overview_card;
use crate::panels::{
    MergeReadiness, PullRequestReadiness, ReviewReadiness,
    merge_readiness as classify_merge_readiness,
    pull_request_readiness as classify_pull_request_readiness,
    review_readiness as classify_review_readiness,
};

impl AppView {
    pub(super) fn render_merge_readiness_card(
        &self,
        pr: &PullRequest,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let (_, review_tone) = review_readiness(pr.review_decision);
        let (_, merge_description, merge_tone) = merge_readiness(pr);
        let (status_label, status_description, status_tone) = pull_request_readiness(pr);
        let merge_queue_status = pull_request_merge_queue_status(pr);
        let unresolved_tone = if pr.unresolved_threads == 0 {
            Tone::Success
        } else {
            Tone::Warning
        };
        let checks_tone = if pr.checks_summary.failed > 0 {
            Tone::Danger
        } else if pr.checks_summary.pending > 0 {
            Tone::Warning
        } else {
            Tone::Success
        };
        let checks_label = if pr.checks_summary.failed > 0 {
            format!("{} failed", pr.checks_summary.failed)
        } else if pr.checks_summary.pending > 0 {
            format!("{} pending", pr.checks_summary.pending)
        } else {
            format!("{} passed", pr.checks_summary.passed)
        };
        let checks_summary_title = if pr.checks_summary.failed > 0 {
            "Checks need attention"
        } else if pr.checks_summary.pending > 0 {
            "Checks running"
        } else {
            "Checks passed"
        };
        let (conflicts_label, conflicts_tone) = if pr.merge_state == Some(MergeState::Dirty) {
            ("Conflicts", Tone::Danger)
        } else {
            ("No conflicts", Tone::Success)
        };
        let pull_request_url = pr.url.clone();
        let close_pull_request_url = pr.url.clone();

        render_overview_card("PR status")
            .debug_selector(|| "pull-request-merge-readiness".to_string())
            .gap_0()
            .child(render_readiness_status(
                status_label,
                status_description,
                status_tone,
            ))
            .child(render_readiness_section_title("Readiness checklist"))
            .child(render_readiness_row(
                "pull-request-review-readiness-row",
                "Review",
                review_readiness_description(pr.review_decision),
                Octicon::Eye,
                review_tone,
                false,
            ))
            .child(render_readiness_row(
                "pull-request-merge-readiness-row",
                "Merge",
                merge_description,
                Octicon::CheckCircle,
                merge_tone,
                false,
            ))
            .child(
                div()
                    .debug_selector(|| "pull-request-unresolved-conversations".to_string())
                    .child(
                        render_readiness_row(
                            "pull-request-unresolved-conversations-row",
                            "Conversations",
                            format!(
                                "Resolve {} open thread{}",
                                pr.unresolved_threads,
                                if pr.unresolved_threads == 1 { "" } else { "s" }
                            ),
                            Octicon::CommentDiscussion,
                            unresolved_tone,
                            true,
                        )
                        .on_click(cx.listener(|view, _, _, cx| {
                            view.set_overview_unresolved_threads_only(true, cx);
                        })),
                    ),
            )
            .child(render_readiness_section_title("Summary"))
            .when_some(merge_queue_status, |card, queue_status| {
                let (icon, value) = match queue_status {
                    PullRequestMergeQueueStatus::Waiting => {
                        (Octicon::Clock, queue_status.label().to_string())
                    }
                    PullRequestMergeQueueStatus::Queued => (
                        Octicon::GitPullRequest,
                        format!("Queued for {}", pr.base_ref),
                    ),
                };
                card.child(render_summary_row(
                    "pull-request-merge-queue-summary-row",
                    "Merge queue",
                    value,
                    icon,
                    Tone::Warning,
                ))
            })
            .child(render_summary_row(
                "pull-request-checks-summary-row",
                checks_summary_title,
                checks_label,
                Octicon::CheckCircle,
                checks_tone,
            ))
            .child(render_summary_row(
                "pull-request-conflicts-summary-row",
                conflicts_label,
                if conflicts_tone == Tone::Success {
                    "Up to date"
                } else {
                    "Resolve to merge"
                },
                Octicon::CheckCircle,
                conflicts_tone,
            ))
            .child(
                div()
                    .pt_2()
                    .flex()
                    .flex_wrap()
                    .gap_1()
                    .child(
                        Button::new("pull-request-draft-action")
                            .label(if pr.is_draft {
                                "Mark ready for review"
                            } else {
                                "Convert to draft"
                            })
                            .xsmall()
                            .link()
                            .on_click(move |_, _, cx| cx.open_url(&pull_request_url)),
                    )
                    .child(
                        Button::new("close-pull-request-action")
                            .label("Close pull request")
                            .xsmall()
                            .link()
                            .on_click(move |_, _, cx| cx.open_url(&close_pull_request_url)),
                    ),
            )
            .into_any_element()
    }
}

pub(super) fn render_readiness_row(
    id: &'static str,
    label: &'static str,
    description: impl Into<SharedString>,
    icon: Octicon,
    tone: Tone,
    navigable: bool,
) -> ListItem {
    let colors = tone_colors(tone);
    let description = description.into();

    ListItem::new(id)
        .w_full()
        .h(px(52.0))
        .px_0()
        .py_0()
        .rounded_none()
        .disabled(!navigable)
        .when(label != "Review", |element| {
            element.border_t_1().border_color(color::border_subtle())
        })
        .child(
            div()
                .w_full()
                .flex()
                .items_center()
                .gap_2()
                .child(Icon::new(icon).xsmall().text_color(colors.text))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .flex()
                        .flex_col()
                        .gap_0p5()
                        .child(
                            div()
                                .text_sm()
                                .text_color(color::text_primary())
                                .child(label),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(color::text_muted())
                                .child(description),
                        ),
                ),
        )
}

pub(super) fn render_readiness_status(
    label: &'static str,
    description: &'static str,
    tone: Tone,
) -> Div {
    let colors = tone_colors(tone);

    div()
        .py_3()
        .flex()
        .items_center()
        .gap_3()
        .child(
            div()
                .size(px(44.0))
                .flex_none()
                .rounded_full()
                .flex()
                .items_center()
                .justify_center()
                .bg(colors.background)
                .child(
                    Icon::new(Octicon::CodeSquare)
                        .size(px(16.0))
                        .text_color(colors.text),
                ),
        )
        .child(
            div()
                .min_w_0()
                .flex()
                .flex_col()
                .gap_0p5()
                .child(
                    div()
                        .text_size(px(16.0))
                        .font_medium()
                        .text_color(colors.text)
                        .child(label),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(color::text_muted())
                        .child(description),
                ),
        )
}

pub(super) fn render_readiness_section_title(title: &'static str) -> Div {
    div()
        .mt_2()
        .pt_3()
        .pb_1()
        .border_t_1()
        .border_color(color::border_subtle())
        .text_sm()
        .font_semibold()
        .text_color(color::text_primary())
        .child(title)
}

pub(super) fn render_summary_row(
    id: &'static str,
    label: &'static str,
    value: impl Into<String>,
    icon: Octicon,
    tone: Tone,
) -> impl IntoElement {
    let colors = tone_colors(tone);

    div()
        .id(id)
        .h(px(36.0))
        .flex()
        .items_center()
        .gap_2()
        .child(Icon::new(icon).xsmall().text_color(colors.text))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .text_xs()
                .text_color(color::text_secondary())
                .child(label),
        )
        .child(
            div()
                .text_xs()
                .text_color(color::text_muted())
                .child(value.into()),
        )
}

pub(super) fn pull_request_readiness(pr: &PullRequest) -> (&'static str, &'static str, Tone) {
    let readiness = classify_pull_request_readiness(pr);
    let tone = match readiness {
        PullRequestReadiness::Conflicts
        | PullRequestReadiness::ChecksFailed
        | PullRequestReadiness::ChangesRequested => Tone::Danger,
        PullRequestReadiness::ChecksPending
        | PullRequestReadiness::ReviewRequired
        | PullRequestReadiness::ConversationsOpen => Tone::Warning,
        PullRequestReadiness::Draft => Tone::Neutral,
        PullRequestReadiness::Ready => Tone::Success,
    };

    (readiness.label(), readiness.description(), tone)
}

pub(super) fn review_readiness_description(decision: Option<ReviewDecision>) -> &'static str {
    classify_review_readiness(decision).description()
}

pub(super) fn review_readiness(decision: Option<ReviewDecision>) -> (&'static str, Tone) {
    let readiness = classify_review_readiness(decision);
    let tone = match readiness {
        ReviewReadiness::Approved => Tone::Success,
        ReviewReadiness::ChangesRequested => Tone::Danger,
        ReviewReadiness::Pending => Tone::Warning,
    };

    (readiness.label(), tone)
}

pub(super) fn merge_readiness(pr: &PullRequest) -> (&'static str, &'static str, Tone) {
    let readiness = classify_merge_readiness(pr);
    let tone = match readiness {
        MergeReadiness::Conflicts | MergeReadiness::Blocked => Tone::Danger,
        MergeReadiness::Behind
        | MergeReadiness::WaitingForApproval
        | MergeReadiness::ConversationsOpen
        | MergeReadiness::ChecksPending => Tone::Warning,
        MergeReadiness::Unknown => Tone::Neutral,
        MergeReadiness::Ready => Tone::Success,
    };

    (readiness.label(), readiness.description(), tone)
}
