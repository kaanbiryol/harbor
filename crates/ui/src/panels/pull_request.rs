use gpui::{AnyElement, Context, IntoElement, div, prelude::*, px, rgb};
use harbor_domain::{
    CheckConclusion, CheckRun, CheckStatus, ChecksSummary, DiffFile, MergeState, PullRequest,
    PullRequestState, ReviewDecision,
};

use crate::workspace::AppView;

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

pub(crate) fn render_changed_file_row(
    index: usize,
    file: &DiffFile,
    selected: bool,
    cx: &mut Context<AppView>,
) -> AnyElement {
    div()
        .id(("file-row", index))
        .h(px(72.))
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
            view.select_file(index, cx);
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
                .child(div().min_w_0().flex_1().truncate().child(file.path.clone()))
                .child(
                    div()
                        .flex_none()
                        .child(format!("+{} -{}", file.additions, file.deletions)),
                ),
        )
        .child(
            div()
                .pt_1()
                .text_xs()
                .text_color(rgb(0x9aa4b2))
                .child(format!("{:?}", file.status)),
        )
        .into_any_element()
}
