use crate::gpui_component::{
    Sizable,
    button::{Button, ButtonVariants},
    progress::ProgressCircle,
    status_bar::StatusBar,
    tooltip::Tooltip,
};
use gpui::{Context, IntoElement, div, prelude::*};

use crate::{
    actions::{KEY_CONTEXT, TogglePullRequestInbox},
    icons::Octicon,
    visual::color,
    workspace::AppView,
};

use super::rate_limits::{
    GitHubRateLimitIndicator, github_rate_limit_indicator, github_rate_limit_indicator_color,
};

const SHOW_STATUS_BAR_RATE_LIMITS: bool = true;

impl AppView {
    pub(super) fn render_status_bar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let inbox_toggle_icon = if self.pull_request_inbox.is_visible() {
            Octicon::SidebarCollapse
        } else {
            Octicon::SidebarExpand
        };
        let inbox_toggle_tooltip = if self.pull_request_inbox.is_visible() {
            "Hide pull request inbox"
        } else {
            "Show pull request inbox"
        };
        let is_busy = self.repository_state.is_loading()
            || self.pull_request_inbox.is_loading()
            || self.pull_request_inbox.is_loading_more()
            || self.detail_state.is_any_loading()
            || self.review_state.reviews_loading()
            || self.repository_actions_state.workflows_loading()
            || self.repository_actions_state.runs_loading()
            || self.repository_actions_state.is_loading_more_runs()
            || self.action_runtime.is_any_running();
        let status_label = (!is_busy)
            .then(|| self.selected_pull_request())
            .flatten()
            .map(|pr| {
                format!(
                    "{} files changed · {} reviewed · {} unresolved",
                    self.detail_state.files().len(),
                    self.reviewed_file_count(),
                    pr.unresolved_threads
                )
            })
            .unwrap_or_else(|| self.status.clone());
        let rate_limit_indicator = if SHOW_STATUS_BAR_RATE_LIMITS {
            let rate_limits = self.github_api.latest_rate_limits();
            let fallback_rate_limit = self.github_api.latest_rate_limit();
            github_rate_limit_indicator(&rate_limits, fallback_rate_limit.as_ref())
        } else {
            None
        };

        StatusBar::new()
            .bg(color::app_background())
            .text_color(color::text_muted())
            .border_1()
            .border_color(color::border())
            .child(
                Button::new("toggle-pull-request-inbox")
                    .ghost()
                    .small()
                    .compact()
                    .icon(inbox_toggle_icon)
                    .tooltip_with_action(
                        inbox_toggle_tooltip,
                        &TogglePullRequestInbox,
                        Some(KEY_CONTEXT),
                    )
                    .on_click(cx.listener(|view, _, window, cx| {
                        view.toggle_pull_request_inbox(&TogglePullRequestInbox, window, cx);
                    })),
            )
            .child(div().min_w_0().flex_1().truncate().child(status_label))
            .when_some(rate_limit_indicator, |element, indicator| {
                element.right(render_rate_limit_indicator(indicator))
            })
    }
}

fn render_rate_limit_indicator(indicator: GitHubRateLimitIndicator) -> impl IntoElement {
    let details = indicator.details;

    div()
        .id("github-rate-limit-indicator")
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .size_5()
        .tooltip(move |window, cx| {
            let details = details.clone();
            Tooltip::element(move |_, _| {
                div()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .whitespace_nowrap()
                    .children(details.iter().map(|detail| div().child(detail.clone())))
            })
            .build(window, cx)
        })
        .child(
            ProgressCircle::new("github-rate-limit-progress")
                .small()
                .value(indicator.value)
                .color(github_rate_limit_indicator_color(indicator.tone)),
        )
}
