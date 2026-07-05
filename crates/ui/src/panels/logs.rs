use gpui::{
    AnyElement, Context, IntoElement, ListHorizontalSizingBehavior, UniformListScrollHandle, div,
    prelude::*, px, uniform_list,
};
use gpui_component::{
    ActiveTheme, Disableable, Sizable,
    button::{Button, ButtonVariants},
};
use harbor_domain::{WorkflowJob, WorkflowRun, WorkflowStep};
use harbor_logs::{LogChunk, LogLine, LogSeverity};

use crate::{
    icons::Octicon,
    visual::{Tone, color, tone_text},
    workspace::AppView,
};

use super::{
    render_empty_panel_card, render_error_panel_card, render_key_hint, render_panel_card,
    render_panel_header, render_status_pill,
    workflows::{render_workflow_conclusion, workflow_conclusion_tone, workflow_run_label},
};

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
                .child(render_panel_header(
                    "Logs",
                    Some(format!(
                        "{} lines",
                        log_chunk.map_or(0, |chunk| chunk.lines.len())
                    )),
                ))
                .child(
                    Button::new("load-workflow-logs")
                        .icon(Octicon::Sync)
                        .label("load logs")
                        .small()
                        .primary()
                        .loading(is_loading)
                        .disabled(run.is_none() || is_loading)
                        .on_click(cx.listener(|view, _, _, cx| {
                            view.load_selected_workflow_logs(cx);
                        })),
                ),
        )
        .child(render_logs_target_card(run))
        .when(is_loading, |element| {
            element.child(render_empty_panel_card("Loading workflow jobs and logs..."))
        })
        .when_some(error.map(str::to_string), |element, error| {
            element.child(render_error_panel_card(error))
        })
        .when(!is_loading && run.is_none(), |element| {
            element.child(render_empty_panel_card(
                "No workflow run found for this PR head",
            ))
        })
        .when(!jobs.is_empty(), |element| {
            element
                .child(
                    div()
                        .pt_1()
                        .text_xs()
                        .text_color(color::text_muted())
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
                    .border_color(color::border())
                    .bg(color::content_background())
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
                        .font_family(cx.theme().mono_font_family.clone())
                        .text_xs(),
                    ),
            )
        })
        .when(
            !is_loading && run.is_some() && error.is_none() && log_chunk.is_none(),
            |element| {
                element.child(render_empty_panel_card(
                    "Press l or load logs to fetch the workflow log output",
                ))
            },
        )
}

fn render_logs_target_card(run: Option<&WorkflowRun>) -> impl IntoElement {
    render_panel_card()
        .px_3()
        .py_2()
        .flex()
        .items_center()
        .justify_between()
        .gap_3()
        .text_xs()
        .text_color(color::text_muted())
        .child(div().min_w_0().flex_1().truncate().child(format!(
                    "target: {}",
                    run.map(workflow_run_label)
                        .unwrap_or_else(|| "no workflow run".to_string())
                )))
        .child(
            div()
                .flex()
                .items_center()
                .gap_1()
                .child(render_key_hint("l")),
        )
}

pub(crate) fn render_workflow_job(job: &WorkflowJob) -> impl IntoElement {
    render_panel_card()
        .flex()
        .flex_col()
        .gap_1()
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
    let (label, tone) = workflow_conclusion_tone(step.conclusion, step.status);

    div()
        .flex()
        .items_center()
        .justify_between()
        .gap_3()
        .pl_3()
        .text_xs()
        .text_color(color::text_muted())
        .child(format!("{}. {}", step.number, step.name))
        .child(render_status_pill(label, tone))
}

pub(crate) fn render_log_line(line: &LogLine) -> AnyElement {
    let color = match line.severity {
        LogSeverity::Trace => color::text_muted(),
        LogSeverity::Info => color::text_secondary(),
        LogSeverity::Warning => tone_text(Tone::Warning),
        LogSeverity::Error => tone_text(Tone::Danger),
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
                .text_color(color::text_muted())
                .child(line.number.to_string()),
        )
        .child(div().flex_none().child(line.text.clone()))
        .into_any_element()
}
