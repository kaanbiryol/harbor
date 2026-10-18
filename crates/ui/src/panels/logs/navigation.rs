use crate::gpui_component::{
    Icon, Sizable, StyledExt,
    list::ListItem,
    scroll::{Scrollbar, ScrollbarMode},
};
use crate::panels::{ci_status_icon, workflows::workflow_conclusion_tone};
use crate::{
    icons::Octicon,
    visual::{Tone, color, tone_text},
    workflow_logs::WorkflowLogView,
};
use gpui::{AnyElement, Context, IntoElement, div, prelude::*, px, uniform_list};
use harbor_logs::{LogNavigationRow, navigation_rows};

pub(super) fn render_navigation_row(
    row: &LogNavigationRow,
    selected_run: Option<u64>,
    selected: Option<&(String, Option<String>)>,
    cx: &mut Context<WorkflowLogView>,
) -> AnyElement {
    let section = row.job.as_ref().map(|job| (job.clone(), row.step.clone()));
    let run_id = row.run_id;
    let label = row.label.clone();
    let tone = if row.jobs_unavailable {
        Tone::Danger
    } else {
        workflow_conclusion_tone(row.conclusion, row.status).1
    };
    let icon = ci_status_icon(row.conclusion, row.status);
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
                                .child(Icon::new(icon).small().text_color(tone_text(tone))),
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
                                        .text_color(tone_text(tone))
                                        .child(row.detail.clone()),
                                ),
                        ),
                )
                .on_click(cx.listener(move |view, _, _, cx| {
                    view.select_workflow_log_section(run_id, section.clone(), cx);
                })),
        )
        .into_any_element()
}

pub(super) fn render_log_navigation(
    view: &WorkflowLogView,
    cx: &mut Context<WorkflowLogView>,
) -> impl IntoElement {
    let state = &view.state;
    let selected_run = view.selected_workflow_run_for_logs().map(|run| run.id);
    let selected = state.selected_section.clone();
    let navigation = navigation_rows(
        &view.runs,
        &state.navigation_jobs,
        selected_run,
        selected.as_ref().map(|(job, _)| job.as_str()),
    );
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
