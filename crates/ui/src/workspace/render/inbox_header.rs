use gpui::{AnyElement, Context, IntoElement, div, prelude::*, px};
use gpui_component::{
    Disableable, Sizable, StyledExt,
    button::{Button, ButtonVariants},
};

use crate::{
    icons::Octicon,
    visual::color,
    workspace::{AppView, PullRequestInboxCacheKey, PullRequestInboxMode},
};

impl AppView {
    fn pull_request_inbox_mode_count(&self, mode: PullRequestInboxMode) -> Option<usize> {
        let repository = self.repository_state.configured_repo()?;
        let key = PullRequestInboxCacheKey::new(repository.clone(), mode);

        if mode == self.pull_request_inbox.mode() {
            return self
                .pull_request_inbox
                .stored_count(&key)
                .or_else(|| self.pull_request_inbox.total_count())
                .or_else(|| {
                    (!self.pull_request_inbox.has_next_page()).then_some(self.pull_requests.len())
                });
        }

        self.pull_request_inbox.snapshot_count(&key)
    }

    pub(super) fn render_pull_request_inbox_header(
        &self,
        current_mode: PullRequestInboxMode,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        div()
            .px_3()
            .pt_3()
            .pb_2()
            .border_b_1()
            .border_color(color::border())
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap_2()
                    .child(
                        div()
                            .min_w_0()
                            .flex_1()
                            .truncate()
                            .text_sm()
                            .font_medium()
                            .text_color(color::text_primary())
                            .child("Pull requests"),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_1()
                            .child(self.render_pull_request_inbox_search(cx))
                            .child(
                                Button::new("refresh-pull-request-inbox")
                                    .ghost()
                                    .small()
                                    .compact()
                                    .icon(Octicon::Sync)
                                    .tooltip("Refresh pull requests")
                                    .loading(self.pull_request_inbox.is_loading())
                                    .disabled(!self.repository_state.has_configured_repo())
                                    .on_click(cx.listener(|view, _, _, cx| {
                                        view.reload_pull_request_inbox(cx);
                                    })),
                            ),
                    ),
            )
            .child(div().pt_2().flex().items_center().gap_1().children(
                PullRequestInboxMode::ALL.into_iter().map(|mode| {
                    let active = mode == current_mode;
                    let count = self.pull_request_inbox_mode_count(mode);

                    render_pull_request_inbox_mode_tab(mode, active, count, cx)
                }),
            ))
    }
}

fn render_pull_request_inbox_mode_tab(
    mode: PullRequestInboxMode,
    active: bool,
    count: Option<usize>,
    cx: &mut Context<AppView>,
) -> AnyElement {
    div()
        .id(format!("pull-request-inbox-mode-{}", mode.key()))
        .h(px(28.))
        .min_w_0()
        .flex()
        .items_center()
        .gap_1()
        .rounded_xs()
        .px_2()
        .text_xs()
        .font_medium()
        .cursor_pointer()
        .text_color(if active {
            color::text_primary()
        } else {
            color::text_secondary()
        })
        .when(active, |element| {
            element
                .border_1()
                .border_color(color::border_strong())
                .bg(color::row_selected())
        })
        .when(!active, |element| {
            element.hover(|style| style.bg(color::row_hover()))
        })
        .on_click(cx.listener(move |view, _, _, cx| {
            view.select_pull_request_inbox_mode(mode, cx);
        }))
        .child(div().truncate().child(mode.label()))
        .when_some(count, |element, count| {
            element.child(
                div()
                    .min_w(px(16.))
                    .h(px(18.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded_xs()
                    .px_1()
                    .text_color(if active {
                        color::text_secondary()
                    } else {
                        color::text_muted()
                    })
                    .bg(if active {
                        color::row_selected_subtle()
                    } else {
                        color::elevated_background()
                    })
                    .child(count.to_string()),
            )
        })
        .into_any_element()
}
