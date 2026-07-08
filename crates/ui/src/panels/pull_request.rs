use gpui::{AnyElement, Context, IntoElement, div, prelude::*, px};
use gpui_component::{Icon, Sizable, tooltip::Tooltip};
use harbor_domain::{ChecksSummary, MergeState, PullRequest, ReviewDecision};

use crate::{
    date_time::month_day_label,
    icons::Octicon,
    panels::pull_request_signals::{
        PullRequestRowRailTone, PullRequestRowSignal, PullRequestRowSignalKind,
        PullRequestRowSignalTone, pull_request_row_rail_tone, visible_pull_request_row_signals,
    },
    visual::{Tone, color, layout, opacity, tone_colors},
    workspace::AppView,
};

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

    super::render_status_pill(label, tone)
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

    super::render_status_pill(label, tone)
}

fn render_row_signal(signal: PullRequestRowSignal) -> impl IntoElement {
    let (text_color, border_color, background_color) =
        row_signal_tone_colors(row_signal_tone(signal.kind));
    let tooltip = row_signal_tooltip(&signal);

    div()
        .id(row_signal_id(signal.kind))
        .flex_none()
        .h(px(24.))
        .w(px(24.))
        .flex()
        .items_center()
        .justify_center()
        .rounded_xs()
        .border_1()
        .border_color(border_color)
        .bg(background_color)
        .text_color(text_color)
        .tooltip(move |window, cx| Tooltip::new(tooltip.clone()).build(window, cx))
        .child(
            Icon::new(row_signal_icon(signal.kind))
                .xsmall()
                .text_color(text_color),
        )
}

fn row_signal_id(kind: PullRequestRowSignalKind) -> &'static str {
    match kind {
        PullRequestRowSignalKind::MergeConflict => "pr-signal-merge-conflict",
        PullRequestRowSignalKind::ReviewApproved => "pr-signal-review-approved",
        PullRequestRowSignalKind::ReviewChangesRequestedThreads => {
            "pr-signal-review-changes-requested"
        }
        PullRequestRowSignalKind::ReviewNeeded => "pr-signal-review-needed",
        PullRequestRowSignalKind::UnresolvedThreads => "pr-signal-unresolved-threads",
    }
}

fn row_signal_icon(kind: PullRequestRowSignalKind) -> Octicon {
    match kind {
        PullRequestRowSignalKind::MergeConflict => Octicon::Alert,
        PullRequestRowSignalKind::ReviewApproved => Octicon::ThumbsUp,
        PullRequestRowSignalKind::ReviewChangesRequestedThreads => Octicon::CommentDiscussion,
        PullRequestRowSignalKind::ReviewNeeded => Octicon::Eye,
        PullRequestRowSignalKind::UnresolvedThreads => Octicon::CommentDiscussion,
    }
}

fn row_signal_tooltip(signal: &PullRequestRowSignal) -> String {
    match signal.kind {
        PullRequestRowSignalKind::MergeConflict => "Merge conflict".to_string(),
        PullRequestRowSignalKind::ReviewApproved => "Review approved".to_string(),
        PullRequestRowSignalKind::ReviewChangesRequestedThreads => signal
            .label
            .as_deref()
            .filter(|label| label.chars().all(|character| character.is_ascii_digit()))
            .map(|count| format!("{count} unresolved review threads with changes requested"))
            .unwrap_or_else(|| "Changes requested".to_string()),
        PullRequestRowSignalKind::ReviewNeeded => "Review required".to_string(),
        PullRequestRowSignalKind::UnresolvedThreads => signal
            .label
            .as_deref()
            .map(|count| format!("{count} unresolved review threads"))
            .unwrap_or_else(|| "Unresolved review threads".to_string()),
    }
}

fn row_signal_tone(kind: PullRequestRowSignalKind) -> PullRequestRowSignalTone {
    match kind {
        PullRequestRowSignalKind::MergeConflict
        | PullRequestRowSignalKind::ReviewChangesRequestedThreads => {
            PullRequestRowSignalTone::Danger
        }
        PullRequestRowSignalKind::ReviewNeeded | PullRequestRowSignalKind::UnresolvedThreads => {
            PullRequestRowSignalTone::Warning
        }
        PullRequestRowSignalKind::ReviewApproved => PullRequestRowSignalTone::Success,
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PullRequestCheckIndicator {
    Failed,
    Pending,
    Passed,
}

fn pull_request_check_indicator(summary: ChecksSummary) -> Option<PullRequestCheckIndicator> {
    if summary.failed > 0 {
        Some(PullRequestCheckIndicator::Failed)
    } else if summary.pending > 0 {
        Some(PullRequestCheckIndicator::Pending)
    } else if summary.total > 0 {
        Some(PullRequestCheckIndicator::Passed)
    } else {
        None
    }
}

fn render_check_indicator(indicator: PullRequestCheckIndicator) -> impl IntoElement {
    let (icon, tone) = match indicator {
        PullRequestCheckIndicator::Failed => (Octicon::XCircle, Tone::Danger),
        PullRequestCheckIndicator::Pending => (Octicon::Clock, Tone::Warning),
        PullRequestCheckIndicator::Passed => (Octicon::Check, Tone::Success),
    };

    Icon::new(icon).xsmall().text_color(tone_colors(tone).text)
}

fn pull_request_metadata_label(pr: &PullRequest) -> String {
    pr.created_at
        .as_ref()
        .or(pr.updated_at.as_ref())
        .map(|time| format!("{} by {}", month_day_label(*time), pr.author))
        .unwrap_or_else(|| format!("by {}", pr.author))
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
    let is_draft = pr.is_draft;
    let check_indicator = pull_request_check_indicator(pr.checks_summary);
    let metadata_label = pull_request_metadata_label(pr);
    let rail_color = row_rail_color(pull_request_row_rail_tone(pr));

    div()
        .id(("pr-row", index))
        .h(px(layout::PULL_REQUEST_ROW_HEIGHT))
        .w_full()
        .min_w_0()
        .flex()
        .overflow_hidden()
        .border_1()
        .border_color(color::border_subtle())
        .when(is_draft, |element| {
            element.opacity(opacity::DEEMPHASIZED_ITEM)
        })
        .when(selected, |element| element.bg(color::row_selected_active()))
        .hover(move |style| {
            let style = style.bg(color::row_hover());
            if is_draft {
                style.opacity(opacity::DEEMPHASIZED_ITEM_HOVER)
            } else {
                style
            }
        })
        .on_click(cx.listener(move |view, _, _, cx| {
            view.select_pull_request(index, cx);
        }))
        .child(
            div()
                .h_full()
                .w(px(if selected { 3.0 } else { 2.0 }))
                .flex_none()
                .bg(rail_color),
        )
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
                                .child(div().min_w_0().flex_1().truncate().child(pr.title.clone()))
                                .when_some(check_indicator, |element, indicator| {
                                    element.child(render_check_indicator(indicator))
                                }),
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
                                .flex()
                                .items_center()
                                .gap_2()
                                .text_color(color::text_secondary())
                                .child(div().min_w_0().flex_1().truncate().child(metadata_label)),
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
