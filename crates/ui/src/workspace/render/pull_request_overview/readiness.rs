use gpui::{AnyElement, Context, div, prelude::*};
use gpui_component::{
    Sizable,
    button::{Button, ButtonVariants},
};
use harbor_domain::{MergeState, PullRequest};

use crate::{actions::PanelTab, icons::Octicon, visual::Tone, workspace::AppView};

use super::sidebar::{
    merge_readiness, pull_request_readiness, render_overview_card, render_readiness_row,
    render_readiness_section_title, render_readiness_status, render_summary_row, review_readiness,
    review_readiness_description,
};

impl AppView {
    pub(super) fn render_merge_readiness_card(
        &self,
        pr: &PullRequest,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let (review_label, review_tone) = review_readiness(pr.review_decision);
        let (merge_label, merge_description, merge_tone) = merge_readiness(pr);
        let (status_label, status_description, status_tone) = pull_request_readiness(pr);
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
                review_label,
                Octicon::Eye,
                review_tone,
                false,
            ))
            .child(render_readiness_row(
                "pull-request-merge-readiness-row",
                "Merge",
                merge_description,
                merge_label,
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
                            "Resolve open threads",
                            format!("{} open", pr.unresolved_threads),
                            Octicon::CommentDiscussion,
                            unresolved_tone,
                            true,
                        )
                        .on_click(cx.listener(|view, _, _, cx| {
                            view.select_panel_tab(PanelTab::Review, cx);
                        })),
                    ),
            )
            .child(render_readiness_section_title("Summary"))
            .child(render_summary_row(
                "pull-request-checks-summary-row",
                checks_summary_title,
                checks_label,
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
