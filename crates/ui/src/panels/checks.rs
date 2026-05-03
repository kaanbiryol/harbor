use gpui::{IntoElement, div, prelude::*, rgb};
use harbor_domain::{CheckConclusion, CheckRun, CheckStatus, ChecksSummary};

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
                .text_color(rgb(0x9aa4b2))
                .child(format!("total {}", summary.total))
                .child(format!("passed {}", summary.passed))
                .child(format!("failed {}", summary.failed))
                .child(format!("pending {}", summary.pending))
                .child(format!("skipped {}", summary.skipped)),
        )
        .when(is_loading, |element| {
            element.child(
                div()
                    .rounded_sm()
                    .border_1()
                    .border_color(rgb(0x242a31))
                    .bg(rgb(0x0c0f12))
                    .p_3()
                    .text_color(rgb(0x9aa4b2))
                    .child("Loading check runs..."),
            )
        })
        .when_some(error.map(str::to_string), |element, error| {
            element.child(
                div()
                    .rounded_sm()
                    .border_1()
                    .border_color(rgb(0x242a31))
                    .bg(rgb(0x0c0f12))
                    .p_3()
                    .text_color(rgb(0xf87171))
                    .child(error),
            )
        })
        .when(
            !is_loading && error.is_none() && check_runs.is_empty(),
            |element| {
                element.child(
                    div()
                        .rounded_sm()
                        .border_1()
                        .border_color(rgb(0x242a31))
                        .bg(rgb(0x0c0f12))
                        .p_3()
                        .text_color(rgb(0x9aa4b2))
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
        .rounded_sm()
        .border_1()
        .border_color(rgb(0x242a31))
        .bg(rgb(0x0c0f12))
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
                        .text_color(rgb(0x9aa4b2))
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
    let (label, color) = match (status, conclusion) {
        (CheckStatus::Completed, Some(CheckConclusion::Success)) => ("passed", rgb(0x34d399)),
        (CheckStatus::Completed, Some(CheckConclusion::Skipped)) => ("skipped", rgb(0x9aa4b2)),
        (CheckStatus::Completed, Some(CheckConclusion::Neutral)) => ("neutral", rgb(0x9aa4b2)),
        (CheckStatus::Completed, Some(CheckConclusion::Cancelled)) => ("cancelled", rgb(0xfbbf24)),
        (CheckStatus::Completed, Some(CheckConclusion::TimedOut)) => ("timed out", rgb(0xf87171)),
        (CheckStatus::Completed, Some(CheckConclusion::ActionRequired)) => {
            ("action required", rgb(0xfbbf24))
        }
        (CheckStatus::Completed, Some(CheckConclusion::Failure) | None) => {
            ("failed", rgb(0xf87171))
        }
        (CheckStatus::InProgress, _) => ("running", rgb(0x93c5fd)),
        (CheckStatus::Queued, _) => ("queued", rgb(0xfbbf24)),
    };

    div().text_sm().text_color(color).child(label)
}
