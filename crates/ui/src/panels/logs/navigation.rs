use crate::gpui_component::{
    Icon, Sizable, StyledExt,
    list::ListItem,
    scroll::{Scrollbar, ScrollbarMode},
};
use crate::panels::workflows::workflow_conclusion_tone;
use crate::{
    icons::Octicon,
    visual::{Tone, color, tone_text},
    workflow_logs::WorkflowLogView,
};
use gpui::{AnyElement, Context, IntoElement, div, prelude::*, px, uniform_list};
use harbor_domain::WorkflowRun;

pub(super) fn render_navigation_row(
    row: &LogNavigationRow,
    selected_run: Option<u64>,
    selected: Option<&(String, Option<String>)>,
    cx: &mut Context<WorkflowLogView>,
) -> AnyElement {
    let section = row.job.as_ref().map(|job| (job.clone(), row.step.clone()));
    let run_id = row.run_id;
    let label = row.label.clone();
    div()
        .h(px(52.))
        .py_0p5()
        .child(
            ListItem::new(gpui::SharedString::from(format!("log-section-{}", row.id)))
                .selected(
                    selected_run == Some(run_id)
                        && row.job.is_some()
                        && selected == section.as_ref(),
                )
                .h_full()
                .px_2()
                .tooltip(move |window, cx| {
                    crate::gpui_component::tooltip::Tooltip::new(label.clone()).build(window, cx)
                })
                .child(
                    div()
                        .flex()
                        .items_start()
                        .gap_2()
                        .min_w_0()
                        .child(
                            div()
                                .flex_none()
                                .w_3p5()
                                .h(px(20.))
                                .flex()
                                .items_center()
                                .justify_center()
                                .child(Icon::new(row.icon).small().text_color(tone_text(row.tone))),
                        )
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
                                        .line_height(px(20.))
                                        .when(row.job.is_none(), |element| element.font_semibold())
                                        .truncate()
                                        .text_color(color::text_primary())
                                        .child(row.label.clone()),
                                )
                                .child(
                                    div()
                                        .text_xs()
                                        .line_height(px(16.))
                                        .truncate()
                                        .text_color(tone_text(row.tone))
                                        .child(row.status.clone()),
                                ),
                        ),
                )
                .on_click(cx.listener(move |view, _, _, cx| {
                    view.select_workflow_log_section(run_id, section.clone(), cx);
                })),
        )
        .into_any_element()
}

pub(super) struct LogNavigationRow {
    id: String,
    run_id: u64,
    job: Option<String>,
    step: Option<String>,
    label: String,
    status: String,
    tone: Tone,
    icon: Octicon,
}

pub(super) fn navigation_rows(
    runs: &[WorkflowRun],
    state: &crate::workflow_logs::WorkflowLogState,
    selected_run: Option<u64>,
) -> Vec<LogNavigationRow> {
    let mut rows = Vec::new();
    for run in runs {
        let (status, tone) = workflow_conclusion_tone(run.conclusion, run.status);
        let jobs = state.navigation_jobs.get(&run.id);
        rows.push(LogNavigationRow {
            id: format!("run-{}", run.id),
            run_id: run.id,
            job: None,
            step: None,
            label: run.workflow_name.as_ref().unwrap_or(&run.name).clone(),
            status: match jobs {
                Some(Ok(jobs)) => format!("{} checks · {status}", jobs.len()),
                Some(Err(error)) => format!("Checks unavailable: {error}"),
                None => "Loading checks…".to_string(),
            },
            tone: if jobs.is_some_and(Result::is_err) {
                Tone::Danger
            } else {
                tone
            },
            icon: status_icon(run.conclusion, run.status),
        });
        let Some(Ok(jobs)) = jobs else {
            continue;
        };
        for job in jobs.iter() {
            let (status, tone) = workflow_conclusion_tone(job.conclusion, job.status);
            rows.push(LogNavigationRow {
                id: format!("{}-{}", run.id, job.id),
                run_id: run.id,
                job: Some(job.name.clone()),
                step: None,
                label: job.name.clone(),
                status: status.into(),
                tone,
                icon: status_icon(job.conclusion, job.status),
            });
            if selected_run != Some(run.id)
                || state
                    .selected_section
                    .as_ref()
                    .is_none_or(|(name, _)| *name != job.name)
            {
                continue;
            }
            for step in &job.steps {
                let (status, tone) = workflow_conclusion_tone(step.conclusion, step.status);
                let duration = step
                    .started_at
                    .zip(step.completed_at)
                    .map(|(start, end)| format!(" · {}s", (end - start).num_seconds().max(0)))
                    .unwrap_or_default();
                rows.push(LogNavigationRow {
                    id: format!("{}-{}-{}", run.id, job.id, step.number),
                    run_id: run.id,
                    job: Some(job.name.clone()),
                    step: Some(step.name.clone()),
                    label: step.name.clone(),
                    status: format!("{status}{duration}"),
                    tone,
                    icon: status_icon(step.conclusion, step.status),
                });
            }
        }
    }
    rows
}

fn status_icon(
    conclusion: Option<harbor_domain::WorkflowConclusion>,
    status: harbor_domain::WorkflowStatus,
) -> Octicon {
    use harbor_domain::{WorkflowConclusion, WorkflowStatus};
    match conclusion {
        Some(WorkflowConclusion::Success) => Octicon::CheckCircle,
        Some(WorkflowConclusion::Failure | WorkflowConclusion::TimedOut) => Octicon::XCircle,
        Some(WorkflowConclusion::Cancelled) => Octicon::X,
        Some(WorkflowConclusion::ActionRequired) => Octicon::Alert,
        _ if status == WorkflowStatus::InProgress => Octicon::Sync,
        _ => Octicon::Clock,
    }
}

pub(super) fn render_log_navigation(
    view: &WorkflowLogView,
    cx: &mut Context<WorkflowLogView>,
) -> impl IntoElement {
    let state = &view.state;
    let selected_run = view.selected_workflow_run_for_logs().map(|run| run.id);
    let selected = state.selected_section.clone();
    let navigation = navigation_rows(&view.runs, state, selected_run);
    let navigation_count = navigation.len();
    div()
        .w(px(260.))
        .flex_none()
        .flex()
        .flex_col()
        .min_h_0()
        .border_r_1()
        .border_color(color::border_subtle())
        .pr_2()
        .pt_2()
        .child(
            ListItem::new("all-log-output")
                .selected(selected.is_none())
                .h(px(36.))
                .px_2()
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_2()
                        .child(
                            Icon::new(Octicon::Terminal)
                                .small()
                                .text_color(color::text_muted()),
                        )
                        .child(div().text_sm().child("All output")),
                )
                .on_click(cx.listener(|view, _, _, cx| {
                    view.state.selected_section = None;
                    view.filter_workflow_logs(cx);
                })),
        )
        .child(
            div()
                .px_2()
                .pt_3()
                .pb_1()
                .text_xs()
                .text_color(color::text_muted())
                .child("WORKFLOWS / CHECKS"),
        )
        .child(
            div()
                .relative()
                .flex_1()
                .min_h_0()
                .overflow_hidden()
                .child(
                    uniform_list(
                        "log-job-navigation",
                        navigation_count,
                        cx.processor(move |_view, range: std::ops::Range<usize>, _, cx| {
                            range
                                .filter_map(|index| {
                                    navigation.get(index).map(|row| {
                                        render_navigation_row(
                                            row,
                                            selected_run,
                                            selected.as_ref(),
                                            cx,
                                        )
                                    })
                                })
                                .collect::<Vec<_>>()
                        }),
                    )
                    .track_scroll(&state.navigation_scroll)
                    .flex_1()
                    .min_h_0()
                    .size_full(),
                )
                .child(Scrollbar::vertical(&state.navigation_scroll).mode(ScrollbarMode::Always)),
        )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workflow_logs::WorkflowLogState;
    use harbor_domain::{WorkflowJob, WorkflowStatus, WorkflowStep};
    use std::sync::Arc;

    #[test]
    fn navigation_includes_checks_from_every_workflow_and_expands_only_the_selected_job() {
        let first = crate::test_fixtures::workflow_run();
        let mut second = first.clone();
        second.id = 84;
        let mut state = WorkflowLogState::new();
        let job = |name: &str| WorkflowJob {
            id: 1,
            name: name.into(),
            status: WorkflowStatus::Completed,
            conclusion: None,
            steps: vec![WorkflowStep {
                number: 1,
                name: "Run tests".into(),
                status: WorkflowStatus::Completed,
                conclusion: None,
                started_at: None,
                completed_at: None,
            }],
        };
        state
            .navigation_jobs
            .insert(first.id, Ok(Arc::new(vec![job("PR Title")])));
        state
            .navigation_jobs
            .insert(second.id, Ok(Arc::new(vec![job("Android")])));
        state.selected_section = Some(("Android".into(), None));
        let rows = navigation_rows(&[first, second], &state, Some(84));
        assert_eq!(
            rows.iter()
                .filter(|row| row.job.is_some())
                .map(|row| row.label.as_str())
                .collect::<Vec<_>>(),
            ["PR Title", "Android", "Run tests"]
        );
        let ids = rows
            .iter()
            .map(|row| &row.id)
            .collect::<std::collections::HashSet<_>>();
        assert_eq!(ids.len(), rows.len());
        assert!(
            rows.iter()
                .filter(|row| row.step.is_some())
                .all(|row| row.run_id == 84)
        );
    }
}
