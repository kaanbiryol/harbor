use gpui::{IntoElement, div, prelude::*};
use harbor_domain::{CheckConclusion, CheckRun, CheckStatus, ChecksSummary};

use crate::visual::{Tone, color, tone_text};

pub(crate) fn render_checks_panel(
    summary: ChecksSummary,
    check_runs: &[CheckRun],
    is_loading: bool,
    error: Option<&str>,
) -> impl IntoElement {
    div()
        .id("checks-panel-scroll")
        .flex()
        .flex_col()
        .flex_1()
        .min_h_0()
        .overflow_y_scroll()
        .gap_2()
        .child("Checks summary")
        .child(
            div()
                .flex()
                .gap_3()
                .text_xs()
                .text_color(color::text_muted())
                .child(format!("total {}", summary.total))
                .child(format!("passed {}", summary.passed))
                .child(format!("failed {}", summary.failed))
                .child(format!("pending {}", summary.pending))
                .child(format!("skipped {}", summary.skipped)),
        )
        .when(is_loading, |element| {
            element.child(
                div()
                    .border_1()
                    .border_color(color::border())
                    .bg(color::content_background())
                    .p_3()
                    .text_color(color::text_muted())
                    .child("Loading check runs..."),
            )
        })
        .when_some(error.map(str::to_string), |element, error| {
            element.child(
                div()
                    .border_1()
                    .border_color(color::border())
                    .bg(color::content_background())
                    .p_3()
                    .text_color(color::danger())
                    .child(error),
            )
        })
        .when(
            !is_loading && error.is_none() && check_runs.is_empty(),
            |element| {
                element.child(
                    div()
                        .border_1()
                        .border_color(color::border())
                        .bg(color::content_background())
                        .p_3()
                        .text_color(color::text_muted())
                        .child("No check runs found for this PR head"),
                )
            },
        )
        .children(check_runs.iter().map(render_check_run))
}

pub(crate) fn render_check_run(check_run: &CheckRun) -> impl IntoElement {
    div()
        .flex()
        .items_center()
        .justify_between()
        .gap_3()
        .border_1()
        .border_color(color::border())
        .bg(color::content_background())
        .px_3()
        .py_2()
        .child(
            div()
                .flex()
                .flex_col()
                .gap_1()
                .child(check_run.name.clone())
                .child(
                    div()
                        .text_xs()
                        .text_color(color::text_muted())
                        .child(format!("{:?}", check_run.status)),
                ),
        )
        .child(render_check_conclusion(
            check_run.conclusion,
            check_run.status,
        ))
}

pub(crate) fn render_check_conclusion(
    conclusion: Option<CheckConclusion>,
    status: CheckStatus,
) -> impl IntoElement {
    let (label, tone) = match (status, conclusion) {
        (CheckStatus::Completed, Some(CheckConclusion::Success)) => ("passed", Tone::Success),
        (CheckStatus::Completed, Some(CheckConclusion::Skipped)) => ("skipped", Tone::Neutral),
        (CheckStatus::Completed, Some(CheckConclusion::Neutral)) => ("neutral", Tone::Neutral),
        (CheckStatus::Completed, Some(CheckConclusion::Cancelled)) => ("cancelled", Tone::Warning),
        (CheckStatus::Completed, Some(CheckConclusion::TimedOut)) => ("timed out", Tone::Danger),
        (CheckStatus::Completed, Some(CheckConclusion::ActionRequired)) => {
            ("action required", Tone::Warning)
        }
        (CheckStatus::Completed, Some(CheckConclusion::Failure) | None) => ("failed", Tone::Danger),
        (CheckStatus::InProgress, _) => ("running", Tone::Info),
        (CheckStatus::Queued, _) => ("queued", Tone::Warning),
    };

    div().text_sm().text_color(tone_text(tone)).child(label)
}
