use gpui::{Context, IntoElement, div, prelude::*, px, rgb, uniform_list};
use gpui_component::{
    Sizable,
    button::{Button, ButtonVariants},
};
use harbor_domain::RepoId;

use crate::{
    panels::render_pull_request_row,
    workspace::{AppView, PullRequestInboxMode},
};

impl AppView {
    pub(super) fn render_inbox(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let show_list =
            !self.is_loading_prs && self.load_error.is_none() && !self.pull_requests.is_empty();
        let current_mode = self.pull_request_inbox.mode;
        let repository_label = self
            .configured_repo
            .as_ref()
            .map(RepoId::full_name)
            .unwrap_or_else(|| "choose a repository from the header".to_string());
        let empty_message = if self.configured_repo.is_some() {
            current_mode.empty_message()
        } else {
            "Choose a repository from the header"
        };

        div()
            .w(px(320.))
            .flex()
            .flex_col()
            .min_h_0()
            .border_1()
            .border_color(rgb(0x242a31))
            .bg(rgb(0x15191e))
            .overflow_hidden()
            .child(
                div()
                    .px_3()
                    .py_2()
                    .text_sm()
                    .text_color(rgb(0xf1f5f9))
                    .child("Pull request inbox")
                    .child(
                        div()
                            .pt_1()
                            .text_xs()
                            .text_color(rgb(0x9aa4b2))
                            .child(repository_label),
                    )
                    .child(div().pt_2().flex().items_center().gap_1().children(
                        PullRequestInboxMode::ALL.into_iter().map(|mode| {
                            let active = mode == current_mode;
                            let button =
                                Button::new(format!("pull-request-inbox-mode-{}", mode.key()))
                                    .label(mode.label())
                                    .small()
                                    .compact();
                            let button = if active {
                                button.primary()
                            } else {
                                button.ghost()
                            };

                            button.on_click(cx.listener(move |view, _, _, cx| {
                                view.select_pull_request_inbox_mode(mode, cx);
                            }))
                        }),
                    )),
            )
            .when(self.is_loading_prs, |element| {
                element.child(
                    div()
                        .flex_1()
                        .px_3()
                        .py_3()
                        .text_sm()
                        .text_color(rgb(0x9aa4b2))
                        .child(format!("Loading {}...", current_mode.status_label())),
                )
            })
            .when(
                !self.is_loading_prs && self.load_error.is_some(),
                |element| {
                    element.child(
                        div()
                            .flex_1()
                            .px_3()
                            .py_3()
                            .text_sm()
                            .text_color(rgb(0xf87171))
                            .child(self.load_error.clone().unwrap_or_default()),
                    )
                },
            )
            .when(
                !self.is_loading_prs && self.load_error.is_none() && self.pull_requests.is_empty(),
                |element| {
                    element.child(
                        div()
                            .flex_1()
                            .px_3()
                            .py_3()
                            .text_sm()
                            .text_color(rgb(0x9aa4b2))
                            .child(empty_message),
                    )
                },
            )
            .when(show_list, |element| {
                element.child(
                    uniform_list(
                        "pull-request-inbox-list",
                        self.pull_requests.len(),
                        cx.processor(|view, range: std::ops::Range<usize>, _window, cx| {
                            let mut rows = Vec::with_capacity(range.len());

                            for index in range {
                                let Some(pr) = view.pull_requests.get(index) else {
                                    continue;
                                };
                                rows.push(render_pull_request_row(
                                    index,
                                    pr,
                                    index == view.selected_pr,
                                    cx,
                                ));
                            }

                            rows
                        }),
                    )
                    .track_scroll(&self.pr_list_scroll)
                    .flex_1()
                    .min_h_0()
                    .w_full(),
                )
            })
    }
}
