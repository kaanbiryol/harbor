use gpui::{AnyElement, Context, IntoElement, div, prelude::*, px, rgb};
use gpui_component::{
    Icon, IconName, Sizable, StyledExt,
    button::{Button, ButtonVariants},
};
use harbor_domain::{
    CheckConclusion, CheckRun, CheckStatus, ChecksSummary, DiffFile, FileStatus, MergeState,
    PullRequest, PullRequestState, ReviewDecision,
};

use crate::workspace::{AppView, ChangedFileFolderRow, ChangedFileRow, changed_file_status_label};

const CHANGED_FILE_TREE_ROW_HEIGHT: f32 = 44.;

pub(crate) fn render_checks_summary(summary: ChecksSummary) -> impl IntoElement {
    let color = if summary.failed > 0 {
        rgb(0xf87171)
    } else if summary.pending > 0 {
        rgb(0xfbbf24)
    } else {
        rgb(0x34d399)
    };

    div()
        .text_xs()
        .text_color(color)
        .child(format!("{}/{}", summary.passed, summary.total))
}

pub(crate) fn checks_summary_from_runs(check_runs: &[CheckRun]) -> ChecksSummary {
    let mut summary = ChecksSummary {
        total: check_runs.len(),
        ..ChecksSummary::default()
    };

    for check_run in check_runs {
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

pub(crate) fn review_action_blocker(pr: &PullRequest) -> Option<String> {
    if pr.state != PullRequestState::Open {
        Some(format!("PR #{} is not open", pr.number))
    } else {
        None
    }
}

pub(crate) fn merge_blocker(pr: &PullRequest) -> Option<String> {
    if pr.state != PullRequestState::Open {
        return Some(format!("PR #{} is not open", pr.number));
    }

    if pr.is_draft {
        return Some(format!("PR #{} is still a draft", pr.number));
    }

    if pr.head_sha.is_empty() {
        return Some(format!("PR #{} is missing a head SHA", pr.number));
    }

    match pr.merge_state {
        Some(MergeState::Clean) => {}
        Some(MergeState::Dirty) => {
            return Some(format!("PR #{} has merge conflicts", pr.number));
        }
        Some(MergeState::Blocked) => {
            return Some(format!("PR #{} is blocked by repository rules", pr.number));
        }
        Some(MergeState::Behind) => {
            return Some(format!("PR #{} is behind the base branch", pr.number));
        }
        Some(MergeState::Unknown) | None => {
            return Some(format!(
                "PR #{} is not confirmed mergeable by GitHub",
                pr.number
            ));
        }
    }

    if pr.checks_summary.failed > 0 {
        return Some(format!("PR #{} still has failing checks", pr.number));
    }

    if pr.checks_summary.pending > 0 {
        return Some(format!("PR #{} still has pending checks", pr.number));
    }

    if pr.unresolved_threads > 0 {
        return Some(format!(
            "PR #{} still has {} unresolved review threads",
            pr.number, pr.unresolved_threads
        ));
    }

    None
}

pub(crate) fn render_review_decision(decision: Option<ReviewDecision>) -> impl IntoElement {
    let label = match decision {
        Some(ReviewDecision::Approved) => "approved",
        Some(ReviewDecision::ChangesRequested) => "changes requested",
        Some(ReviewDecision::ReviewRequired) => "review required",
        None => "no review",
    };

    div().text_xs().text_color(rgb(0x93c5fd)).child(label)
}

pub(crate) fn render_merge_state(state: Option<MergeState>) -> impl IntoElement {
    let label = match state {
        Some(MergeState::Clean) => "mergeable",
        Some(MergeState::Dirty) => "dirty",
        Some(MergeState::Blocked) => "blocked",
        Some(MergeState::Behind) => "behind",
        Some(MergeState::Unknown) | None => "unknown",
    };

    div().text_xs().text_color(rgb(0x9aa4b2)).child(label)
}

pub(crate) fn render_pull_request_row(
    index: usize,
    pr: &PullRequest,
    selected: bool,
    cx: &mut Context<AppView>,
) -> AnyElement {
    div()
        .id(("pr-row", index))
        .h(px(76.))
        .w_full()
        .min_w_0()
        .flex()
        .flex_col()
        .justify_center()
        .overflow_hidden()
        .px_3()
        .py_2()
        .border_1()
        .border_color(rgb(0x20252b))
        .when(selected, |element| element.bg(rgb(0x243244)))
        .hover(|style| style.bg(rgb(0x202a35)))
        .on_click(cx.listener(move |view, _, _, cx| {
            view.select_pull_request(index, cx);
        }))
        .child(
            div()
                .flex()
                .w_full()
                .min_w_0()
                .justify_between()
                .items_center()
                .gap_2()
                .text_sm()
                .child(
                    div()
                        .min_w_0()
                        .flex_1()
                        .truncate()
                        .child(format!("#{} {}", pr.number, pr.title)),
                )
                .child(render_checks_summary(pr.checks_summary)),
        )
        .child(
            div()
                .pt_1()
                .text_xs()
                .text_color(rgb(0x9aa4b2))
                .truncate()
                .child(format!(
                    "{} into {} by {}",
                    pr.head_ref, pr.base_ref, pr.author
                )),
        )
        .into_any_element()
}

pub(crate) fn render_changed_folder_row(
    folder: &ChangedFileFolderRow,
    cx: &mut Context<AppView>,
) -> AnyElement {
    let folder_path = folder.path.clone();
    let chevron = if folder.expanded {
        IconName::ChevronDown
    } else {
        IconName::ChevronRight
    };
    let folder_icon = if folder.expanded {
        IconName::FolderOpen
    } else {
        IconName::FolderClosed
    };

    div()
        .id(format!("folder-row-{}", folder.path))
        .h(px(CHANGED_FILE_TREE_ROW_HEIGHT))
        .w_full()
        .min_w_0()
        .flex()
        .items_center()
        .overflow_hidden()
        .pl(file_tree_padding(folder.depth))
        .pr_3()
        .gap_2()
        .text_sm()
        .cursor_pointer()
        .hover(|style| style.bg(rgb(0x202a35)))
        .on_click(cx.listener(move |view, _, _, cx| {
            view.toggle_changed_file_folder(folder_path.clone(), cx);
        }))
        .child(Icon::new(chevron).xsmall().text_color(rgb(0x9aa4b2)))
        .child(Icon::new(folder_icon).xsmall().text_color(rgb(0x93c5fd)))
        .child(
            div()
                .min_w_0()
                .flex_1()
                .truncate()
                .font_medium()
                .text_color(rgb(0xd5dde7))
                .child(folder.name.clone()),
        )
        .child(
            div()
                .flex_none()
                .text_xs()
                .text_color(rgb(0x7d8794))
                .child(folder_review_summary(
                    folder.reviewed_file_count,
                    folder.file_count,
                )),
        )
        .into_any_element()
}

pub(crate) fn render_changed_file_row(
    row: &ChangedFileRow,
    file: &DiffFile,
    selected: bool,
    reviewed: bool,
    cx: &mut Context<AppView>,
) -> AnyElement {
    let index = row.file_index;
    let review_button = Button::new(format!("file-reviewed-{index}"))
        .icon(Icon::new(if reviewed {
            IconName::Check
        } else {
            IconName::Eye
        }))
        .small()
        .compact()
        .tooltip(if reviewed {
            "Mark as unreviewed"
        } else {
            "Mark as reviewed"
        });
    let review_button = if reviewed {
        review_button.primary()
    } else {
        review_button.ghost()
    };

    div()
        .id(("file-row", index))
        .h(px(CHANGED_FILE_TREE_ROW_HEIGHT))
        .w_full()
        .min_w_0()
        .flex()
        .items_center()
        .overflow_hidden()
        .pl(file_tree_padding(row.depth))
        .pr_2()
        .gap_2()
        .when(selected, |element| element.bg(rgb(0x243244)))
        .hover(|style| style.bg(rgb(0x202a35)))
        .on_click(cx.listener(move |view, _, _, cx| {
            view.select_file(index, cx);
        }))
        .child(
            div()
                .w(px(14.))
                .flex()
                .items_center()
                .justify_center()
                .child(Icon::new(IconName::File).xsmall().text_color(rgb(0x9aa4b2))),
        )
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
                        .text_color(if reviewed {
                            rgb(0x7d8794)
                        } else {
                            rgb(0xe6e8eb)
                        })
                        .child(row.name.clone()),
                )
                .child(
                    div()
                        .flex_none()
                        .truncate()
                        .text_xs()
                        .text_color(file_status_color(file.status))
                        .child(changed_file_status_label(file.status)),
                ),
        )
        .child(
            div()
                .flex_none()
                .flex()
                .items_center()
                .gap_1()
                .text_xs()
                .child(
                    div()
                        .text_color(diff_stat_color(file.additions, rgb(0x34d399)))
                        .child(format!("+{}", file.additions)),
                )
                .child(
                    div()
                        .text_color(diff_stat_color(file.deletions, rgb(0xf87171)))
                        .child(format!("-{}", file.deletions)),
                ),
        )
        .child(review_button.on_click(cx.listener(move |view, _, _, cx| {
            view.toggle_changed_file_reviewed(index, cx);
        })))
        .into_any_element()
}

fn folder_review_summary(reviewed_file_count: usize, file_count: usize) -> String {
    if reviewed_file_count == 0 {
        format!("{file_count}")
    } else {
        format!("{reviewed_file_count}/{file_count}")
    }
}

fn file_tree_padding(depth: usize) -> gpui::Pixels {
    px(10. + depth as f32 * 16.)
}

fn file_status_color(status: FileStatus) -> gpui::Rgba {
    match status {
        FileStatus::Added => rgb(0x34d399),
        FileStatus::Removed => rgb(0xf87171),
        FileStatus::Renamed | FileStatus::Copied => rgb(0x93c5fd),
        FileStatus::Modified | FileStatus::Changed => rgb(0xfbbf24),
        FileStatus::Unchanged => rgb(0x9aa4b2),
    }
}

fn diff_stat_color(count: u32, active_color: gpui::Rgba) -> gpui::Rgba {
    if count == 0 {
        rgb(0x9aa4b2)
    } else {
        active_color
    }
}

#[cfg(test)]
mod tests {
    use harbor_domain::{
        CheckConclusion, CheckRun, CheckStatus, ChecksSummary, MergeState, PullRequest,
        PullRequestState, RepoId,
    };

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
    fn allows_review_actions_for_open_pull_requests() {
        assert_eq!(review_action_blocker(&pull_request()), None);
    }

    #[test]
    fn blocks_merge_until_pull_request_is_ready() {
        let mut pr = pull_request();
        pr.checks_summary.pending = 1;

        assert_eq!(
            merge_blocker(&pr).as_deref(),
            Some("PR #7 still has pending checks")
        );

        pr.checks_summary.pending = 0;
        pr.unresolved_threads = 2;

        assert_eq!(
            merge_blocker(&pr).as_deref(),
            Some("PR #7 still has 2 unresolved review threads")
        );
    }

    #[test]
    fn allows_clean_pull_request_merge() {
        assert_eq!(merge_blocker(&pull_request()), None);
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

    fn pull_request() -> PullRequest {
        PullRequest {
            repo: RepoId::new("acme", "app"),
            node_id: "pr-node".to_string(),
            number: 7,
            title: "Add feature".to_string(),
            body: None,
            author: "octocat".to_string(),
            url: "https://github.com/acme/app/pull/7".to_string(),
            state: PullRequestState::Open,
            is_draft: false,
            head_ref: "feature".to_string(),
            base_ref: "main".to_string(),
            head_sha: "abc123".to_string(),
            review_decision: None,
            merge_state: Some(MergeState::Clean),
            labels: Vec::new(),
            checks_summary: ChecksSummary {
                total: 1,
                passed: 1,
                failed: 0,
                pending: 0,
                skipped: 0,
            },
            unresolved_threads: 0,
        }
    }
}
