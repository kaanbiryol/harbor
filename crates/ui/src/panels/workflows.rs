use chrono::Duration;
use gpui::{Context, IntoElement, ListState, div, list, prelude::*, px};
use gpui_component::{
    Disableable, Icon, Sizable, StyledExt,
    button::{Button, ButtonVariants},
};
use harbor_domain::{
    PullRequest, RepoId, Workflow, WorkflowConclusion, WorkflowRun, WorkflowState, WorkflowStatus,
};

use crate::actions::WorkflowAction;
use crate::date_time::{full_time_label, natural_time_label};
use crate::icons::Octicon;
use crate::visual::{Tone, color, tone_colors};
use crate::workspace::AppView;

use super::{
    render_empty_panel_card, render_error_panel_card, render_key_hint, render_panel_card,
    render_panel_header, render_status_pill, sync_virtual_list_item_count,
};

pub(crate) struct ActionsPanelRenderInput<'a> {
    pub(crate) repository: Option<&'a RepoId>,
    pub(crate) pr: Option<&'a PullRequest>,
    pub(crate) repository_workflows: &'a [Workflow],
    pub(crate) selected_repository_workflow_id: Option<u64>,
    pub(crate) repository_workflow_runs: &'a [WorkflowRun],
    pub(crate) repository_workflows_loading: bool,
    pub(crate) repository_runs_loading: bool,
    pub(crate) repository_workflows_error: Option<&'a str>,
    pub(crate) repository_runs_error: Option<&'a str>,
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
        repository_workflows_loading,
        repository_runs_loading,
        repository_workflows_error,
        repository_runs_error,
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
                                is_loading: repository_runs_loading,
                                error: repository_runs_error,
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

fn render_workflow_sidebar(
    workflows: &[Workflow],
    is_loading: bool,
    error: Option<&str>,
    list_state: ListState,
    cx: &mut Context<AppView>,
) -> impl IntoElement {
    render_panel_card()
        .flex_none()
        .w(px(236.0))
        .flex()
        .flex_col()
        .min_h_0()
        .overflow_hidden()
        .child(
            div()
                .px_3()
                .py_2()
                .border_b_1()
                .border_color(color::border())
                .font_medium()
                .child("Workflows"),
        )
        .when(is_loading, |element| {
            element.child(
                div()
                    .px_3()
                    .py_2()
                    .text_xs()
                    .text_color(color::text_muted())
                    .child("Loading workflows..."),
            )
        })
        .when_some(error.map(str::to_string), |element, error| {
            element.child(div().mx_2().my_2().child(render_error_panel_card(error)))
        })
        .when(
            !is_loading && error.is_none() && workflows.is_empty(),
            |element| {
                element.child(
                    div()
                        .px_3()
                        .py_2()
                        .text_xs()
                        .text_color(color::text_muted())
                        .child("No workflows found"),
                )
            },
        )
        .child(
            list(
                list_state,
                cx.processor(|view, index: usize, _window, cx| {
                    let selected_workflow_id = view.repository_actions_state.selected_workflow_id();

                    if index == 0 {
                        return render_workflow_filter_row(
                            None,
                            "All workflows".to_string(),
                            "Repository run history".to_string(),
                            selected_workflow_id.is_none(),
                            cx,
                        )
                        .into_any_element();
                    }

                    let workflow_index = index.saturating_sub(1);
                    let Some(workflow) = view
                        .repository_actions_state
                        .workflows()
                        .get(workflow_index)
                    else {
                        return div().into_any_element();
                    };

                    render_workflow_filter_row(
                        Some(workflow.id),
                        workflow.name.clone(),
                        workflow_sidebar_subtitle(workflow),
                        selected_workflow_id == Some(workflow.id),
                        cx,
                    )
                    .into_any_element()
                }),
            )
            .flex_1()
            .min_h_0()
            .w_full()
            .min_w_0(),
        )
}

fn render_workflow_filter_row(
    workflow_id: Option<u64>,
    title: String,
    subtitle: String,
    active: bool,
    cx: &mut Context<AppView>,
) -> impl IntoElement {
    let text_color = if active {
        color::text_primary()
    } else {
        color::text_secondary()
    };

    div()
        .id(("workflow-filter", workflow_id.unwrap_or(0)))
        .mx_2()
        .my_1()
        .rounded_xs()
        .px_2()
        .py_2()
        .cursor_pointer()
        .bg(if active {
            color::row_selected()
        } else {
            color::content_background()
        })
        .hover(move |style| {
            style.bg(if active {
                color::row_selected_active()
            } else {
                color::row_hover()
            })
        })
        .on_click(cx.listener(move |view, _, _, cx| {
            view.select_repository_actions_workflow(workflow_id, cx);
        }))
        .flex()
        .items_start()
        .gap_2()
        .child(Icon::new(Octicon::Gear).xsmall().text_color(if active {
            color::accent()
        } else {
            color::text_muted()
        }))
        .child(
            div()
                .min_w_0()
                .flex_1()
                .flex()
                .flex_col()
                .gap_1()
                .child(
                    div()
                        .min_w_0()
                        .truncate()
                        .font_medium()
                        .text_color(text_color)
                        .child(title),
                )
                .child(
                    div()
                        .min_w_0()
                        .truncate()
                        .text_xs()
                        .text_color(color::text_muted())
                        .child(subtitle),
                ),
        )
}

fn workflow_sidebar_subtitle(workflow: &Workflow) -> String {
    format!(
        "{}  {}",
        workflow_state_label(&workflow.state),
        workflow.path
    )
}

fn workflow_state_label(state: &WorkflowState) -> String {
    match state {
        WorkflowState::Active => "active".to_string(),
        WorkflowState::DisabledManually => "disabled".to_string(),
        WorkflowState::DisabledInactivity => "inactive".to_string(),
        WorkflowState::DisabledFork => "fork disabled".to_string(),
        WorkflowState::Deleted => "deleted".to_string(),
        WorkflowState::Unknown(state) => state.clone(),
    }
}

struct RepositoryWorkflowRunsRenderInput<'a> {
    repository: &'a RepoId,
    workflows: &'a [Workflow],
    selected_workflow_id: Option<u64>,
    workflow_runs: &'a [WorkflowRun],
    is_loading: bool,
    error: Option<&'a str>,
    list_state: ListState,
}

fn render_repository_workflow_runs(
    input: RepositoryWorkflowRunsRenderInput<'_>,
    cx: &mut Context<AppView>,
) -> impl IntoElement {
    let RepositoryWorkflowRunsRenderInput {
        repository,
        workflows,
        selected_workflow_id,
        workflow_runs,
        is_loading,
        error,
        list_state,
    } = input;
    let selected_workflow = selected_workflow_id
        .and_then(|workflow_id| workflows.iter().find(|workflow| workflow.id == workflow_id));
    let title = selected_workflow
        .map(|workflow| workflow.name.clone())
        .unwrap_or_else(|| "All workflow runs".to_string());
    let subtitle = selected_workflow
        .map(|workflow| format!("{} in {}", workflow.path, repository.full_name()))
        .unwrap_or_else(|| {
            format!(
                "Showing runs from all workflows in {}",
                repository.full_name()
            )
        });

    div()
        .flex()
        .flex_col()
        .flex_1()
        .min_h_0()
        .min_w_0()
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
                        .child(div().font_medium().truncate().child(title))
                        .child(
                            div()
                                .text_xs()
                                .text_color(color::text_muted())
                                .truncate()
                                .child(subtitle),
                        ),
                )
                .child(render_status_pill(
                    format!("{} runs", workflow_runs.len()),
                    Tone::Neutral,
                )),
        )
        .when(is_loading, |element| {
            element.child(render_empty_panel_card("Loading workflow runs..."))
        })
        .when_some(error.map(str::to_string), |element, error| {
            element.child(render_error_panel_card(error))
        })
        .when(
            !is_loading && error.is_none() && workflow_runs.is_empty(),
            |element| element.child(render_empty_panel_card("No workflow runs found")),
        )
        .when(!workflow_runs.is_empty(), |element| {
            element.child(
                render_panel_card()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_h_0()
                    .overflow_hidden()
                    .child(
                        list(
                            list_state,
                            cx.processor(|view, index: usize, _window, _cx| {
                                let Some(run) =
                                    view.repository_actions_state.workflow_runs().get(index)
                                else {
                                    return div().into_any_element();
                                };

                                render_repository_workflow_run(run).into_any_element()
                            }),
                        )
                        .flex_1()
                        .min_h_0()
                        .w_full()
                        .min_w_0(),
                    ),
            )
        })
}

fn render_repository_workflow_run(run: &WorkflowRun) -> impl IntoElement {
    let (status_label, tone) = workflow_conclusion_tone(run.conclusion, run.status);
    let colors = tone_colors(tone);

    div()
        .w_full()
        .min_w_0()
        .border_b_1()
        .border_color(color::border())
        .px_3()
        .py_3()
        .flex()
        .items_center()
        .gap_3()
        .child(
            Icon::new(workflow_status_icon(run.conclusion, run.status))
                .small()
                .text_color(colors.text),
        )
        .child(
            div()
                .min_w_0()
                .flex_1()
                .flex()
                .flex_col()
                .gap_1()
                .child(
                    div()
                        .min_w_0()
                        .truncate()
                        .font_medium()
                        .text_color(color::text_primary())
                        .child(run.name.clone()),
                )
                .child(
                    div()
                        .min_w_0()
                        .truncate()
                        .text_xs()
                        .text_color(color::text_muted())
                        .child(repository_run_metadata(run)),
                ),
        )
        .child(
            div()
                .flex_none()
                .flex()
                .items_center()
                .gap_2()
                .child(render_branch_label(run))
                .child(render_status_pill(status_label, tone)),
        )
        .child(
            div()
                .flex_none()
                .flex()
                .flex_col()
                .items_end()
                .gap_1()
                .text_xs()
                .text_color(color::text_muted())
                .child(natural_time_label(run.created_at))
                .child(run_duration_label(run)),
        )
}

fn render_branch_label(run: &WorkflowRun) -> impl IntoElement {
    div()
        .max_w(px(220.0))
        .truncate()
        .rounded_xs()
        .border_1()
        .border_color(color::border())
        .bg(color::input_background())
        .px_1()
        .py_0p5()
        .text_xs()
        .font_medium()
        .text_color(color::accent())
        .child(if run.head_branch.is_empty() {
            "unknown branch".to_string()
        } else {
            run.head_branch.clone()
        })
}

fn repository_run_metadata(run: &WorkflowRun) -> String {
    let workflow = run.workflow_name.as_deref().unwrap_or("workflow");
    let number = run
        .run_number
        .map(|number| format!(" #{number}"))
        .unwrap_or_default();
    let actor = run
        .actor_login
        .as_ref()
        .map(|actor| format!(" by {actor}"))
        .unwrap_or_default();
    let attempt = run
        .run_attempt
        .filter(|attempt| *attempt > 1)
        .map(|attempt| format!(" attempt {attempt}"))
        .unwrap_or_default();

    format!("{workflow}{number}: {}{actor}{attempt}", run.event)
}

fn run_duration_label(run: &WorkflowRun) -> String {
    let start = run.run_started_at.unwrap_or(run.created_at);
    let duration = run.updated_at.signed_duration_since(start);

    if duration < Duration::zero() {
        return full_time_label(run.created_at);
    }

    short_duration_label(duration)
}

fn short_duration_label(duration: Duration) -> String {
    if duration.num_hours() > 0 {
        format!("{}h {}m", duration.num_hours(), duration.num_minutes() % 60)
    } else if duration.num_minutes() > 0 {
        format!(
            "{}m {}s",
            duration.num_minutes(),
            duration.num_seconds() % 60
        )
    } else {
        format!("{}s", duration.num_seconds().max(0))
    }
}

fn workflow_status_icon(conclusion: Option<WorkflowConclusion>, status: WorkflowStatus) -> Octicon {
    match (status, conclusion) {
        (WorkflowStatus::Completed, Some(WorkflowConclusion::Success)) => Octicon::CheckCircle,
        (WorkflowStatus::Completed, Some(WorkflowConclusion::Failure)) => Octicon::XCircle,
        (WorkflowStatus::Completed, Some(WorkflowConclusion::TimedOut)) => Octicon::Alert,
        (WorkflowStatus::Completed, Some(WorkflowConclusion::ActionRequired)) => Octicon::Alert,
        (WorkflowStatus::Completed, Some(WorkflowConclusion::Cancelled)) => Octicon::XCircle,
        (WorkflowStatus::Completed, Some(WorkflowConclusion::Skipped)) => Octicon::Clock,
        (WorkflowStatus::Completed, None) => Octicon::XCircle,
        (WorkflowStatus::InProgress, _) => Octicon::Sync,
        (WorkflowStatus::Queued, _) => Octicon::Clock,
    }
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
    match (status, conclusion) {
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
    }
}

#[cfg(test)]
mod tests {
    use chrono::{Duration, TimeZone, Utc};
    use harbor_domain::{WorkflowConclusion, WorkflowRun, WorkflowStatus};

    use super::{repository_run_metadata, run_duration_label};

    fn run() -> WorkflowRun {
        let created_at = Utc
            .with_ymd_and_hms(2026, 7, 8, 10, 0, 0)
            .single()
            .expect("valid test time");
        WorkflowRun {
            id: 42,
            workflow_id: Some(9),
            name: "build".to_string(),
            workflow_name: Some("CI".to_string()),
            status: WorkflowStatus::Completed,
            conclusion: Some(WorkflowConclusion::Success),
            head_branch: "main".to_string(),
            head_sha: "abc123".to_string(),
            event: "push".to_string(),
            url: "https://api.github.com/repos/acme/app/actions/runs/42".to_string(),
            html_url: "https://github.com/acme/app/actions/runs/42".to_string(),
            created_at,
            updated_at: created_at + Duration::seconds(75),
            run_number: Some(12),
            run_attempt: Some(2),
            actor_login: Some("octocat".to_string()),
            run_started_at: Some(created_at + Duration::seconds(15)),
        }
    }

    #[test]
    fn formats_repository_run_metadata() {
        assert_eq!(
            repository_run_metadata(&run()),
            "CI #12: push by octocat attempt 2"
        );
    }

    #[test]
    fn formats_run_duration_from_start_time() {
        assert_eq!(run_duration_label(&run()), "1m 0s");
    }
}
