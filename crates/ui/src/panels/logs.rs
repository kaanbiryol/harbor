use gpui::{
    AnyElement, Context, IntoElement, ListHorizontalSizingBehavior, UniformListScrollHandle, div,
    prelude::*, px, rgb, uniform_list,
};
use gpui_component::{Disableable, Sizable, button::Button};
use harbor_domain::{WorkflowJob, WorkflowRun, WorkflowStep};
use harbor_logs::{LogChunk, LogLine, LogSeverity};

use crate::workspace::AppView;

use super::workflows::{render_workflow_conclusion, workflow_conclusion_label, workflow_run_label};

pub(crate) fn render_logs_panel(
    run: Option<&WorkflowRun>,
    jobs: &[WorkflowJob],
    log_chunk: Option<&LogChunk>,
    is_loading: bool,
    error: Option<&str>,
    scroll_handle: UniformListScrollHandle,
    cx: &mut Context<AppView>,
) -> impl IntoElement {
    let line_count = log_chunk.map_or(0, |chunk| chunk.lines.len());

    div()
        .id("logs-panel")
        .flex()
        .flex_col()
        .flex_1()
        .min_h_0()
        .gap_2()
        .child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .gap_3()
                .child("Logs")
                .child(
                    Button::new("load-workflow-logs")
                        .label("load logs")
                        .small()
                        .outline()
                        .loading(is_loading)
                        .disabled(run.is_none() || is_loading)
                        .on_click(cx.listener(|view, _, _, cx| {
                            view.load_selected_workflow_logs(cx);
                        })),
                ),
        )
        .child(div().text_xs().text_color(rgb(0x9aa4b2)).child(format!(
                    "target: {}",
                    run.map(workflow_run_label)
                        .unwrap_or_else(|| "no workflow run".to_string())
                )))
        .when(is_loading, |element| {
            element.child(
                div()
                    .border_1()
                    .border_color(rgb(0x242a31))
                    .bg(rgb(0x0c0f12))
                    .p_3()
                    .text_color(rgb(0x9aa4b2))
                    .child("Loading workflow jobs and logs..."),
            )
        })
        .when_some(error.map(str::to_string), |element, error| {
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
        .when(!is_loading && run.is_none(), |element| {
            element.child(
                div()
                    .border_1()
                    .border_color(rgb(0x242a31))
                    .bg(rgb(0x0c0f12))
                    .p_3()
                    .text_color(rgb(0x9aa4b2))
                    .child("No workflow run found for this PR head"),
            )
        })
        .when(!jobs.is_empty(), |element| {
            element
                .child(
                    div()
                        .pt_1()
                        .text_xs()
                        .text_color(rgb(0x9aa4b2))
                        .child(format!("jobs {}", jobs.len())),
                )
                .children(jobs.iter().map(render_workflow_job))
        })
        .when(line_count > 0, |element| {
            element.child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_h_0()
                    .min_w_0()
                    .border_1()
                    .border_color(rgb(0x242a31))
                    .bg(rgb(0x0c0f12))
                    .overflow_hidden()
                    .child(
                        uniform_list(
                            "workflow-log-lines",
                            line_count,
                            cx.processor(|view, range: std::ops::Range<usize>, _window, _cx| {
                                let Some(chunk) = view.detail_state.log_state.chunk() else {
                                    return Vec::new();
                                };
                                let mut rows = Vec::with_capacity(range.len());

                                for index in range {
                                    let Some(line) = chunk.lines.get(index) else {
                                        continue;
                                    };
                                    rows.push(render_log_line(line));
                                }

                                rows
                            }),
                        )
                        .track_scroll(&scroll_handle)
                        .with_horizontal_sizing_behavior(
                            ListHorizontalSizingBehavior::Unconstrained,
                        )
                        .flex_1()
                        .min_h_0()
                        .min_w_0()
                        .font_family("Menlo")
                        .text_xs(),
                    ),
            )
        })
        .when(
            !is_loading && run.is_some() && error.is_none() && log_chunk.is_none(),
            |element| {
                element.child(
                    div()
                        .border_1()
                        .border_color(rgb(0x242a31))
                        .bg(rgb(0x0c0f12))
                        .p_3()
                        .text_color(rgb(0x9aa4b2))
                        .child("Press l or load logs to fetch the workflow log output"),
                )
            },
        )
}

pub(crate) fn render_workflow_job(job: &WorkflowJob) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .gap_1()
        .border_1()
        .border_color(rgb(0x242a31))
        .bg(rgb(0x0c0f12))
        .px_3()
        .py_2()
        .child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .gap_3()
                .child(job.name.clone())
                .child(render_workflow_conclusion(job.conclusion, job.status)),
        )
        .children(job.steps.iter().map(render_workflow_step))
}

pub(crate) fn render_workflow_step(step: &WorkflowStep) -> impl IntoElement {
    let (label, color) = workflow_conclusion_label(step.conclusion, step.status);

    div()
        .flex()
        .items_center()
        .justify_between()
        .gap_3()
        .pl_3()
        .text_xs()
        .text_color(rgb(0x9aa4b2))
        .child(format!("{}. {}", step.number, step.name))
        .child(div().text_color(color).child(label))
}

pub(crate) fn render_log_line(line: &LogLine) -> AnyElement {
    let color = match line.severity {
        LogSeverity::Trace => rgb(0x64748b),
        LogSeverity::Info => rgb(0xcbd5e1),
        LogSeverity::Warning => rgb(0xfbbf24),
        LogSeverity::Error => rgb(0xf87171),
    };

    div()
        .h(px(22.))
        .flex()
        .items_center()
        .whitespace_nowrap()
        .text_color(color)
        .child(
            div()
                .w(px(64.))
                .flex_none()
                .pr_3()
                .text_right()
                .text_color(rgb(0x64748b))
                .child(line.number.to_string()),
        )
        .child(div().flex_none().child(line.text.clone()))
        .into_any_element()
}
