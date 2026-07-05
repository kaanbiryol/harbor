use gpui::{Context, IntoElement, ListState, div, list, prelude::*};
use gpui_component::{
    Disableable, Sizable,
    button::{Button, ButtonVariants},
};
use harbor_domain::{PullRequest, WorkflowConclusion, WorkflowRun, WorkflowStatus};

use crate::actions::WorkflowAction;
use crate::icons::Octicon;
use crate::visual::{Tone, color};
use crate::workspace::AppView;

use super::{
    render_empty_panel_card, render_error_panel_card, render_key_hint, render_panel_card,
    render_panel_header, render_status_pill, sync_virtual_list_item_count,
};

pub(crate) struct ActionsPanelRenderInput<'a> {
    pub(crate) pr: Option<&'a PullRequest>,
    pub(crate) workflow_runs: &'a [WorkflowRun],
    pub(crate) is_loading: bool,
    pub(crate) error: Option<&'a str>,
    pub(crate) action_error: Option<&'a str>,
    pub(crate) is_running_action: bool,
    pub(crate) list_state: ListState,
}

pub(crate) fn render_actions_panel(
    input: ActionsPanelRenderInput<'_>,
    cx: &mut Context<AppView>,
) -> impl IntoElement {
    let ActionsPanelRenderInput {
        pr,
        workflow_runs,
        is_loading,
        error,
        action_error,
        is_running_action,
        list_state,
    } = input;
    let rerun_target = workflow_runs
        .iter()
        .find(|run| workflow_run_failed(run))
        .or_else(|| workflow_runs.first());
    let dispatch_target = workflow_runs.iter().find(|run| run.workflow_id.is_some());
    let can_rerun = !is_loading && error.is_none() && rerun_target.is_some() && !is_running_action;
    let can_dispatch =
        !is_loading && error.is_none() && dispatch_target.is_some() && !is_running_action;
    sync_virtual_list_item_count(&list_state, workflow_runs.len());

    div()
        .id("actions-panel")
        .flex()
        .flex_col()
        .flex_1()
        .min_h_0()
        .gap_2()
        .child(render_panel_header(
            "Workflow runs",
            Some(format!("{} runs", workflow_runs.len())),
        ))
        .child(
            div()
                .flex()
                .items_center()
                .gap_2()
                .child(
                    Button::new("trigger-build")
                        .icon(Octicon::Sync)
                        .label("trigger build")
                        .small()
                        .primary()
                        .loading(is_running_action)
                        .disabled(!can_dispatch)
                        .on_click(cx.listener(|view, _, _, cx| {
                            view.run_workflow_action(WorkflowAction::DispatchBuild, cx);
                        })),
                )
                .child(
                    Button::new("rerun-failed-jobs")
                        .icon(Octicon::Sync)
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
                        .flex()
                        .items_center()
                        .gap_1()
                        .text_xs()
                        .text_color(color::text_muted())
                        .child(render_key_hint("b"))
                        .child(div().child("/"))
                        .child(render_key_hint("shift+r")),
                ),
        )
        .child(render_workflow_action_targets(
            pr,
            dispatch_target,
            rerun_target,
        ))
        .when_some(action_error.map(str::to_string), |element, error| {
            element.child(render_error_panel_card(error))
        })
        .when(is_loading, |element| {
            element.child(render_empty_panel_card("Loading workflow runs..."))
        })
        .when_some(error.map(str::to_string), |element, error| {
            element.child(render_error_panel_card(error))
        })
        .when(
            !is_loading && error.is_none() && workflow_runs.is_empty(),
            |element| {
                element.child(render_empty_panel_card(
                    "No workflow runs found for this PR head",
                ))
            },
        )
        .when(!workflow_runs.is_empty(), |element| {
            element.child(
                list(
                    list_state,
                    cx.processor(|view, index: usize, _window, _cx| {
                        let Some(run) = view.detail_state.workflow_runs().get(index) else {
                            return div().into_any_element();
                        };

                        render_workflow_run(run).into_any_element()
                    }),
                )
                .flex_1()
                .min_h_0()
                .w_full()
                .min_w_0(),
            )
        })
}

fn render_workflow_action_targets(
    pr: Option<&PullRequest>,
    dispatch_target: Option<&WorkflowRun>,
    rerun_target: Option<&WorkflowRun>,
) -> impl IntoElement {
    let dispatch_target_label = format!(
        "dispatch target: {} on {}",
        dispatch_target
            .map(workflow_run_label)
            .unwrap_or_else(|| "none".to_string()),
        pr.map(|pr| pr.head_ref.as_str())
            .unwrap_or("no selected branch")
    );
    let rerun_target_label = format!(
        "rerun target: {}",
        rerun_target
            .map(workflow_run_label)
            .unwrap_or_else(|| "none".to_string())
    );

    render_panel_card()
        .px_3()
        .py_2()
        .flex()
        .flex_col()
        .gap_1()
        .overflow_hidden()
        .text_xs()
        .text_color(color::text_muted())
        .child(div().min_w_0().truncate().child(dispatch_target_label))
        .child(div().min_w_0().truncate().child(rerun_target_label))
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
                .child(div().min_w_0().truncate().child(run.name.clone()))
                .child(
                    div()
                        .min_w_0()
                        .truncate()
                        .text_xs()
                        .text_color(color::text_muted())
                        .child(format!(
                            "{}  {}  {}",
                            run.workflow_name.as_deref().unwrap_or("workflow"),
                            run.event,
                            run.head_branch
                        )),
                ),
        )
        .child(render_workflow_conclusion(run.conclusion, run.status))
}

pub(crate) fn render_workflow_conclusion(
    conclusion: Option<WorkflowConclusion>,
    status: WorkflowStatus,
) -> impl IntoElement {
    let (label, tone) = workflow_conclusion_tone(conclusion, status);

    render_status_pill(label, tone)
}

pub(crate) fn workflow_conclusion_tone(
    conclusion: Option<WorkflowConclusion>,
    status: WorkflowStatus,
) -> (&'static str, Tone) {
    let (label, tone) = match (status, conclusion) {
        (WorkflowStatus::Completed, Some(WorkflowConclusion::Success)) => ("passed", Tone::Success),
        (WorkflowStatus::Completed, Some(WorkflowConclusion::Skipped)) => {
            ("skipped", Tone::Neutral)
        }
        (WorkflowStatus::Completed, Some(WorkflowConclusion::Cancelled)) => {
            ("cancelled", Tone::Warning)
        }
        (WorkflowStatus::Completed, Some(WorkflowConclusion::TimedOut)) => {
            ("timed out", Tone::Danger)
        }
        (WorkflowStatus::Completed, Some(WorkflowConclusion::ActionRequired)) => {
            ("action required", Tone::Warning)
        }
        (WorkflowStatus::Completed, Some(WorkflowConclusion::Failure) | None) => {
            ("failed", Tone::Danger)
        }
        (WorkflowStatus::InProgress, _) => ("running", Tone::Info),
        (WorkflowStatus::Queued, _) => ("queued", Tone::Warning),
    };

    (label, tone)
}
