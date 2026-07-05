use gpui::{AnyElement, Context, IntoElement, div, prelude::*, px};
use gpui_component::{Icon, Sizable, StyledExt};
use harbor_domain::{MergeState, PullRequest, ReviewDecision};

use crate::{
    icons::Octicon,
    panels::pull_request_signals::{
        PullRequestRowRailTone, PullRequestRowSignal, PullRequestRowSignalKind,
        PullRequestRowSignalTone, pull_request_row_rail_tone, visible_pull_request_row_signals,
    },
    visual::{Tone, color, tone_colors},
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

fn row_signal_icon(kind: PullRequestRowSignalKind) -> Octicon {
    match kind {
        PullRequestRowSignalKind::Conflict => Octicon::Alert,
        PullRequestRowSignalKind::ChecksFailed => Octicon::XCircle,
        PullRequestRowSignalKind::ChecksRunning => Octicon::Clock,
        PullRequestRowSignalKind::ChecksPassed => Octicon::Check,
        PullRequestRowSignalKind::ReviewApproved => Octicon::ThumbsUp,
        PullRequestRowSignalKind::ReviewChangesRequested => Octicon::ThumbsDown,
        PullRequestRowSignalKind::ReviewNeeded => Octicon::Eye,
        PullRequestRowSignalKind::UnresolvedThreads => Octicon::CommentDiscussion,
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
    let rail_color = if selected {
        color::accent()
    } else {
        row_rail_color(pull_request_row_rail_tone(pr))
    };

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
        .when(selected, |element| element.bg(color::row_selected_active()))
        .hover(|style| style.bg(color::row_hover()))
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
