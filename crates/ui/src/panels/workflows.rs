use gpui::{Context, IntoElement, ListState, div, prelude::*};
use gpui_component::{
    Disableable, Sizable, StyledExt,
    button::{Button, ButtonVariants},
};
use harbor_domain::{PullRequest, RepoId, Workflow, WorkflowRun};

use crate::actions::WorkflowAction;
use crate::icons::Octicon;
use crate::visual::color;
use crate::workspace::AppView;

use super::{
    render_empty_panel_card, render_error_panel_card, render_key_hint, render_panel_card,
    render_panel_header, sync_virtual_list_item_count,
};

#[path = "workflows/runs.rs"]
mod runs;
#[path = "workflows/sidebar.rs"]
mod sidebar;

use runs::{RepositoryWorkflowRunsRenderInput, render_repository_workflow_runs};
pub(crate) use runs::{
    render_workflow_conclusion, workflow_conclusion_tone, workflow_run_failed, workflow_run_label,
};
use sidebar::render_workflow_sidebar;

pub(crate) struct ActionsPanelRenderInput<'a> {
    pub(crate) repository: Option<&'a RepoId>,
    pub(crate) pr: Option<&'a PullRequest>,
    pub(crate) repository_workflows: &'a [Workflow],
    pub(crate) selected_repository_workflow_id: Option<u64>,
    pub(crate) repository_workflow_runs: &'a [WorkflowRun],
    pub(crate) repository_workflow_run_total_count: Option<usize>,
    pub(crate) repository_workflow_runs_has_next_page: bool,
    pub(crate) repository_workflows_loading: bool,
    pub(crate) repository_runs_loading: bool,
    pub(crate) repository_runs_loading_more: bool,
    pub(crate) repository_workflows_error: Option<&'a str>,
    pub(crate) repository_runs_error: Option<&'a str>,
    pub(crate) repository_runs_load_more_error: Option<&'a str>,
    pub(crate) selected_pr_workflow_runs: &'a [WorkflowRun],
    pub(crate) selected_pr_workflows_loading: bool,
    pub(crate) selected_pr_workflows_error: Option<&'a str>,
    pub(crate) action_error: Option<&'a str>,
    pub(crate) is_running_action: bool,
    pub(crate) workflow_list_state: ListState,
    pub(crate) run_list_state: ListState,
}

pub(crate) fn render_actions_panel(
    input: ActionsPanelRenderInput<'_>,
    cx: &mut Context<AppView>,
) -> impl IntoElement {
    let ActionsPanelRenderInput {
        repository,
        pr,
        repository_workflows,
        selected_repository_workflow_id,
        repository_workflow_runs,
        repository_workflow_run_total_count,
        repository_workflow_runs_has_next_page,
        repository_workflows_loading,
        repository_runs_loading,
        repository_runs_loading_more,
        repository_workflows_error,
        repository_runs_error,
        repository_runs_load_more_error,
        selected_pr_workflow_runs,
        selected_pr_workflows_loading,
        selected_pr_workflows_error,
        action_error,
        is_running_action,
        workflow_list_state,
        run_list_state,
    } = input;

    sync_virtual_list_item_count(&workflow_list_state, repository_workflows.len() + 1);
    sync_virtual_list_item_count(&run_list_state, repository_workflow_runs.len());

    let metadata = Some(if repository.is_some() {
        format!(
            "{} workflows, {} runs",
            repository_workflows.len(),
            repository_workflow_runs.len()
        )
    } else {
        "No repository selected".to_string()
    });

    div()
        .id("actions-panel")
        .flex()
        .flex_col()
        .flex_1()
        .min_h_0()
        .gap_2()
        .child(render_panel_header("Actions", metadata))
        .when(repository.is_none(), |element| {
            element.child(render_empty_panel_card(
                "Select a repository before loading Actions",
            ))
        })
        .when_some(repository, |element, repository| {
            element
                .child(render_selected_pull_request_workflow_actions(
                    pr,
                    selected_pr_workflow_runs,
                    selected_pr_workflows_loading,
                    selected_pr_workflows_error,
                    action_error,
                    is_running_action,
                    cx,
                ))
                .child(
                    div()
                        .flex()
                        .flex_1()
                        .min_h_0()
                        .min_w_0()
                        .gap_2()
                        .child(render_workflow_sidebar(
                            repository_workflows,
                            repository_workflows_loading,
                            repository_workflows_error,
                            workflow_list_state,
                            cx,
                        ))
                        .child(render_repository_workflow_runs(
                            RepositoryWorkflowRunsRenderInput {
                                repository,
                                workflows: repository_workflows,
                                selected_workflow_id: selected_repository_workflow_id,
                                workflow_runs: repository_workflow_runs,
                                total_count: repository_workflow_run_total_count,
                                has_next_page: repository_workflow_runs_has_next_page,
                                is_loading: repository_runs_loading,
                                is_loading_more: repository_runs_loading_more,
                                error: repository_runs_error,
                                load_more_error: repository_runs_load_more_error,
                                list_state: run_list_state,
                            },
                            cx,
                        )),
                )
        })
}

fn render_selected_pull_request_workflow_actions(
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
    let can_rerun = pr.is_some()
        && !is_loading
        && error.is_none()
        && rerun_target.is_some()
        && !is_running_action;
    let can_dispatch = pr.is_some()
        && !is_loading
        && error.is_none()
        && dispatch_target.is_some()
        && !is_running_action;

    render_panel_card()
        .px_3()
        .py_2()
        .flex()
        .flex_col()
        .gap_2()
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
                        .flex()
                        .flex_col()
                        .gap_1()
                        .child(
                            div()
                                .font_medium()
                                .truncate()
                                .child(selected_pull_request_actions_title(pr)),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(color::text_muted())
                                .truncate()
                                .child(selected_pull_request_actions_subtitle(
                                    pr,
                                    dispatch_target,
                                    rerun_target,
                                )),
                        ),
                )
                .child(
                    div()
                        .flex_none()
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
                ),
        )
        .when(is_loading, |element| {
            element.child(
                div()
                    .text_xs()
                    .text_color(color::text_muted())
                    .child("Loading selected PR workflow runs..."),
            )
        })
        .when_some(error.map(str::to_string), |element, error| {
            element.child(render_error_panel_card(error))
        })
        .when_some(action_error.map(str::to_string), |element, error| {
            element.child(render_error_panel_card(error))
        })
}

fn selected_pull_request_actions_title(pr: Option<&PullRequest>) -> String {
    pr.map(|pr| format!("Selected PR #{} workflow actions", pr.number))
        .unwrap_or_else(|| "Selected PR workflow actions".to_string())
}

fn selected_pull_request_actions_subtitle(
    pr: Option<&PullRequest>,
    dispatch_target: Option<&WorkflowRun>,
    rerun_target: Option<&WorkflowRun>,
) -> String {
    let Some(pr) = pr else {
        return "Select a pull request to dispatch or rerun its workflows".to_string();
    };
    let dispatch_target = dispatch_target
        .map(workflow_run_label)
        .unwrap_or_else(|| "none".to_string());
    let rerun_target = rerun_target
        .map(workflow_run_label)
        .unwrap_or_else(|| "none".to_string());

    format!(
        "dispatch {dispatch_target} on {}; rerun target {rerun_target}",
        pr.head_ref
    )
}
