use std::collections::HashSet;

use chrono::Duration;
use gpui::{Context, IntoElement, ListState, div, list, prelude::*, px};
use gpui_component::{Icon, Sizable, StyledExt};
use harbor_domain::{CheckConclusion, CheckRun, CheckStatus, ChecksSummary};

use crate::{
    date_time::short_duration_label,
    icons::Octicon,
    visual::{Tone, color, tone_colors},
    workspace::AppView,
};

use super::{
    render_empty_panel_card, render_error_panel_card, render_panel_card, render_panel_header,
    sync_virtual_list_item_count,
};

const CHECK_GROUP_HEADER_HEIGHT: f32 = 42.0;
const CHECK_RUN_ROW_HEIGHT: f32 = 40.0;

#[path = "checks/model.rs"]
mod model;

pub(crate) use model::CheckRunFilter;
use model::{CheckPanelRow, CheckRunGroup, check_panel_rows, check_run_display_name};

pub(crate) struct CheckPanelRenderInput<'a> {
    pub(crate) summary: ChecksSummary,
    pub(crate) check_runs: &'a [CheckRun],
    pub(crate) expanded_groups: &'a HashSet<String>,
    pub(crate) active_filter: CheckRunFilter,
    pub(crate) is_loading: bool,
    pub(crate) error: Option<&'a str>,
    pub(crate) list_state: ListState,
}

pub(crate) fn render_checks_panel(
    input: CheckPanelRenderInput<'_>,
    cx: &mut Context<AppView>,
) -> impl IntoElement {
    let CheckPanelRenderInput {
        summary,
        check_runs,
        expanded_groups,
        active_filter,
        is_loading,
        error,
        list_state,
    } = input;
    let rows = check_panel_rows(check_runs, expanded_groups, active_filter);
    sync_virtual_list_item_count(&list_state, rows.len());
    let rows_for_render = rows.clone();

    div()
        .id("checks-panel")
        .flex()
        .flex_col()
        .flex_1()
        .min_h_0()
        .gap_2()
        .child(render_panel_header(
            "Checks",
            Some(format!("{} runs", summary.total)),
        ))
        .child(
            div()
                .flex()
                .items_center()
                .gap_2()
                .flex_wrap()
                .child(render_check_filter_pill(
                    CheckRunFilter::All,
                    summary.total,
                    Tone::Neutral,
                    active_filter,
                    cx,
                ))
                .child(render_check_filter_pill(
                    CheckRunFilter::Passed,
                    summary.passed,
                    Tone::Success,
                    active_filter,
                    cx,
                ))
                .child(render_check_filter_pill(
                    CheckRunFilter::Failed,
                    summary.failed,
                    Tone::Danger,
                    active_filter,
                    cx,
                ))
                .child(render_check_filter_pill(
                    CheckRunFilter::Pending,
                    summary.pending,
                    Tone::Warning,
                    active_filter,
                    cx,
                ))
                .child(render_check_filter_pill(
                    CheckRunFilter::Skipped,
                    summary.skipped,
                    Tone::Neutral,
                    active_filter,
                    cx,
                )),
        )
        .when(summary.total > 0, |element| {
            element.child(render_check_completion_summary(summary))
        })
        .when(is_loading, |element| {
            element.child(render_empty_panel_card("Loading check runs..."))
        })
        .when_some(error.map(str::to_string), |element, error| {
            element.child(render_error_panel_card(error))
        })
        .when(
            !is_loading && error.is_none() && check_runs.is_empty(),
            |element| {
                element.child(render_empty_panel_card(
                    "No check runs found for this PR head",
                ))
            },
        )
        .when(
            !is_loading && error.is_none() && !check_runs.is_empty() && rows.is_empty(),
            |element| element.child(render_empty_panel_card(active_filter.empty_message())),
        )
        .when(!check_runs.is_empty() && !rows.is_empty(), |element| {
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
                            cx.processor(move |view, index: usize, _window, cx| {
                                let Some(row) = rows_for_render.get(index) else {
                                    return div().into_any_element();
                                };

                                match row {
                                    CheckPanelRow::Group(group) => {
                                        render_check_group_header(group.clone(), cx)
                                            .into_any_element()
                                    }
                                    CheckPanelRow::Check { check_index } => {
                                        let Some(check_run) =
                                            view.detail_state.check_runs().get(*check_index)
                                        else {
                                            return div().into_any_element();
                                        };

                                        render_check_run(*check_index, check_run).into_any_element()
                                    }
                                }
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

fn render_check_filter_pill(
    filter: CheckRunFilter,
    value: usize,
    tone: Tone,
    active_filter: CheckRunFilter,
    cx: &mut Context<AppView>,
) -> impl IntoElement {
    let colors = tone_colors(tone);
    let is_active = filter == active_filter;
    let border_color = if is_active && tone == Tone::Neutral {
        color::accent()
    } else if is_active {
        colors.text
    } else {
        colors.border
    };
    let label = format!("{} {value}", filter.status_label());

    div()
        .id(("check-filter", filter.id()))
        .flex_none()
        .rounded_xs()
        .border_1()
        .border_color(border_color)
        .bg(if is_active {
            color::row_selected()
        } else {
            colors.background
        })
        .px_1()
        .py_0p5()
        .text_xs()
        .font_medium()
        .text_color(colors.text)
        .cursor_pointer()
        .hover(move |style| {
            style.bg(if is_active {
                color::row_selected_active()
            } else {
                color::row_hover()
            })
        })
        .on_click(cx.listener(move |view, _, _, cx| {
            view.set_checks_filter(filter, cx);
        }))
        .child(label)
}

fn render_check_completion_summary(summary: ChecksSummary) -> impl IntoElement {
    let [passed, skipped, complete] = check_completion_summary_labels(summary);

    div()
        .flex()
        .items_center()
        .gap_2()
        .text_xs()
        .child(render_check_outcome_label(passed, Tone::Success))
        .child(div().text_color(color::text_disabled()).child("·"))
        .child(render_check_outcome_label(skipped, Tone::Neutral))
        .child(div().text_color(color::text_disabled()).child("·"))
        .child(render_check_outcome_label(complete, Tone::Neutral))
}

fn check_completion_summary_labels(summary: ChecksSummary) -> [String; 3] {
    let completed = summary.total.saturating_sub(summary.pending);

    [
        format!("{} passed", summary.passed),
        format!("{} skipped", summary.skipped),
        format!("{completed} of {} complete", summary.total),
    ]
}

fn render_check_group_header(group: CheckRunGroup, cx: &mut Context<AppView>) -> impl IntoElement {
    let chevron = if group.expanded {
        Octicon::ChevronDown
    } else {
        Octicon::ChevronRight
    };
    let group_name = group.name.clone();
    let toggle_group_name = group.name.clone();
    let group_id = group.check_indices.first().copied().unwrap_or(0);
    let (_, summary_tone) = check_group_summary_label(group.summary);
    let summary_icon = check_group_status_icon(group.summary);

    div()
        .id(("check-group", group_id))
        .h(px(CHECK_GROUP_HEADER_HEIGHT))
        .w_full()
        .min_w_0()
        .flex()
        .items_center()
        .justify_between()
        .gap_3()
        .overflow_hidden()
        .px_3()
        .border_b_1()
        .border_color(color::border_subtle())
        .bg(color::panel_background())
        .cursor_pointer()
        .hover(|style| style.bg(color::row_hover()))
        .on_click(cx.listener(move |view, _, _, cx| {
            view.toggle_check_group(toggle_group_name.clone(), cx);
        }))
        .child(
            div()
                .min_w_0()
                .flex_1()
                .flex()
                .items_center()
                .gap_2()
                .child(Icon::new(chevron).xsmall().text_color(color::text_muted()))
                .child(render_check_status_icon(summary_icon, summary_tone))
                .child(
                    div()
                        .min_w_0()
                        .truncate()
                        .font_medium()
                        .text_color(color::text_primary())
                        .child(group_name),
                )
                .child(
                    div()
                        .flex_none()
                        .text_xs()
                        .text_color(color::text_muted())
                        .child(check_count_label(group.summary.total)),
                ),
        )
        .child(render_check_group_summary(group.summary))
}

fn render_check_group_summary(summary: ChecksSummary) -> impl IntoElement {
    let (label, tone) = check_group_summary_label(summary);

    render_check_outcome_label(label, tone)
}

fn check_group_summary_label(summary: ChecksSummary) -> (String, Tone) {
    let (label, tone) = if summary.failed > 0 {
        (
            check_result_count_label(summary.failed, "failed", summary.total),
            Tone::Danger,
        )
    } else if summary.pending > 0 {
        (
            check_result_count_label(summary.pending, "pending", summary.total),
            Tone::Warning,
        )
    } else if summary.passed > 0 {
        (
            check_result_count_label(summary.passed, "passed", summary.total),
            Tone::Success,
        )
    } else {
        (
            check_result_count_label(summary.skipped, "skipped", summary.total),
            Tone::Neutral,
        )
    };

    (label, tone)
}

pub(crate) fn render_check_run(check_index: usize, check_run: &CheckRun) -> impl IntoElement {
    let check_url = check_run
        .html_url
        .clone()
        .or_else(|| check_run.details_url.clone());
    let click_url = check_url.clone();
    let display_name = check_run_display_name(check_run);
    let (_, tone) = check_conclusion_label(check_run.conclusion, check_run.status);
    let status_icon = check_status_icon(check_run.conclusion, check_run.status);

    div()
        .id(("check-run", check_index))
        .h(px(CHECK_RUN_ROW_HEIGHT))
        .w_full()
        .min_w_0()
        .flex()
        .items_center()
        .justify_between()
        .gap_3()
        .overflow_hidden()
        .px_3()
        .border_b_1()
        .border_color(color::border_subtle())
        .bg(color::content_background())
        .when(check_url.is_some(), |element| {
            element
                .cursor_pointer()
                .hover(|style| style.bg(color::row_hover()))
        })
        .when_some(click_url, |element, url| {
            element.on_click(move |_, _, cx| {
                cx.open_url(&url);
            })
        })
        .child(
            div()
                .min_w_0()
                .flex_1()
                .flex()
                .items_center()
                .gap_2()
                .child(div().w(px(16.0)).flex_none())
                .child(render_check_status_icon(status_icon, tone))
                .child(
                    div()
                        .min_w_0()
                        .flex_1()
                        .flex()
                        .items_center()
                        .gap_2()
                        .child(
                            div()
                                .min_w_0()
                                .flex_1()
                                .truncate()
                                .text_sm()
                                .text_color(color::text_primary())
                                .child(display_name),
                        )
                        .child(
                            div()
                                .flex_none()
                                .truncate()
                                .text_xs()
                                .text_color(color::text_muted())
                                .child(check_run_metadata_label(check_run)),
                        ),
                ),
        )
        .child(
            div()
                .flex_none()
                .flex()
                .items_center()
                .gap_2()
                .child(render_check_conclusion(
                    check_run.conclusion,
                    check_run.status,
                ))
                .when(check_url.is_some(), |element| {
                    element.child(
                        Icon::new(Octicon::LinkExternal)
                            .xsmall()
                            .text_color(color::text_muted()),
                    )
                }),
        )
}

fn check_run_metadata_label(check_run: &CheckRun) -> String {
    let status = check_status_label(check_run.status);

    match check_run_duration_label(check_run) {
        Some(duration) => format!("{status} in {duration}"),
        None => status.to_string(),
    }
}

fn check_run_duration_label(check_run: &CheckRun) -> Option<String> {
    if check_run.status != CheckStatus::Completed {
        return None;
    }

    let started_at = check_run.started_at?;
    let completed_at = check_run.completed_at?;
    let duration = completed_at.signed_duration_since(started_at);
    (duration >= Duration::zero()).then(|| short_duration_label(duration))
}

fn check_count_label(count: usize) -> String {
    if count == 1 {
        "1 check".to_string()
    } else {
        format!("{count} checks")
    }
}

fn check_result_count_label(count: usize, label: &str, total: usize) -> String {
    if total == 1 {
        label.to_string()
    } else {
        format!("{count} {label}")
    }
}

fn render_check_status_icon(icon: Octicon, tone: Tone) -> impl IntoElement {
    let colors = tone_colors(tone);

    div()
        .w(px(16.0))
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .child(Icon::new(icon).small().text_color(colors.text))
}

fn check_group_status_icon(summary: ChecksSummary) -> Octicon {
    if summary.failed > 0 {
        Octicon::XCircle
    } else if summary.pending > 0 {
        Octicon::Sync
    } else if summary.passed > 0 {
        Octicon::CheckCircle
    } else {
        Octicon::Clock
    }
}

fn check_status_icon(conclusion: Option<CheckConclusion>, status: CheckStatus) -> Octicon {
    match (status, conclusion) {
        (CheckStatus::Completed, Some(CheckConclusion::Success)) => Octicon::CheckCircle,
        (CheckStatus::Completed, Some(CheckConclusion::Skipped | CheckConclusion::Neutral)) => {
            Octicon::Clock
        }
        (
            CheckStatus::Completed,
            Some(CheckConclusion::TimedOut | CheckConclusion::ActionRequired),
        ) => Octicon::Alert,
        (
            CheckStatus::Completed,
            Some(CheckConclusion::Cancelled | CheckConclusion::Failure) | None,
        ) => Octicon::XCircle,
        (CheckStatus::InProgress, _) => Octicon::Sync,
        (CheckStatus::Queued, _) => Octicon::Clock,
    }
}

fn render_check_outcome_label(label: impl Into<String>, tone: Tone) -> impl IntoElement {
    let colors = tone_colors(tone);

    div()
        .flex_none()
        .text_xs()
        .font_medium()
        .text_color(colors.text)
        .child(label.into())
}

pub(crate) fn render_check_conclusion(
    conclusion: Option<CheckConclusion>,
    status: CheckStatus,
) -> impl IntoElement {
    let (label, tone) = check_conclusion_label(conclusion, status);

    render_check_outcome_label(label, tone)
}

fn check_conclusion_label(
    conclusion: Option<CheckConclusion>,
    status: CheckStatus,
) -> (&'static str, Tone) {
    match (status, conclusion) {
        (CheckStatus::Completed, Some(CheckConclusion::Success)) => ("passed", Tone::Success),
        (CheckStatus::Completed, Some(CheckConclusion::Skipped)) => ("skipped", Tone::Neutral),
        (CheckStatus::Completed, Some(CheckConclusion::Neutral)) => ("neutral", Tone::Neutral),
        (CheckStatus::Completed, Some(CheckConclusion::Cancelled)) => ("cancelled", Tone::Warning),
        (CheckStatus::Completed, Some(CheckConclusion::TimedOut)) => ("timed out", Tone::Danger),
        (CheckStatus::Completed, Some(CheckConclusion::ActionRequired)) => {
            ("action required", Tone::Warning)
        }
        (CheckStatus::Completed, Some(CheckConclusion::Failure) | None) => ("failed", Tone::Danger),
        (CheckStatus::InProgress, _) => ("running", Tone::Info),
        (CheckStatus::Queued, _) => ("queued", Tone::Warning),
    }
}

fn check_status_label(status: CheckStatus) -> &'static str {
    match status {
        CheckStatus::Queued => "queued",
        CheckStatus::InProgress => "running",
        CheckStatus::Completed => "completed",
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use chrono::{Duration, TimeZone, Utc};
    use harbor_domain::{CheckConclusion, CheckRun, CheckStatus};

    use super::*;
    use crate::test_fixtures::check_run;

    #[test]
    fn groups_checks_by_name_prefix_and_collapses_them_by_default() {
        let checks = vec![
            named_check(
                "ci / unit",
                CheckStatus::Completed,
                Some(CheckConclusion::Success),
            ),
            named_check(
                "ci / lint",
                CheckStatus::Completed,
                Some(CheckConclusion::Failure),
            ),
            named_check("security / scan", CheckStatus::InProgress, None),
            named_check("external", CheckStatus::Queued, None),
        ];

        let rows = check_panel_rows(&checks, &HashSet::new(), CheckRunFilter::All);

        assert_eq!(
            row_labels(&rows),
            vec![
                "group:ci:2:closed",
                "group:other checks:1:closed",
                "group:security:1:closed",
            ]
        );
    }

    #[test]
    fn shows_checks_for_expanded_groups() {
        let checks = vec![
            named_check(
                "ci / unit",
                CheckStatus::Completed,
                Some(CheckConclusion::Success),
            ),
            named_check(
                "ci / lint",
                CheckStatus::Completed,
                Some(CheckConclusion::Failure),
            ),
        ];
        let expanded_groups = HashSet::from(["ci".to_string()]);

        let rows = check_panel_rows(&checks, &expanded_groups, CheckRunFilter::All);

        assert_eq!(
            row_labels(&rows),
            vec!["group:ci:2:open", "check:0", "check:1"]
        );
    }

    #[test]
    fn trims_group_prefix_from_check_display_names() {
        let check = named_check(
            "ci / unit",
            CheckStatus::Completed,
            Some(CheckConclusion::Success),
        );
        let external = named_check("external", CheckStatus::Queued, None);

        assert_eq!(check_run_display_name(&check), "unit");
        assert_eq!(check_run_display_name(&external), "external");
    }

    #[test]
    fn filters_checks_by_outcome_before_grouping() {
        let checks = vec![
            named_check(
                "ci / unit",
                CheckStatus::Completed,
                Some(CheckConclusion::Success),
            ),
            named_check(
                "ci / lint",
                CheckStatus::Completed,
                Some(CheckConclusion::Failure),
            ),
            named_check(
                "deploy / smoke",
                CheckStatus::Completed,
                Some(CheckConclusion::Skipped),
            ),
            named_check("external", CheckStatus::Queued, None),
        ];

        let expanded_groups = HashSet::from(["ci".to_string(), "other checks".to_string()]);
        let rows = check_panel_rows(&checks, &expanded_groups, CheckRunFilter::Failed);

        assert_eq!(row_labels(&rows), vec!["group:ci:1:open", "check:1"]);

        let rows = check_panel_rows(&checks, &expanded_groups, CheckRunFilter::Pending);

        assert_eq!(
            row_labels(&rows),
            vec!["group:other checks:1:open", "check:3"]
        );
    }

    #[test]
    fn formats_check_group_counts_without_false_plurals() {
        assert_eq!(check_count_label(1), "1 check");
        assert_eq!(check_count_label(2), "2 checks");
    }

    #[test]
    fn summarizes_single_check_groups_without_repeating_the_count() {
        let summary = ChecksSummary {
            total: 1,
            skipped: 1,
            ..ChecksSummary::default()
        };

        assert_eq!(
            check_group_summary_label(summary),
            ("skipped".to_string(), Tone::Neutral)
        );
    }

    #[test]
    fn summarizes_multi_check_groups_with_outcome_counts() {
        let summary = ChecksSummary {
            total: 3,
            failed: 1,
            passed: 2,
            ..ChecksSummary::default()
        };

        assert_eq!(
            check_group_summary_label(summary),
            ("1 failed".to_string(), Tone::Danger)
        );

        let summary = ChecksSummary {
            total: 3,
            passed: 3,
            ..ChecksSummary::default()
        };

        assert_eq!(
            check_group_summary_label(summary),
            ("3 passed".to_string(), Tone::Success)
        );
    }

    #[test]
    fn formats_compact_completion_summary() {
        let summary = ChecksSummary {
            total: 12,
            passed: 2,
            pending: 0,
            skipped: 10,
            failed: 0,
        };

        assert_eq!(
            check_completion_summary_labels(summary),
            ["2 passed", "10 skipped", "12 of 12 complete"]
        );

        let summary = ChecksSummary {
            total: 12,
            passed: 2,
            pending: 3,
            skipped: 7,
            failed: 0,
        };

        assert_eq!(
            check_completion_summary_labels(summary),
            ["2 passed", "7 skipped", "9 of 12 complete"]
        );
    }

    #[test]
    fn uses_actions_status_icons_for_check_runs() {
        assert_eq!(
            check_status_icon(CheckConclusion::Success.into(), CheckStatus::Completed),
            Octicon::CheckCircle
        );
        assert_eq!(
            check_status_icon(CheckConclusion::Failure.into(), CheckStatus::Completed),
            Octicon::XCircle
        );
        assert_eq!(
            check_status_icon(CheckConclusion::TimedOut.into(), CheckStatus::Completed),
            Octicon::Alert
        );
        assert_eq!(
            check_status_icon(None, CheckStatus::InProgress),
            Octicon::Sync
        );
        assert_eq!(check_status_icon(None, CheckStatus::Queued), Octicon::Clock);
    }

    #[test]
    fn includes_completed_check_duration_when_available() {
        let started_at = Utc
            .with_ymd_and_hms(2026, 7, 8, 12, 0, 0)
            .single()
            .expect("valid timestamp");
        let completed_at = started_at + Duration::seconds(75);
        let mut check = named_check(
            "ci / unit",
            CheckStatus::Completed,
            Some(CheckConclusion::Success),
        );
        check.started_at = Some(started_at);
        check.completed_at = Some(completed_at);

        assert_eq!(check_run_metadata_label(&check), "completed in 1m 15s");
    }

    fn named_check(
        name: &str,
        status: CheckStatus,
        conclusion: Option<CheckConclusion>,
    ) -> CheckRun {
        let mut check = check_run(status, conclusion);
        check.name = name.to_string();
        check
    }

    fn row_labels(rows: &[CheckPanelRow]) -> Vec<String> {
        rows.iter()
            .map(|row| match row {
                CheckPanelRow::Group(group) => format!(
                    "group:{}:{}:{}",
                    group.name,
                    group.summary.total,
                    if group.expanded { "open" } else { "closed" }
                ),
                CheckPanelRow::Check { check_index } => format!("check:{check_index}"),
            })
            .collect()
    }
}
