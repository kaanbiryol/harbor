use gpui::{Context, IntoElement, div, prelude::*, rgb};
use gpui_component::{Disableable, Sizable, button::Button};
use harbor_domain::{PullRequest, WorkflowConclusion, WorkflowRun, WorkflowStatus};

use crate::actions::WorkflowAction;
use crate::workspace::AppView;

pub(crate) fn render_actions_panel(
    pr: Option<&PullRequest>,
    workflow_runs: &[WorkflowRun],
    is_loading: bool,
    error: Option<&str>,
    action_error: Option<&str>,
    is_running_action: bool,
    cx: &mut Context<AppView>,
) -> impl IntoElement {
    let rerun_target = workflow_runs
        .iter()
        .find(|run| workflow_run_failed(run))
        .or_else(|| workflow_runs.first());
    let dispatch_target = workflow_runs.iter().find(|run| run.workflow_id.is_some());
    let can_rerun = !is_loading && error.is_none() && rerun_target.is_some() && !is_running_action;
    let can_dispatch =
        !is_loading && error.is_none() && dispatch_target.is_some() && !is_running_action;

    div()
        .id("actions-panel-scroll")
        .flex()
        .flex_col()
        .flex_1()
        .min_h_0()
        .overflow_y_scroll()
        .gap_2()
        .child("Workflow runs")
        .child(
            div()
                .flex()
                .items_center()
                .gap_2()
                .child(
                    Button::new("trigger-build")
                        .label("trigger build")
                        .small()
                        .outline()
                        .loading(is_running_action)
                        .disabled(!can_dispatch)
                        .on_click(cx.listener(|view, _, _, cx| {
                            view.run_workflow_action(WorkflowAction::DispatchBuild, cx);
                        })),
                )
                .child(
                    Button::new("rerun-failed-jobs")
                        .label("rerun failed")
                        .small()
                        .outline()
                        .loading(is_running_action)
                        .disabled(!can_rerun)
                        .on_click(cx.listener(|view, _, _, cx| {
                            view.run_workflow_action(WorkflowAction::RerunFailedJobs, cx);
                        })),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(rgb(0x9aa4b2))
                        .child("b / shift+r"),
                ),
        )
        .child(div().text_xs().text_color(rgb(0x9aa4b2)).child(format!(
                    "dispatch target: {} on {}",
                    dispatch_target
                        .map(workflow_run_label)
                        .unwrap_or_else(|| "none".to_string()),
                    pr.map(|pr| pr.head_ref.as_str()).unwrap_or("no selected branch")
                )))
        .child(div().text_xs().text_color(rgb(0x9aa4b2)).child(format!(
                    "rerun target: {}",
                    rerun_target
                        .map(workflow_run_label)
                        .unwrap_or_else(|| "none".to_string())
                )))
        .when_some(action_error.map(str::to_string), |element, error| {
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
        .when(is_loading, |element| {
            element.child(
                div()
                    .border_1()
                    .border_color(rgb(0x242a31))
                    .bg(rgb(0x0c0f12))
                    .p_3()
                    .text_color(rgb(0x9aa4b2))
                    .child("Loading workflow runs..."),
            )
        })
        .when_some(error.map(str::to_string), |element, error| {
            element.child(
                div()
                    .border_1()
                    .border_color(rgb(0x242a31))
                    .bg(rgb(0x0c0f12))
                    .p_3()
                    .text_color(rgb(0xf87171))
                    .child(error),
            )
        })
        .when(
            !is_loading && error.is_none() && workflow_runs.is_empty(),
            |element| {
                element.child(
                    div()
                        .border_1()
                        .border_color(rgb(0x242a31))
                        .bg(rgb(0x0c0f12))
                        .p_3()
                        .text_color(rgb(0x9aa4b2))
                        .child("No workflow runs found for this PR head"),
                )
            },
        )
        .children(workflow_runs.iter().map(render_workflow_run))
}

pub(crate) fn workflow_run_label(run: &WorkflowRun) -> String {
    run.workflow_name
        .as_deref()
        .unwrap_or(run.name.as_str())
        .to_string()
}

pub(crate) fn workflow_run_failed(run: &WorkflowRun) -> bool {
    matches!(
        (run.status, run.conclusion),
        (
            WorkflowStatus::Completed,
            Some(
                WorkflowConclusion::Failure
                    | WorkflowConclusion::Cancelled
                    | WorkflowConclusion::TimedOut
                    | WorkflowConclusion::ActionRequired
            )
        )
    )
}

pub(crate) fn render_workflow_run(run: &WorkflowRun) -> impl IntoElement {
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
                .flex()
                .flex_col()
                .gap_1()
                .child(run.name.clone())
                .child(div().text_xs().text_color(rgb(0x9aa4b2)).child(format!(
                    "{}  {}  {}",
                    run.workflow_name.as_deref().unwrap_or("workflow"),
                    run.event,
                    run.head_branch
                ))),
        )
        .child(render_workflow_conclusion(run.conclusion, run.status))
}

pub(crate) fn render_workflow_conclusion(
    conclusion: Option<WorkflowConclusion>,
    status: WorkflowStatus,
) -> impl IntoElement {
    let (label, color) = workflow_conclusion_label(conclusion, status);

    div().text_sm().text_color(color).child(label)
}

pub(crate) fn workflow_conclusion_label(
    conclusion: Option<WorkflowConclusion>,
    status: WorkflowStatus,
) -> (&'static str, gpui::Hsla) {
    let (label, color) = match (status, conclusion) {
        (WorkflowStatus::Completed, Some(WorkflowConclusion::Success)) => ("passed", rgb(0x34d399)),
        (WorkflowStatus::Completed, Some(WorkflowConclusion::Skipped)) => {
            ("skipped", rgb(0x9aa4b2))
        }
        (WorkflowStatus::Completed, Some(WorkflowConclusion::Cancelled)) => {
            ("cancelled", rgb(0xfbbf24))
        }
        (WorkflowStatus::Completed, Some(WorkflowConclusion::TimedOut)) => {
            ("timed out", rgb(0xf87171))
        }
        (WorkflowStatus::Completed, Some(WorkflowConclusion::ActionRequired)) => {
            ("action required", rgb(0xfbbf24))
        }
        (WorkflowStatus::Completed, Some(WorkflowConclusion::Failure) | None) => {
            ("failed", rgb(0xf87171))
        }
        (WorkflowStatus::InProgress, _) => ("running", rgb(0x93c5fd)),
        (WorkflowStatus::Queued, _) => ("queued", rgb(0xfbbf24)),
    };

    (label, color.into())
}
