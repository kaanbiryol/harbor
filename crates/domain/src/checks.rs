use std::collections::{BTreeMap, HashSet};

use crate::{CheckConclusion, CheckRun, CheckStatus, ChecksSummary};

pub fn checks_summary_from_runs(check_runs: &[CheckRun]) -> ChecksSummary {
    summarize_check_runs(check_runs)
}

fn summarize_check_runs<'a>(check_runs: impl IntoIterator<Item = &'a CheckRun>) -> ChecksSummary {
    let mut summary = ChecksSummary::default();

    for check_run in check_runs {
        summary.total += 1;
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

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum CheckRunFilter {
    #[default]
    All,
    Passed,
    Failed,
    Pending,
    Skipped,
}

impl CheckRunFilter {
    pub fn id(self) -> usize {
        match self {
            Self::All => 0,
            Self::Passed => 1,
            Self::Failed => 2,
            Self::Pending => 3,
            Self::Skipped => 4,
        }
    }

    pub fn status_label(self) -> &'static str {
        match self {
            Self::All => "total",
            Self::Passed => "passed",
            Self::Failed => "failed",
            Self::Pending => "pending",
            Self::Skipped => "skipped",
        }
    }

    pub fn status_message(self) -> &'static str {
        match self {
            Self::All => "Showing all checks",
            Self::Passed => "Showing passed checks",
            Self::Failed => "Showing failed checks",
            Self::Pending => "Showing pending checks",
            Self::Skipped => "Showing skipped checks",
        }
    }

    pub fn empty_message(self) -> &'static str {
        match self {
            Self::All => "No check runs found for this PR head",
            Self::Passed => "No passed check runs",
            Self::Failed => "No failed check runs",
            Self::Pending => "No pending check runs",
            Self::Skipped => "No skipped check runs",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckRunGroup {
    pub name: String,
    pub check_indices: Vec<usize>,
    pub summary: ChecksSummary,
    pub expanded: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CheckPanelRow {
    Group(CheckRunGroup),
    Check { check_index: usize },
}

pub fn check_panel_rows(
    check_runs: &[CheckRun],
    expanded_groups: &HashSet<String>,
    active_filter: CheckRunFilter,
) -> Vec<CheckPanelRow> {
    let mut rows = Vec::new();

    for group in check_run_groups(check_runs, expanded_groups, active_filter) {
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
    expanded_groups: &HashSet<String>,
    active_filter: CheckRunFilter,
) -> Vec<CheckRunGroup> {
    let mut indices_by_group = BTreeMap::<String, Vec<usize>>::new();

    for (index, check_run) in check_runs.iter().enumerate() {
        if !check_run_matches_filter(check_run, active_filter) {
            continue;
        }

        indices_by_group
            .entry(check_group_name(&check_run.name).to_string())
            .or_default()
            .push(index);
    }

    indices_by_group
        .into_iter()
        .map(|(name, check_indices)| {
            let summary = checks_summary_for_indices(check_runs, &check_indices);
            let expanded = expanded_groups.contains(&name);

            CheckRunGroup {
                name,
                check_indices,
                summary,
                expanded,
            }
        })
        .collect()
}

fn check_run_matches_filter(check_run: &CheckRun, active_filter: CheckRunFilter) -> bool {
    active_filter == CheckRunFilter::All || check_run_filter(check_run) == active_filter
}

fn check_run_filter(check_run: &CheckRun) -> CheckRunFilter {
    match (check_run.status, check_run.conclusion) {
        (CheckStatus::Completed, Some(CheckConclusion::Success)) => CheckRunFilter::Passed,
        (CheckStatus::Completed, Some(CheckConclusion::Skipped | CheckConclusion::Neutral)) => {
            CheckRunFilter::Skipped
        }
        (
            CheckStatus::Completed,
            Some(
                CheckConclusion::Cancelled
                | CheckConclusion::Failure
                | CheckConclusion::TimedOut
                | CheckConclusion::ActionRequired,
            )
            | None,
        ) => CheckRunFilter::Failed,
        (CheckStatus::InProgress | CheckStatus::Queued, _) => CheckRunFilter::Pending,
    }
}

fn checks_summary_for_indices(check_runs: &[CheckRun], check_indices: &[usize]) -> ChecksSummary {
    summarize_check_runs(
        check_indices
            .iter()
            .filter_map(|index| check_runs.get(*index)),
    )
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

pub fn check_run_display_name(check_run: &CheckRun) -> String {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn summarizes_check_runs() {
        let check_runs = vec![
            check_run(CheckStatus::Completed, Some(CheckConclusion::Success)),
            check_run(CheckStatus::Completed, Some(CheckConclusion::Failure)),
            check_run(CheckStatus::Completed, Some(CheckConclusion::Skipped)),
            check_run(CheckStatus::InProgress, None),
        ];

        let summary = checks_summary_from_runs(&check_runs);

        assert_eq!(summary.total, 4);
        assert_eq!(summary.passed, 1);
        assert_eq!(summary.failed, 1);
        assert_eq!(summary.skipped, 1);
        assert_eq!(summary.pending, 1);
    }

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

    fn check_run(status: CheckStatus, conclusion: Option<CheckConclusion>) -> CheckRun {
        CheckRun {
            id: None,
            name: "check".to_string(),
            status,
            conclusion,
            details_url: None,
            html_url: None,
            started_at: None,
            completed_at: None,
        }
    }
}
