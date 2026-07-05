use gpui::{Context, IntoElement, ListState, div, list, prelude::*};
use harbor_domain::{CheckConclusion, CheckRun, CheckStatus, ChecksSummary};

use crate::{
    visual::{Tone, color},
    workspace::AppView,
};

use super::{
    render_empty_panel_card, render_error_panel_card, render_metric_pill, render_panel_card,
    render_panel_header, render_status_pill, sync_virtual_list_item_count,
};

pub(crate) fn render_checks_panel(
    summary: ChecksSummary,
    check_runs: &[CheckRun],
    is_loading: bool,
    error: Option<&str>,
    list_state: ListState,
    cx: &mut Context<AppView>,
) -> impl IntoElement {
    sync_virtual_list_item_count(&list_state, check_runs.len());

    div()
        .id("checks-panel")
        .flex()
        .flex_col()
        .flex_1()
        .min_h_0()
        .gap_2()
        .child(render_panel_header(
            "Checks",
            Some(format!("{} runs", summary.total)),
        ))
        .child(
            div()
                .flex()
                .items_center()
                .gap_2()
                .flex_wrap()
                .child(render_metric_pill("total", summary.total, Tone::Neutral))
                .child(render_metric_pill("passed", summary.passed, Tone::Success))
                .child(render_metric_pill("failed", summary.failed, Tone::Danger))
                .child(render_metric_pill(
                    "pending",
                    summary.pending,
                    Tone::Warning,
                ))
                .child(render_metric_pill(
                    "skipped",
                    summary.skipped,
                    Tone::Neutral,
                )),
        )
        .when(is_loading, |element| {
            element.child(render_empty_panel_card("Loading check runs..."))
        })
        .when_some(error.map(str::to_string), |element, error| {
            element.child(render_error_panel_card(error))
        })
        .when(
            !is_loading && error.is_none() && check_runs.is_empty(),
            |element| {
                element.child(render_empty_panel_card(
                    "No check runs found for this PR head",
                ))
            },
        )
        .when(!check_runs.is_empty(), |element| {
            element.child(
                list(
                    list_state,
                    cx.processor(|view, index: usize, _window, _cx| {
                        let Some(check_run) = view.detail_state.check_runs().get(index) else {
                            return div().into_any_element();
                        };

                        render_check_run(check_run).into_any_element()
                    }),
                )
                .flex_1()
                .min_h_0()
                .w_full()
                .min_w_0(),
            )
        })
}

pub(crate) fn render_check_run(check_run: &CheckRun) -> impl IntoElement {
    render_panel_card()
        .flex()
        .items_center()
        .justify_between()
        .gap_3()
        .overflow_hidden()
        .px_3()
        .py_2()
        .child(
            div()
                .min_w_0()
                .flex_1()
                .flex()
                .flex_col()
                .gap_1()
                .child(div().min_w_0().truncate().child(check_run.name.clone()))
                .child(
                    div()
                        .min_w_0()
                        .truncate()
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
    let (label, tone) = check_conclusion_label(conclusion, status);

    render_status_pill(label, tone)
}

fn check_conclusion_label(
    conclusion: Option<CheckConclusion>,
    status: CheckStatus,
) -> (&'static str, Tone) {
    match (status, conclusion) {
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
    }
}
