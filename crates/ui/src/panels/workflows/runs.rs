use chrono::Duration;
use gpui::{Context, IntoElement, ListState, div, list, prelude::*, px};
use gpui_component::{Icon, Sizable, StyledExt};
use harbor_domain::{RepoId, Workflow, WorkflowConclusion, WorkflowRun, WorkflowStatus};

use crate::date_time::{full_time_label, natural_time_label, short_duration_label};
use crate::icons::Octicon;
use crate::visual::{Tone, color, tone_colors};
use crate::workspace::AppView;

use super::super::{
    render_empty_panel_card, render_error_panel_card, render_panel_card, render_status_pill,
};

pub(super) struct RepositoryWorkflowRunsRenderInput<'a> {
    pub(super) repository: &'a RepoId,
    pub(super) workflows: &'a [Workflow],
    pub(super) selected_workflow_id: Option<u64>,
    pub(super) workflow_runs: &'a [WorkflowRun],
    pub(super) is_loading: bool,
    pub(super) error: Option<&'a str>,
    pub(super) list_state: ListState,
}

pub(super) fn render_repository_workflow_runs(
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
        assert_eq!(run_duration_label(&run()), "1m");
    }
}
