use gpui::{Context, IntoElement, div, prelude::*};
use gpui_component::{
    Disableable, Sizable, StyledExt,
    button::{Button, ButtonVariants},
    tab::{Tab, TabBar},
};

use crate::{
    icons::Octicon,
    panels::render_status_pill,
    visual::{Tone, color},
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
        let active_index = PullRequestInboxMode::ALL
            .iter()
            .position(|mode| *mode == current_mode)
            .unwrap_or_default();
        let tabs = PullRequestInboxMode::ALL.into_iter().map(|mode| {
            Tab::new()
                .label(mode.label())
                .when_some(self.pull_request_inbox_mode_count(mode), |tab, count| {
                    tab.suffix(render_status_pill(count.to_string(), Tone::Neutral))
                })
        });
        let view = cx.entity().clone();

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
                            .child(self.render_pull_request_filters(cx))
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
            .child(
                div().pt_2().child(
                    TabBar::new("pull-request-inbox-mode-tabs")
                        .w_full()
                        .underline()
                        .xsmall()
                        .selected_index(active_index)
                        .children(tabs)
                        .on_click(move |index, _, cx| {
                            let Some(mode) = PullRequestInboxMode::ALL.get(*index).copied() else {
                                return;
                            };
                            view.update(cx, |view, cx| {
                                view.select_pull_request_inbox_mode(mode, cx);
                            });
                        }),
                ),
            )
            .when(self.has_active_pull_request_filters(), |element| {
                element.child(self.render_pull_request_filter_chips(cx))
            })
    }
}
