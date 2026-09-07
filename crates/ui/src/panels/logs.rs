use crate::gpui_component::{
    ActiveTheme, Disableable, Icon, Selectable, Sizable, StyledExt,
    button::{Button, ButtonVariants},
    list::ListItem,
    menu::{DropdownMenu, PopupMenuItem},
    scroll::{Scrollbar, ScrollbarMode},
};
use gpui::{
    AnyElement, Context, IntoElement, ListHorizontalSizingBehavior, div, prelude::*, px,
    uniform_list,
};
use harbor_domain::WorkflowJob;
use harbor_logs::{LogLine, LogSeverity};

use crate::{
    icons::Octicon,
    visual::{Tone, color, tone_text},
    workspace::AppView,
};

use super::{
    render_error_panel_card, render_panel_header,
    workflows::{workflow_conclusion_tone, workflow_run_label},
};

pub(crate) fn render_logs_panel(view: &AppView, cx: &mut Context<AppView>) -> impl IntoElement {
    let state = &view.detail_state.log_state;
    let run = view.selected_workflow_run_for_logs();
    let jobs = view.detail_state.workflow_jobs();
    let chunk = state.chunk();
    let all_skipped = !jobs.is_empty()
        && jobs
            .iter()
            .all(|job| job.conclusion == Some(harbor_domain::WorkflowConclusion::Skipped));
    let scroll = state.list_scroll.clone();
    let line_count = state.visible_lines.len();
    let navigation = navigation_rows(jobs);
    let navigation_count = navigation.len();
    let selected = state.selected_section.clone();
    let runs = view
        .detail_state
        .workflow_runs()
        .iter()
        .map(|run| {
            (
                run.id,
                format!(
                    "{} · #{} · attempt {}",
                    workflow_run_label(run),
                    run.run_number.unwrap_or(run.id),
                    run.run_attempt.unwrap_or(1)
                ),
            )
        })
        .collect::<Vec<_>>();
    let selected_run = run.map(|run| run.id);
    let weak_view = cx.entity().downgrade();
    let mut content = div()
        .id("logs-panel")
        .flex()
        .flex_col()
        .flex_1()
        .min_h_0()
        .min_w_0()
        .gap_2()
        .child(
            div()
                .flex()
                .flex_wrap()
                .items_center()
                .gap_2()
                .child(render_panel_header(
                    "Logs",
                    Some(format!("{} jobs", jobs.len())),
                ))
                .child(div().flex_1())
                .when(!all_skipped, |element| {
                    element.child(
                        Button::new("load-workflow-logs")
                            .icon(Octicon::Sync)
                            .label(if chunk.is_some() {
                                "Refresh"
                            } else {
                                "Load logs"
                            })
                            .small()
                            .ghost()
                            .loading(state.is_loading())
                            .disabled(run.is_none() || state.is_loading() || all_skipped)
                            .on_click(
                                cx.listener(|view, _, _, cx| view.load_selected_workflow_logs(cx)),
                            ),
                    )
                })
                .when_some(run, |element, run| {
                    let url = run.html_url.clone();
                    element.child(
                        Button::new("open-log-run")
                            .icon(Octicon::LinkExternal)
                            .tooltip("Open run in GitHub")
                            .small()
                            .ghost()
                            .on_click(move |_, _, cx| cx.open_url(&url)),
                    )
                }),
        )
        .child(
            div()
                .flex()
                .gap_2()
                .items_center()
                .child(
                    Button::new("previous-log-run")
                        .label(
                            run.map(workflow_run_label)
                                .unwrap_or_else(|| "Select run".into()),
                        )
                        .icon(Octicon::ChevronDown)
                        .small()
                        .ghost()
                        .max_w(px(300.))
                        .disabled(runs.len() < 2 || state.is_loading())
                        .dropdown_menu(move |mut menu, _, _| {
                            for (run_id, label) in &runs {
                                let run_id = *run_id;
                                let weak_view = weak_view.clone();
                                menu = menu.item(
                                    PopupMenuItem::new(label.clone())
                                        .checked(Some(run_id) == selected_run)
                                        .on_click(move |_, _, cx| {
                                            if let Err(error) = weak_view.update(cx, |view, cx| {
                                                view.detail_state.log_state.selected_run =
                                                    Some(run_id);
                                                view.load_selected_workflow_logs(cx);
                                            }) {
                                                tracing::debug!(%error, "log run selector closed");
                                            }
                                        }),
                                );
                            }
                            menu
                        }),
                )
                .child(
                    div()
                        .min_w_0()
                        .flex_1()
                        .truncate()
                        .text_xs()
                        .text_color(color::text_muted())
                        .child(
                            run.map(|run| {
                                format!(
                                    "{} · {} · attempt {}",
                                    run.head_branch,
                                    run.head_sha.chars().take(7).collect::<String>(),
                                    run.run_attempt.unwrap_or(1)
                                )
                            })
                            .unwrap_or_else(|| "No workflow run".into()),
                        ),
                ),
        );
    if let Some(error) = state.error() {
        content = content.child(render_error_panel_card(error.to_string()));
    }
    if let Some(input) = state
        .search_input
        .as_ref()
        .filter(|_| run.is_some() && !all_skipped)
    {
        content = content.child(
            div()
                .flex()
                .flex_wrap()
                .items_center()
                .gap_2()
                .child(
                    div().w(px(240.)).min_w(px(140.)).child(crate::gpui_component::input::Input::new(input)
                        .prefix(Icon::new(Octicon::Search).xsmall())
                        .disabled(chunk.is_none())
                        .small()
                        .cleanable(true)),
                )
                .child(
                    Button::new("log-timestamps")
                        .icon(Octicon::Clock)
                        .tooltip("Toggle timestamps")
                        .small()
                        .ghost()
                        .selected(state.show_timestamps)
                        .on_click(cx.listener(|view, _, _, cx| {
                            view.detail_state.log_state.show_timestamps =
                                !view.detail_state.log_state.show_timestamps;
                            cx.notify();
                        })),
                )
                .child(Button::new("next-failed-step").label("Failed step").icon(Octicon::ArrowRight).small().ghost()
                    .disabled(!jobs.iter().any(|job| job.steps.iter().any(|step| matches!(step.conclusion, Some(harbor_domain::WorkflowConclusion::Failure | harbor_domain::WorkflowConclusion::TimedOut)))))
                    .on_click(cx.listener(|view, _, _, cx| {
                        let failures = view.detail_state.workflow_jobs().iter().flat_map(|job| job.steps.iter().filter(|step| matches!(step.conclusion, Some(harbor_domain::WorkflowConclusion::Failure | harbor_domain::WorkflowConclusion::TimedOut))).map(|step| (job.name.clone(), Some(step.name.clone())))).collect::<Vec<_>>();
                        let current = failures.iter().position(|section| Some(section) == view.detail_state.log_state.selected_section.as_ref());
                        if let Some(section) = failures.get(current.map_or(0, |index| (index + 1) % failures.len().max(1))) {
                            view.detail_state.log_state.selected_section = Some(section.clone());
                            view.filter_workflow_logs(cx);
                        }
                    })))
                .child(
                    Button::new("next-log-failure")
                        .label("Next error")
                        .small()
                        .ghost()
                        .disabled(chunk.is_none_or(|chunk| chunk.error_line_indices.is_empty()))
                        .on_click(cx.listener(|view, _, _, cx| {
                            let state = &mut view.detail_state.log_state;
                            let next = state
                                .chunk()
                                .and_then(|chunk| {
                                    state
                                        .visible_lines
                                        .iter()
                                        .enumerate()
                                        .skip(state.next_failure)
                                        .find(|(_, index)| {
                                            chunk.error_line_indices.binary_search(index).is_ok()
                                        })
                                        .or_else(|| {
                                            state.visible_lines.iter().enumerate().find(
                                                |(_, index)| {
                                                    chunk
                                                        .error_line_indices
                                                        .binary_search(index)
                                                        .is_ok()
                                                },
                                            )
                                        })
                                })
                                .map(|(position, _)| position);
                            if let Some(position) = next {
                                state.next_failure = position + 1;
                                state
                                    .list_scroll
                                    .scroll_to_item(position, gpui::ScrollStrategy::Top);
                            }
                            cx.notify();
                        })),
                )
                .child(
                    Button::new("copy-workflow-logs")
                        .icon(Octicon::Copy)
                        .tooltip("Copy visible logs")
                        .small()
                        .ghost()
                        .disabled(line_count == 0)
                        .on_click(
                            cx.listener(|view, _, _, cx| view.export_workflow_logs(false, cx)),
                        ),
                )
                .child(
                    Button::new("download-workflow-logs")
                        .label("Download")
                        .small()
                        .ghost()
                        .disabled(line_count == 0)
                        .on_click(
                            cx.listener(|view, _, _, cx| view.export_workflow_logs(true, cx)),
                        ),
                ),
        );
    }
    let output_title = selected
        .as_ref()
        .map(|(job, step)| step.as_deref().unwrap_or(job).to_string())
        .unwrap_or_else(|| "All output".into());
    content.child(
        div()
            .flex()
            .flex_1()
            .min_h_0()
            .min_w_0()
            .overflow_hidden()
            .border_t_1()
            .border_color(color::border_subtle())
            .when(run.is_some(), |element| {
                element.child(
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
                                    view.detail_state.log_state.selected_section = None;
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
                                .child(format!("JOBS · {}", jobs.len())),
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
                                        cx.processor(
                                            move |_view, range: std::ops::Range<usize>, _, cx| {
                                                range
                                                    .filter_map(|index| {
                                                        navigation.get(index).map(|row| {
                                                            render_navigation_row(
                                                                row,
                                                                selected.as_ref(),
                                                                cx,
                                                            )
                                                        })
                                                    })
                                                    .collect::<Vec<_>>()
                                            },
                                        ),
                                    )
                                    .track_scroll(&state.navigation_scroll)
                                    .flex_1()
                                    .min_h_0()
                                    .size_full(),
                                )
                                .child(
                                    Scrollbar::vertical(&state.navigation_scroll)
                                        .mode(ScrollbarMode::Always),
                                ),
                        ),
                )
            })
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_w_0()
                    .min_h_0()
                    .bg(color::content_background())
                    .overflow_hidden()
                    .child(
                        div()
                            .h(px(40.))
                            .flex_none()
                            .flex()
                            .items_center()
                            .gap_3()
                            .px_4()
                            .border_b_1()
                            .border_color(color::border_subtle())
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .truncate()
                                    .text_xs()
                                    .text_color(color::text_secondary())
                                    .child(output_title),
                            )
                            .child(div().text_xs().text_color(color::text_muted()).child(
                                if state.is_loading() {
                                    if chunk.is_some() {
                                        "Refreshing…".into()
                                    } else {
                                        "Loading output…".into()
                                    }
                                } else if state.is_filtering {
                                    "Updating output…".into()
                                } else if all_skipped {
                                    "Skipped".into()
                                } else {
                                    format!("{line_count} lines")
                                },
                            )),
                    )
                    .when(state.is_loading() && chunk.is_none(), |element| {
                        element.child(super::render_loading_panel_skeleton(
                            "logs-loading-skeleton",
                            12,
                            22.0,
                        ))
                    })
                    .when(line_count > 0, |element| {
                        element.child(
                            div()
                                .relative()
                                .flex_1()
                                .min_h_0()
                                .min_w_0()
                                .overflow_hidden()
                                .child(
                                    uniform_list(
                                        "workflow-log-lines",
                                        line_count,
                                        cx.processor(
                                            |view, range: std::ops::Range<usize>, _, _| {
                                                let state = &view.detail_state.log_state;
                                                let Some(chunk) = state.chunk() else {
                                                    return Vec::new();
                                                };
                                                range
                                                    .filter_map(|index| {
                                                        state.visible_lines.get(index).and_then(
                                                            |index| chunk.lines.get(*index),
                                                        )
                                                    })
                                                    .map(|line| {
                                                        render_log_line_with_timestamps(
                                                            line,
                                                            state.show_timestamps,
                                                        )
                                                    })
                                                    .collect()
                                            },
                                        ),
                                    )
                                    .track_scroll(&scroll)
                                    .with_width_from_item(state.widest_line)
                                    .debug_selector(|| "workflow-log-viewport".to_string())
                                    .with_horizontal_sizing_behavior(
                                        ListHorizontalSizingBehavior::Unconstrained,
                                    )
                                    .size_full()
                                    .min_h_0()
                                    .min_w_0()
                                    .font_family(cx.theme().mono_font_family.clone())
                                    .text_xs(),
                                )
                                .child(Scrollbar::new(&scroll).mode(ScrollbarMode::Always)),
                        )
                    })
                    .when(
                        line_count == 0
                            && !state.is_loading()
                            && !state.is_filtering
                            && state.error().is_none(),
                        |element| {
                            let (title, description) = if run.is_none() {
                                (
                                    "No workflow run available",
                                    "No workflow run was found for this pull request head.",
                                )
                            } else if all_skipped {
                                (
                                    "No logs for this run",
                                    "All jobs were skipped. No output was produced.",
                                )
                            } else if chunk.is_none() {
                                (
                                    "Ready to load logs",
                                    "Load this run’s output to inspect jobs and steps.",
                                )
                            } else if chunk.is_some_and(|chunk| chunk.lines.is_empty()) {
                                ("No output yet", "Refresh after the running jobs complete.")
                            } else {
                                (
                                    "No matching log lines",
                                    "Select All output or clear the search to see more logs.",
                                )
                            };
                            element.child(
                                div()
                                    .flex_1()
                                    .flex()
                                    .flex_col()
                                    .items_center()
                                    .justify_center()
                                    .gap_2()
                                    .p_6()
                                    .text_center()
                                    .child(
                                        Icon::new(if all_skipped {
                                            Octicon::Clock
                                        } else {
                                            Octicon::Terminal
                                        })
                                        .large()
                                        .text_color(color::text_disabled()),
                                    )
                                    .child(
                                        div()
                                            .mt_1()
                                            .text_sm()
                                            .font_medium()
                                            .text_color(color::text_secondary())
                                            .child(title),
                                    )
                                    .child(
                                        div()
                                            .max_w(px(320.))
                                            .text_xs()
                                            .text_color(color::text_muted())
                                            .child(description),
                                    ),
                            )
                        },
                    ),
            ),
    )
}

fn render_navigation_row(
    row: &LogNavigationRow,
    selected: Option<&(String, Option<String>)>,
    cx: &mut Context<AppView>,
) -> AnyElement {
    let section = (row.job.clone(), row.step.clone());
    let label = row.label.clone();
    div()
        .h(px(52.))
        .py_0p5()
        .child(
            ListItem::new(gpui::SharedString::from(format!("log-section-{}", row.id)))
                .selected(selected == Some(&section))
                .h_full()
                .px_2()
                .when(row.step.is_some(), |element| element.pl_5())
                .tooltip(move |window, cx| {
                    crate::gpui_component::tooltip::Tooltip::new(label.clone()).build(window, cx)
                })
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_2()
                        .min_w_0()
                        .child(
                            Icon::new(row.icon)
                                .small()
                                .flex_none()
                                .text_color(tone_text(row.tone)),
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
                                        .truncate()
                                        .text_color(color::text_primary())
                                        .child(row.label.clone()),
                                )
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(tone_text(row.tone))
                                        .child(row.status.clone()),
                                ),
                        ),
                )
                .on_click(cx.listener(move |view, _, _, cx| {
                    view.detail_state.log_state.selected_section = Some(section.clone());
                    view.filter_workflow_logs(cx);
                })),
        )
        .into_any_element()
}

struct LogNavigationRow {
    id: String,
    job: String,
    step: Option<String>,
    label: String,
    status: String,
    tone: Tone,
    icon: Octicon,
}

fn navigation_rows(jobs: &[WorkflowJob]) -> Vec<LogNavigationRow> {
    let mut rows = Vec::new();
    for job in jobs {
        let (status, tone) = workflow_conclusion_tone(job.conclusion, job.status);
        rows.push(LogNavigationRow {
            id: job.id.to_string(),
            job: job.name.clone(),
            step: None,
            label: job.name.clone(),
            status: status.into(),
            tone,
            icon: status_icon(job.conclusion, job.status),
        });
        for step in &job.steps {
            let (status, tone) = workflow_conclusion_tone(step.conclusion, step.status);
            let duration = step
                .started_at
                .zip(step.completed_at)
                .map(|(start, end)| format!(" · {}s", (end - start).num_seconds().max(0)))
                .unwrap_or_default();
            rows.push(LogNavigationRow {
                id: format!("{}-{}", job.id, step.number),
                job: job.name.clone(),
                step: Some(step.name.clone()),
                label: step.name.clone(),
                status: format!("{status}{duration}"),
                tone,
                icon: status_icon(step.conclusion, step.status),
            });
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

fn render_log_line_with_timestamps(line: &LogLine, timestamps: bool) -> AnyElement {
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
        .child(div().flex_none().child(if timestamps {
            line.text.clone()
        } else {
            line.text
                .split_once(' ')
                .filter(|(prefix, _)| prefix.contains('T') && prefix.ends_with('Z'))
                .map_or_else(|| line.text.clone(), |(_, text)| text.to_string())
        }))
        .into_any_element()
}
