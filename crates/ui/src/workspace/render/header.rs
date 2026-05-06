#[path = "header/open_with.rs"]
mod open_with;
#[path = "header/switchers.rs"]
mod switchers;

#[cfg(test)]
pub(crate) use open_with::open_with_app_disabled;

use gpui::{Context, IntoElement, div, prelude::*, rgb};
use gpui_component::{
    IconName, Sizable, TitleBar,
    button::{Button, ButtonVariants},
};

use crate::actions::TogglePullRequestInbox;
use crate::workspace::AppView;

impl AppView {
    fn header_repository_label(&self) -> String {
        self.current_repository()
            .map(|repository| repository.name.clone())
            .unwrap_or_else(|| "repository".to_string())
    }

    fn header_pull_request_label(&self) -> String {
        if let Some(pull_request) = self.selected_pull_request() {
            return format!("#{} {}", pull_request.number, pull_request.title);
        }

        if self.is_loading_prs {
            "loading pull requests".to_string()
        } else if self.load_error.is_some() {
            "pull requests unavailable".to_string()
        } else {
            "no pull request selected".to_string()
        }
    }

    pub(super) fn render_title_bar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let inbox_toggle_icon = if self.pull_request_inbox_visible {
            IconName::PanelLeft
        } else {
            IconName::PanelLeftOpen
        };
        let inbox_toggle_tooltip = if self.pull_request_inbox_visible {
            "Hide pull request inbox"
        } else {
            "Show pull request inbox"
        };

        TitleBar::new()
            .bg(rgb(0x101214))
            .border_color(rgb(0x242a31))
            .child(
                div()
                    .flex()
                    .h_full()
                    .w_full()
                    .min_w_0()
                    .items_center()
                    .justify_between()
                    .gap_3()
                    .pr_2()
                    .child(
                        div()
                            .flex()
                            .h_full()
                            .min_w_0()
                            .items_center()
                            .gap_1()
                            .child(
                                Button::new("toggle-pull-request-inbox")
                                    .ghost()
                                    .small()
                                    .compact()
                                    .icon(inbox_toggle_icon)
                                    .tooltip(inbox_toggle_tooltip)
                                    .on_click(cx.listener(|view, _, window, cx| {
                                        view.toggle_pull_request_inbox(
                                            &TogglePullRequestInbox,
                                            window,
                                            cx,
                                        );
                                    })),
                            )
                            .child(self.render_repository_switcher(cx))
                            .child(div().px_1().text_color(rgb(0x6f7782)).child("/"))
                            .child(self.render_pull_request_switcher(cx)),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(self.render_open_with_dropdown()),
                    ),
            )
    }
}
