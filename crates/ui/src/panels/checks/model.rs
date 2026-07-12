use std::collections::{BTreeMap, HashSet};

use harbor_domain::{CheckConclusion, CheckRun, CheckStatus, ChecksSummary};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) enum CheckRunFilter {
    #[default]
    All,
    Passed,
    Failed,
    Pending,
    Skipped,
}

impl CheckRunFilter {
    pub(super) fn id(self) -> usize {
        match self {
            Self::All => 0,
            Self::Passed => 1,
            Self::Failed => 2,
            Self::Pending => 3,
            Self::Skipped => 4,
        }
    }

    pub(super) fn status_label(self) -> &'static str {
        match self {
            Self::All => "total",
            Self::Passed => "passed",
            Self::Failed => "failed",
            Self::Pending => "pending",
            Self::Skipped => "skipped",
        }
    }

    pub(crate) fn status_message(self) -> &'static str {
        match self {
            Self::All => "Showing all checks",
            Self::Passed => "Showing passed checks",
            Self::Failed => "Showing failed checks",
            Self::Pending => "Showing pending checks",
            Self::Skipped => "Showing skipped checks",
        }
    }

    pub(super) fn empty_message(self) -> &'static str {
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
pub(super) struct CheckRunGroup {
    pub(super) name: String,
    pub(super) check_indices: Vec<usize>,
    pub(super) summary: ChecksSummary,
    pub(super) expanded: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum CheckPanelRow {
    Group(CheckRunGroup),
    Check { check_index: usize },
}

pub(super) fn check_panel_rows(
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

pub(super) fn check_run_display_name(check_run: &CheckRun) -> String {
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
