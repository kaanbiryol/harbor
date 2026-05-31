use gpui::{AnyElement, Context, IntoElement, div, prelude::*, px};
use gpui_component::{
    Icon, IconName, Sizable, StyledExt,
    button::{Button, ButtonVariants},
};
use harbor_domain::{DiffFile, FileStatus, MergeState, PullRequest, ReviewDecision};

use crate::{
    panels::pull_request_signals::{
        PullRequestRowRailTone, PullRequestRowSignal, PullRequestRowSignalKind,
        PullRequestRowSignalTone, pull_request_row_rail_tone, visible_pull_request_row_signals,
    },
    visual::{Tone, color, tone_colors, tone_text},
    workspace::{AppView, ChangedFileFolderRow, ChangedFileRow, changed_file_status_label},
};

const CHANGED_FILE_TREE_ROW_HEIGHT: f32 = 44.;

pub(crate) fn render_review_decision(decision: Option<ReviewDecision>) -> impl IntoElement {
    let label = match decision {
        Some(ReviewDecision::Approved) => "approved",
        Some(ReviewDecision::ChangesRequested) => "changes requested",
        Some(ReviewDecision::ReviewRequired) => "review required",
        None => "no review",
    };

    let tone = match decision {
        Some(ReviewDecision::Approved) => Tone::Success,
        Some(ReviewDecision::ChangesRequested) => Tone::Danger,
        Some(ReviewDecision::ReviewRequired) => Tone::Warning,
        None => Tone::Info,
    };

    div().text_xs().text_color(tone_text(tone)).child(label)
}

pub(crate) fn render_merge_state(state: Option<MergeState>) -> impl IntoElement {
    let label = match state {
        Some(MergeState::Clean) => "mergeable",
        Some(MergeState::Dirty) => "dirty",
        Some(MergeState::Blocked) => "blocked",
        Some(MergeState::Behind) => "behind",
        Some(MergeState::Unknown) | None => "unknown",
    };

    let tone = match state {
        Some(MergeState::Clean) => Tone::Success,
        Some(MergeState::Dirty) | Some(MergeState::Blocked) => Tone::Danger,
        Some(MergeState::Behind) => Tone::Warning,
        Some(MergeState::Unknown) | None => Tone::Neutral,
    };

    div().text_xs().text_color(tone_text(tone)).child(label)
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
        PullRequestRowSignalKind::ChecksPassed | PullRequestRowSignalKind::ReviewApproved => {
            PullRequestRowSignalTone::Success
        }
    }
}

fn row_signal_tone_colors(tone: PullRequestRowSignalTone) -> (gpui::Rgba, gpui::Rgba, gpui::Rgba) {
    let tone = match tone {
        PullRequestRowSignalTone::Danger => Tone::Danger,
        PullRequestRowSignalTone::Warning => Tone::Warning,
        PullRequestRowSignalTone::Success => Tone::Success,
    };
    let colors = tone_colors(tone);

    (colors.text, colors.border, colors.background)
}

fn row_rail_color(tone: PullRequestRowRailTone) -> gpui::Rgba {
    match tone {
        PullRequestRowRailTone::Neutral => color::border(),
        PullRequestRowRailTone::Danger => color::danger(),
        PullRequestRowRailTone::Warning => color::warning(),
        PullRequestRowRailTone::Success => color::success(),
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
    let rail_color = row_rail_color(pull_request_row_rail_tone(pr));

    div()
        .id(("pr-row", index))
        .h(px(76.))
        .w_full()
        .min_w_0()
        .flex()
        .overflow_hidden()
        .border_1()
        .border_color(color::border_subtle())
        .when(pr.is_draft, |element| element.opacity(0.72))
        .when(selected, |element| element.bg(color::row_selected()))
        .hover(|style| style.bg(color::row_hover()))
        .on_click(cx.listener(move |view, _, _, cx| {
            view.select_pull_request(index, cx);
        }))
        .child(div().h_full().w(px(3.)).flex_none().bg(rail_color))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .flex_col()
                .justify_center()
                .overflow_hidden()
                .px_3()
                .py_2()
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
                                .flex()
                                .items_center()
                                .gap_1()
                                .child(
                                    div()
                                        .flex_none()
                                        .text_color(color::text_muted())
                                        .child(format!("#{}", pr.number)),
                                )
                                .child(div().min_w_0().flex_1().truncate().child(pr.title.clone())),
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
                                .text_color(color::text_secondary())
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
        .hover(|style| style.bg(color::row_hover()))
        .on_click(cx.listener(move |view, _, _, cx| {
            view.toggle_changed_file_folder(folder_path.clone(), cx);
        }))
        .child(Icon::new(chevron).xsmall().text_color(color::text_muted()))
        .child(Icon::new(folder_icon).xsmall().text_color(color::accent()))
        .child(
            div()
                .min_w_0()
                .flex_1()
                .truncate()
                .font_medium()
                .text_color(color::text_primary())
                .child(folder.name.clone()),
        )
        .child(
            div()
                .flex_none()
                .text_xs()
                .text_color(color::text_muted())
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
        .when(selected, |element| element.bg(color::row_selected()))
        .hover(|style| style.bg(color::row_hover()))
        .on_click(cx.listener(move |view, _, _, cx| {
            view.select_file(index, cx);
        }))
        .child(
            div()
                .w(px(14.))
                .flex()
                .items_center()
                .justify_center()
                .child(
                    Icon::new(IconName::File)
                        .xsmall()
                        .text_color(color::text_muted()),
                ),
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
                            color::text_muted()
                        } else {
                            color::text_primary()
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
                        .text_color(diff_stat_color(file.additions, color::success()))
                        .child(format!("+{}", file.additions)),
                )
                .child(
                    div()
                        .text_color(diff_stat_color(file.deletions, color::danger()))
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
        FileStatus::Added => color::success(),
        FileStatus::Removed => color::danger(),
        FileStatus::Renamed | FileStatus::Copied => color::accent(),
        FileStatus::Modified | FileStatus::Changed => color::warning(),
        FileStatus::Unchanged => color::text_muted(),
    }
}

fn diff_stat_color(count: u32, active_color: gpui::Rgba) -> gpui::Rgba {
    if count == 0 {
        color::text_muted()
    } else {
        active_color
    }
}
