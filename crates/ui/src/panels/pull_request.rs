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
const MAX_PULL_REQUEST_ROW_SIGNALS: usize = 3;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PullRequestRowSignalTone {
    Danger,
    Warning,
    Success,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PullRequestRowSignalKind {
    Conflict,
    ChecksFailed,
    ChecksRunning,
    ChecksPassed,
    ReviewApproved,
    ReviewChangesRequested,
    ReviewNeeded,
    UnresolvedThreads,
    Ready,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct PullRequestRowSignal {
    kind: PullRequestRowSignalKind,
    label: Option<String>,
}

impl PullRequestRowSignal {
    fn new(kind: PullRequestRowSignalKind) -> Self {
        Self { kind, label: None }
    }

    fn with_label(kind: PullRequestRowSignalKind, label: impl Into<String>) -> Self {
        Self {
            kind,
            label: Some(label.into()),
        }
    }
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

fn visible_pull_request_row_signals(pr: &PullRequest) -> Vec<PullRequestRowSignal> {
    pull_request_row_signals(pr)
        .into_iter()
        .take(MAX_PULL_REQUEST_ROW_SIGNALS)
        .collect()
}

fn pull_request_row_signals(pr: &PullRequest) -> Vec<PullRequestRowSignal> {
    if pr.merge_state == Some(MergeState::Dirty) {
        return vec![PullRequestRowSignal::with_label(
            PullRequestRowSignalKind::Conflict,
            "conflict",
        )];
    }

    let mut action_signals = Vec::new();

    if let Some(signal) = action_checks_signal(pr.checks_summary) {
        action_signals.push(signal);
    }

    match pr.review_decision {
        Some(ReviewDecision::ChangesRequested) => {
            action_signals.push(PullRequestRowSignal::with_label(
                PullRequestRowSignalKind::ReviewChangesRequested,
                "changes",
            ))
        }
        Some(ReviewDecision::ReviewRequired) => {
            action_signals.push(PullRequestRowSignal::new(
                PullRequestRowSignalKind::ReviewNeeded,
            ));
        }
        Some(ReviewDecision::Approved) | None => {}
    }

    if pr.unresolved_threads > 0 {
        action_signals.push(PullRequestRowSignal::with_label(
            PullRequestRowSignalKind::UnresolvedThreads,
            pr.unresolved_threads.to_string(),
        ));
    }

    if !action_signals.is_empty() {
        return action_signals;
    }

    let mut quiet_signals = Vec::new();

    if is_ready_to_merge(pr) {
        quiet_signals.push(PullRequestRowSignal::new(PullRequestRowSignalKind::Ready));
    }

    if pr.review_decision == Some(ReviewDecision::Approved) {
        quiet_signals.push(PullRequestRowSignal::new(
            PullRequestRowSignalKind::ReviewApproved,
        ));
    }

    if quiet_signals.is_empty()
        && let Some(signal) = quiet_checks_signal(pr.checks_summary)
    {
        quiet_signals.push(signal);
    }

    quiet_signals
}

fn action_checks_signal(summary: ChecksSummary) -> Option<PullRequestRowSignal> {
    if summary.failed > 0 {
        Some(PullRequestRowSignal::with_label(
            PullRequestRowSignalKind::ChecksFailed,
            summary.failed.to_string(),
        ))
    } else if summary.pending > 0 {
        Some(PullRequestRowSignal::with_label(
            PullRequestRowSignalKind::ChecksRunning,
            summary.pending.to_string(),
        ))
    } else {
        None
    }
}

fn quiet_checks_signal(summary: ChecksSummary) -> Option<PullRequestRowSignal> {
    if summary.total == 0 {
        None
    } else if summary.passed == summary.total {
        Some(PullRequestRowSignal::new(
            PullRequestRowSignalKind::ChecksPassed,
        ))
    } else {
        Some(PullRequestRowSignal::with_label(
            PullRequestRowSignalKind::ChecksPassed,
            format!("{}/{}", summary.passed, summary.total),
        ))
    }
}

fn is_ready_to_merge(pr: &PullRequest) -> bool {
    !pr.is_draft
        && pr.merge_state == Some(MergeState::Clean)
        && pr.checks_summary.total > 0
        && pr.checks_summary.failed == 0
        && pr.checks_summary.pending == 0
        && pr.unresolved_threads == 0
        && !matches!(
            pr.review_decision,
            Some(ReviewDecision::ChangesRequested | ReviewDecision::ReviewRequired)
        )
}

fn render_row_signal(signal: PullRequestRowSignal) -> impl IntoElement {
    let has_label = signal.label.is_some();
    let (text_color, border_color, background_color) =
        row_signal_tone_colors(row_signal_tone(signal.kind));

    div()
        .flex_none()
        .h(px(24.))
        .min_w(px(24.))
        .max_w(px(96.))
        .flex()
        .items_center()
        .justify_center()
        .gap_1()
        .rounded_xs()
        .px_1()
        .text_xs()
        .font_medium()
        .text_color(text_color)
        .when(has_label, |element| {
            element
                .border_1()
                .border_color(border_color)
                .bg(background_color)
        })
        .child(
            Icon::new(row_signal_icon(signal.kind))
                .xsmall()
                .text_color(text_color),
        )
        .when_some(signal.label, |element, label| {
            element.child(div().truncate().child(label))
        })
}

fn row_signal_icon(kind: PullRequestRowSignalKind) -> IconName {
    match kind {
        PullRequestRowSignalKind::Conflict => IconName::TriangleAlert,
        PullRequestRowSignalKind::ChecksFailed => IconName::CircleX,
        PullRequestRowSignalKind::ChecksRunning => IconName::LoaderCircle,
        PullRequestRowSignalKind::ChecksPassed => IconName::Check,
        PullRequestRowSignalKind::ReviewApproved => IconName::ThumbsUp,
        PullRequestRowSignalKind::ReviewChangesRequested => IconName::ThumbsDown,
        PullRequestRowSignalKind::ReviewNeeded => IconName::Eye,
        PullRequestRowSignalKind::UnresolvedThreads => IconName::Info,
        PullRequestRowSignalKind::Ready => IconName::CircleCheck,
    }
}

fn row_signal_tone(kind: PullRequestRowSignalKind) -> PullRequestRowSignalTone {
    match kind {
        PullRequestRowSignalKind::Conflict
        | PullRequestRowSignalKind::ChecksFailed
        | PullRequestRowSignalKind::ReviewChangesRequested => PullRequestRowSignalTone::Danger,
        PullRequestRowSignalKind::ChecksRunning
        | PullRequestRowSignalKind::ReviewNeeded
        | PullRequestRowSignalKind::UnresolvedThreads => PullRequestRowSignalTone::Warning,
        PullRequestRowSignalKind::ChecksPassed
        | PullRequestRowSignalKind::ReviewApproved
        | PullRequestRowSignalKind::Ready => PullRequestRowSignalTone::Success,
    }
}

fn row_signal_tone_colors(tone: PullRequestRowSignalTone) -> (gpui::Rgba, gpui::Rgba, gpui::Rgba) {
    match tone {
        PullRequestRowSignalTone::Danger => (rgb(0xfca5a5), rgb(0x7f1d1d), rgb(0x2a1214)),
        PullRequestRowSignalTone::Warning => (rgb(0xfcd34d), rgb(0x713f12), rgb(0x241a0c)),
        PullRequestRowSignalTone::Success => (rgb(0x86efac), rgb(0x14532d), rgb(0x102016)),
    }
}

pub(crate) fn render_pull_request_row(
    index: usize,
    pr: &PullRequest,
    selected: bool,
    cx: &mut Context<AppView>,
) -> AnyElement {
    let signals = visible_pull_request_row_signals(pr);
    let primary_signal = signals.first().cloned();
    let secondary_signals = signals.iter().skip(1).cloned().collect::<Vec<_>>();

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
        .when(pr.is_draft, |element| element.opacity(0.72))
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
                .when_some(primary_signal, |element, signal| {
                    element.child(render_row_signal(signal))
                }),
        )
        .child(
            div()
                .pt_1()
                .flex()
                .w_full()
                .min_w_0()
                .items_center()
                .justify_between()
                .gap_2()
                .text_xs()
                .child(
                    div()
                        .min_w_0()
                        .flex_1()
                        .truncate()
                        .text_color(rgb(0x9aa4b2))
                        .child(format!(
                            "{} into {} by {}",
                            pr.head_ref, pr.base_ref, pr.author
                        )),
                )
                .child(
                    div()
                        .flex_none()
                        .flex()
                        .items_center()
                        .gap_1()
                        .children(secondary_signals.into_iter().map(render_row_signal)),
                ),
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
    use harbor_domain::{CheckConclusion, CheckStatus, ReviewDecision};

    use super::*;
    use crate::test_fixtures::{check_run, pull_request};

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
    fn shows_missing_checks_without_no_review_noise() {
        let mut pr = pull_request();
        pr.checks_summary = ChecksSummary::default();

        let signals = visible_pull_request_row_signals(&pr);

        assert!(signals.is_empty());
    }

    #[test]
    fn prioritizes_action_signals_without_draft_text() {
        let mut pr = pull_request();
        pr.is_draft = true;
        pr.checks_summary = ChecksSummary {
            total: 4,
            passed: 2,
            failed: 1,
            pending: 1,
            skipped: 0,
        };
        pr.review_decision = Some(ReviewDecision::ChangesRequested);
        pr.unresolved_threads = 2;

        let signals = visible_pull_request_row_signals(&pr);

        assert_eq!(
            signal_summary(&signals),
            vec![
                (
                    PullRequestRowSignalKind::ChecksFailed,
                    Some("1".to_string())
                ),
                (
                    PullRequestRowSignalKind::ReviewChangesRequested,
                    Some("changes".to_string())
                ),
                (
                    PullRequestRowSignalKind::UnresolvedThreads,
                    Some("2".to_string())
                )
            ]
        );
    }

    #[test]
    fn shows_unresolved_threads_without_no_review_noise() {
        let mut pr = pull_request();
        pr.unresolved_threads = 2;

        let signals = visible_pull_request_row_signals(&pr);

        assert_eq!(
            signal_summary(&signals),
            vec![(
                PullRequestRowSignalKind::UnresolvedThreads,
                Some("2".to_string())
            )]
        );
    }

    #[test]
    fn shows_conflict_as_the_only_row_signal() {
        let mut pr = pull_request();
        pr.merge_state = Some(MergeState::Dirty);
        pr.review_decision = Some(ReviewDecision::Approved);
        pr.unresolved_threads = 2;

        let signals = visible_pull_request_row_signals(&pr);

        assert_eq!(
            signal_summary(&signals),
            vec![(
                PullRequestRowSignalKind::Conflict,
                Some("conflict".to_string())
            )]
        );
    }

    #[test]
    fn shows_ready_and_approved_without_check_text() {
        let mut pr = pull_request();
        pr.review_decision = Some(ReviewDecision::Approved);

        let signals = visible_pull_request_row_signals(&pr);

        assert_eq!(
            signal_summary(&signals),
            vec![
                (PullRequestRowSignalKind::Ready, None),
                (PullRequestRowSignalKind::ReviewApproved, None)
            ]
        );
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

    fn signal_summary(
        signals: &[PullRequestRowSignal],
    ) -> Vec<(PullRequestRowSignalKind, Option<String>)> {
        signals
            .iter()
            .map(|signal| (signal.kind, signal.label.clone()))
            .collect()
    }
}
