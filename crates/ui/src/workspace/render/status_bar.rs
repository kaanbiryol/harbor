use gpui::{Context, Div, IntoElement, div, prelude::*, px};
use gpui_component::{
    Sizable,
    button::{Button, ButtonVariants},
    progress::ProgressCircle,
};

use crate::{actions::TogglePullRequestInbox, icons::Octicon, visual::color, workspace::AppView};

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
        let status_label = self
            .selected_pull_request()
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

        div()
            .flex()
            .items_center()
            .gap_2()
            .px_2()
            .py_1()
            .text_xs()
            .text_color(color::text_muted())
            .border_1()
            .border_color(color::border())
            .child(
                Button::new("toggle-pull-request-inbox")
                    .ghost()
                    .small()
                    .compact()
                    .icon(inbox_toggle_icon)
                    .tooltip(inbox_toggle_tooltip)
                    .on_click(cx.listener(|view, _, window, cx| {
                        view.toggle_pull_request_inbox(&TogglePullRequestInbox, window, cx);
                    })),
            )
            .child(div().min_w_0().flex_1().truncate().child(status_label))
            .when_some(rate_limit_indicator, |element, indicator| {
                element.child(render_rate_limit_indicator(indicator))
            })
    }
}

fn render_rate_limit_indicator(indicator: GitHubRateLimitIndicator) -> impl IntoElement {
    let details = indicator.details.clone();

    div()
        .id("github-rate-limit-indicator")
        .group("github-rate-limit-indicator")
        .relative()
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .size_5()
        .child(
            ProgressCircle::new("github-rate-limit-progress")
                .small()
                .value(indicator.value)
                .color(github_rate_limit_indicator_color(indicator.tone)),
        )
        .child(
            render_rate_limit_tooltip(details)
                .absolute()
                .right_0()
                .bottom(px(24.0))
                .invisible()
                .group_hover("github-rate-limit-indicator", |element| element.visible()),
        )
}

fn render_rate_limit_tooltip(details: Vec<String>) -> Div {
    div()
        .flex()
        .flex_col()
        .gap_1()
        .whitespace_nowrap()
        .rounded_xs()
        .border_1()
        .border_color(color::border())
        .bg(color::elevated_background())
        .px_2()
        .py_1()
        .shadow_lg()
        .text_xs()
        .text_color(color::text_secondary())
        .children(details.into_iter().map(|detail| div().child(detail)))
}
