use gpui::{Context, IntoElement, div, prelude::*, px};
use gpui_component::{
    IconName, Sizable,
    button::{Button, ButtonVariants},
};

use crate::{actions::TogglePullRequestInbox, visual::color, workspace::AppView};

use super::rate_limits::{
    github_rate_limit_color, github_rate_limit_label, github_rate_limits_label,
};

const SHOW_STATUS_BAR_RATE_LIMITS: bool = true;

impl AppView {
    pub(super) fn render_status_bar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let inbox_toggle_icon = if self.pull_request_inbox.is_visible() {
            IconName::PanelLeft
        } else {
            IconName::PanelLeftOpen
        };
        let inbox_toggle_tooltip = if self.pull_request_inbox.is_visible() {
            "Hide pull request inbox"
        } else {
            "Show pull request inbox"
        };
        let status_label = self.status.clone();
        let (rate_limit_label, rate_limit_color) = if SHOW_STATUS_BAR_RATE_LIMITS {
            let rate_limits = self.github_api.latest_rate_limits();
            let rate_limit = self.github_api.latest_rate_limit();
            (
                github_rate_limits_label(&rate_limits)
                    .or_else(|| rate_limit.as_ref().and_then(github_rate_limit_label)),
                rate_limit
                    .as_ref()
                    .map(github_rate_limit_color)
                    .unwrap_or_else(color::text_muted),
            )
        } else {
            (None, color::text_muted())
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
            .when_some(rate_limit_label, |element, label| {
                element.child(
                    div()
                        .flex_none()
                        .max_w(px(260.))
                        .truncate()
                        .text_color(rate_limit_color)
                        .child(label),
                )
            })
    }
}
