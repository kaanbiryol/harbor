use crate::gpui_component::{Icon, Sizable, tooltip::Tooltip};
use gpui::{AnyElement, Context, IntoElement, div, prelude::*, px};
use harbor_domain::{ChecksSummary, PullRequest, PullRequestState};

use crate::{
    date_time::month_day_label,
    icons::Octicon,
    panels::pull_request_signals::{
        PullRequestRowRailTone, PullRequestRowSignal, pull_request_row_rail_tone,
        pull_request_row_signal,
    },
    visual::{Tone, color, layout, opacity, tone_colors},
    workspace::AppView,
};

fn render_row_signal(signal: PullRequestRowSignal) -> impl IntoElement {
    let (icon, tone, tooltip) = match signal {
        PullRequestRowSignal::MergeConflict => (Octicon::Alert, Tone::Danger, "Merge conflict"),
        PullRequestRowSignal::ChangesRequested => (
            Octicon::CommentDiscussion,
            Tone::Danger,
            "Changes requested",
        ),
        PullRequestRowSignal::ReviewNeeded => (Octicon::Eye, Tone::Warning, "Review required"),
    };

    render_status_indicator("pr-row-signal", icon, tone, tooltip)
}

fn render_status_indicator(
    id: &'static str,
    icon: Octicon,
    tone: Tone,
    tooltip: &'static str,
) -> impl IntoElement {
    let text_color = tone_colors(tone).text;

    div()
        .id(id)
        .flex_none()
        .h(px(16.))
        .w(px(16.))
        .flex()
        .items_center()
        .justify_center()
        .text_color(text_color)
        .tooltip(move |window, cx| Tooltip::new(tooltip).build(window, cx))
        .child(Icon::new(icon).xsmall().text_color(text_color))
}

fn row_rail_color(tone: PullRequestRowRailTone) -> Option<gpui::Rgba> {
    match tone {
        PullRequestRowRailTone::Neutral => None,
        PullRequestRowRailTone::Danger => Some(color::danger()),
        PullRequestRowRailTone::Warning => Some(color::warning()),
        PullRequestRowRailTone::Success => Some(color::success()),
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
    let (icon, tone, tooltip) = match indicator {
        PullRequestCheckIndicator::Failed => (Octicon::XCircle, Tone::Danger, "CI failed"),
        PullRequestCheckIndicator::Pending => (Octicon::Clock, Tone::Warning, "CI pending"),
        PullRequestCheckIndicator::Passed => (Octicon::Check, Tone::Success, "CI passed"),
    };

    render_status_indicator("pr-ci-status", icon, tone, tooltip)
}

fn pull_request_metadata_label(pr: &PullRequest) -> String {
    pr.created_at
        .as_ref()
        .or(pr.updated_at.as_ref())
        .map(|time| {
            format!(
                "#{} · {} · {}",
                pr.number,
                pr.author,
                month_day_label(*time)
            )
        })
        .unwrap_or_else(|| format!("#{} · {}", pr.number, pr.author))
}

fn pull_request_state_pill(state: PullRequestState) -> Option<(&'static str, Tone)> {
    match state {
        PullRequestState::Open => None,
        PullRequestState::Closed => Some(("closed", Tone::Neutral)),
        PullRequestState::Merged => Some(("merged", Tone::Success)),
    }
}

pub(crate) fn render_pull_request_row(
    index: usize,
    pr: &PullRequest,
    selected: bool,
    cx: &mut Context<AppView>,
) -> AnyElement {
    let signal = pull_request_row_signal(pr);
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
        .border_b_1()
        .border_color(color::border_subtle())
        .when(selected, |element| element.bg(color::row_selected_active()))
        .hover(move |style| {
            if selected {
                style
            } else {
                style.bg(color::row_hover())
            }
        })
        .on_click(cx.listener(move |view, _, _, cx| {
            view.select_pull_request(index, cx);
        }))
        .child(
            div()
                .h_full()
                .w(px(2.0))
                .flex_none()
                .when_some(rail_color, |element, color| element.bg(color)),
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
                        .w_full()
                        .min_w_0()
                        .flex()
                        .items_center()
                        .gap_2()
                        .text_sm()
                        .child(
                            div()
                                .min_w_0()
                                .flex_1()
                                .truncate()
                                .when(is_draft, |element| {
                                    element.opacity(opacity::DEEMPHASIZED_ITEM)
                                })
                                .child(pr.title.clone()),
                        )
                        .when(check_indicator.is_some() || signal.is_some(), |element| {
                            element.child(
                                div()
                                    .flex_none()
                                    .flex()
                                    .items_center()
                                    .gap_2()
                                    .when_some(check_indicator, |element, indicator| {
                                        element.child(render_check_indicator(indicator))
                                    })
                                    .when_some(signal, |element, signal| {
                                        element.child(render_row_signal(signal))
                                    }),
                            )
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
                                .when(is_draft, |element| {
                                    element.opacity(opacity::DEEMPHASIZED_ITEM)
                                })
                                .when_some(
                                    pull_request_state_pill(pr.state),
                                    |element, (label, tone)| {
                                        element.child(super::render_status_pill(label, tone))
                                    },
                                )
                                .child(div().min_w_0().flex_1().truncate().child(metadata_label)),
                        ),
                ),
        )
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ci_indicator_prioritizes_failure_then_pending_without_inventing_missing_status() {
        for (summary, expected) in [
            (ChecksSummary::default(), None),
            (
                ChecksSummary {
                    total: 1,
                    passed: 1,
                    ..Default::default()
                },
                Some(PullRequestCheckIndicator::Passed),
            ),
            (
                ChecksSummary {
                    total: 2,
                    passed: 1,
                    pending: 1,
                    ..Default::default()
                },
                Some(PullRequestCheckIndicator::Pending),
            ),
            (
                ChecksSummary {
                    total: 3,
                    passed: 1,
                    pending: 1,
                    failed: 1,
                    ..Default::default()
                },
                Some(PullRequestCheckIndicator::Failed),
            ),
        ] {
            assert_eq!(pull_request_check_indicator(summary), expected);
        }
    }

    #[test]
    fn open_pull_request_rows_do_not_show_state_pill() {
        assert_eq!(pull_request_state_pill(PullRequestState::Open), None);
    }

    #[test]
    fn closed_pull_request_rows_show_closed_state_pill() {
        assert_eq!(
            pull_request_state_pill(PullRequestState::Closed),
            Some(("closed", Tone::Neutral))
        );
    }

    #[test]
    fn merged_pull_request_rows_show_merged_state_pill() {
        assert_eq!(
            pull_request_state_pill(PullRequestState::Merged),
            Some(("merged", Tone::Success))
        );
    }
}
