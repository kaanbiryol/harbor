use std::collections::{BTreeMap, HashSet};

use gpui::{Context, IntoElement, ListState, div, list, prelude::*};
use gpui_component::{Icon, Sizable, StyledExt};
use harbor_domain::{CheckConclusion, CheckRun, CheckStatus, ChecksSummary};

use crate::{
    icons::Octicon,
    visual::{Tone, color},
    workspace::AppView,
};

use super::{
    render_empty_panel_card, render_error_panel_card, render_metric_pill, render_panel_card,
    render_panel_header, render_status_pill, sync_virtual_list_item_count,
};

pub(crate) fn render_checks_panel(
    summary: ChecksSummary,
    check_runs: &[CheckRun],
    collapsed_groups: &HashSet<String>,
    is_loading: bool,
    error: Option<&str>,
    list_state: ListState,
    cx: &mut Context<AppView>,
) -> impl IntoElement {
    let rows = check_panel_rows(check_runs, collapsed_groups);
    sync_virtual_list_item_count(&list_state, rows.len());

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
                .child(render_metric_pill("total", summary.total, Tone::Neutral))
                .child(render_metric_pill("passed", summary.passed, Tone::Success))
                .child(render_metric_pill("failed", summary.failed, Tone::Danger))
                .child(render_metric_pill(
                    "pending",
                    summary.pending,
                    Tone::Warning,
                ))
                .child(render_metric_pill(
                    "skipped",
                    summary.skipped,
                    Tone::Neutral,
                )),
        )
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
        .when(!check_runs.is_empty(), |element| {
            element.child(
                list(
                    list_state,
                    cx.processor(|view, index: usize, _window, cx| {
                        let rows = check_panel_rows(
                            view.detail_state.check_runs(),
                            view.collapsed_check_groups(),
                        );
                        let Some(row) = rows.get(index) else {
                            return div().into_any_element();
                        };

                        match row {
                            CheckPanelRow::Group(group) => {
                                render_check_group_header(group.clone(), cx).into_any_element()
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
            )
        })
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct CheckRunGroup {
    name: String,
    check_indices: Vec<usize>,
    summary: ChecksSummary,
    expanded: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum CheckPanelRow {
    Group(CheckRunGroup),
    Check { check_index: usize },
}

fn check_panel_rows(
    check_runs: &[CheckRun],
    collapsed_groups: &HashSet<String>,
) -> Vec<CheckPanelRow> {
    let mut rows = Vec::new();

    for group in check_run_groups(check_runs, collapsed_groups) {
        rows.push(CheckPanelRow::Group(group.clone()));
        if group.expanded {
            rows.extend(
                group
                    .check_indices
                    .into_iter()
                    .map(|check_index| CheckPanelRow::Check { check_index }),
            );
        }
    }

    rows
}

fn check_run_groups(
    check_runs: &[CheckRun],
    collapsed_groups: &HashSet<String>,
) -> Vec<CheckRunGroup> {
    let mut indices_by_group = BTreeMap::<String, Vec<usize>>::new();

    for (index, check_run) in check_runs.iter().enumerate() {
        indices_by_group
            .entry(check_group_name(&check_run.name).to_string())
            .or_default()
            .push(index);
    }

    indices_by_group
        .into_iter()
        .map(|(name, check_indices)| {
            let summary = checks_summary_for_indices(check_runs, &check_indices);
            let expanded = !collapsed_groups.contains(&name);

            CheckRunGroup {
                name,
                check_indices,
                summary,
                expanded,
            }
        })
        .collect()
}

fn checks_summary_for_indices(check_runs: &[CheckRun], check_indices: &[usize]) -> ChecksSummary {
    let mut summary = ChecksSummary {
        total: check_indices.len(),
        ..ChecksSummary::default()
    };

    for check_run in check_indices
        .iter()
        .filter_map(|index| check_runs.get(*index))
    {
        match (check_run.status, check_run.conclusion) {
            (CheckStatus::Completed, Some(CheckConclusion::Success)) => summary.passed += 1,
            (CheckStatus::Completed, Some(CheckConclusion::Skipped)) => summary.skipped += 1,
            (CheckStatus::Completed, Some(CheckConclusion::Neutral)) => summary.skipped += 1,
            (CheckStatus::Completed, Some(CheckConclusion::Cancelled)) => summary.failed += 1,
            (CheckStatus::Completed, Some(CheckConclusion::Failure)) => summary.failed += 1,
            (CheckStatus::Completed, Some(CheckConclusion::TimedOut)) => summary.failed += 1,
            (CheckStatus::Completed, Some(CheckConclusion::ActionRequired)) => summary.failed += 1,
            (CheckStatus::Completed, None) => summary.failed += 1,
            (CheckStatus::InProgress | CheckStatus::Queued, _) => summary.pending += 1,
        }
    }

    summary
}

fn check_group_name(check_name: &str) -> &str {
    check_name
        .split_once(" / ")
        .and_then(|(group_name, check_name)| {
            let group_name = group_name.trim();
            let check_name = check_name.trim();
            (!group_name.is_empty() && !check_name.is_empty()).then_some(group_name)
        })
        .unwrap_or("other checks")
}

fn check_run_display_name(check_run: &CheckRun) -> String {
    check_run
        .name
        .split_once(" / ")
        .and_then(|(group_name, check_name)| {
            let group_name = group_name.trim();
            let check_name = check_name.trim();
            (group_name == check_group_name(&check_run.name) && !check_name.is_empty())
                .then_some(check_name.to_string())
        })
        .unwrap_or_else(|| check_run.name.clone())
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

    render_panel_card()
        .id(("check-group", group_id))
        .flex()
        .items_center()
        .justify_between()
        .gap_3()
        .px_3()
        .py_2()
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
                .child(div().min_w_0().truncate().font_medium().child(group_name))
                .child(
                    div()
                        .flex_none()
                        .text_xs()
                        .text_color(color::text_muted())
                        .child(format!("{} checks", group.summary.total)),
                ),
        )
        .child(render_check_group_summary(group.summary))
}

fn render_check_group_summary(summary: ChecksSummary) -> impl IntoElement {
    let (label, tone) = if summary.failed > 0 {
        (format!("{} failed", summary.failed), Tone::Danger)
    } else if summary.pending > 0 {
        (format!("{} pending", summary.pending), Tone::Warning)
    } else if summary.passed > 0 {
        (format!("{} passed", summary.passed), Tone::Success)
    } else {
        (format!("{} skipped", summary.skipped), Tone::Neutral)
    };

    render_status_pill(label, tone)
}

pub(crate) fn render_check_run(check_index: usize, check_run: &CheckRun) -> impl IntoElement {
    let check_url = check_run
        .html_url
        .clone()
        .or_else(|| check_run.details_url.clone());
    let click_url = check_url.clone();
    let display_name = check_run_display_name(check_run);

    render_panel_card()
        .id(("check-run", check_index))
        .flex()
        .items_center()
        .justify_between()
        .gap_3()
        .overflow_hidden()
        .px_3()
        .py_2()
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
                .flex_col()
                .gap_1()
                .child(div().min_w_0().truncate().child(display_name))
                .child(
                    div()
                        .min_w_0()
                        .truncate()
                        .text_xs()
                        .text_color(color::text_muted())
                        .child(check_status_label(check_run.status)),
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

pub(crate) fn render_check_conclusion(
    conclusion: Option<CheckConclusion>,
    status: CheckStatus,
) -> impl IntoElement {
    let (label, tone) = check_conclusion_label(conclusion, status);

    render_status_pill(label, tone)
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

    use harbor_domain::{CheckConclusion, CheckRun, CheckStatus};

    use super::*;
    use crate::test_fixtures::check_run;

    #[test]
    fn groups_checks_by_name_prefix() {
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

        let rows = check_panel_rows(&checks, &HashSet::new());

        assert_eq!(
            row_labels(&rows),
            vec![
                "group:ci:2:open",
                "check:0",
                "check:1",
                "group:other checks:1:open",
                "check:3",
                "group:security:1:open",
                "check:2",
            ]
        );
    }

    #[test]
    fn hides_checks_for_collapsed_groups() {
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
        let collapsed_groups = HashSet::from(["ci".to_string()]);

        let rows = check_panel_rows(&checks, &collapsed_groups);

        assert_eq!(row_labels(&rows), vec!["group:ci:2:closed"]);
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
