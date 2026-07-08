use gpui::{AnyElement, Context, div, prelude::*, px};
use gpui_component::{
    Disableable, Sizable,
    button::{Button, ButtonVariants},
};

use crate::{icons::Octicon, visual::color, workspace::AppView};

impl AppView {
    pub(super) fn render_pull_request_inbox_page_footer(
        &self,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let loaded_count = self.pull_requests.len();
        let visible_count = self.visible_pull_request_indices().len();
        let total_count = self
            .current_pull_request_inbox_key()
            .as_ref()
            .and_then(|key| self.pull_request_inbox.stored_count(key))
            .or_else(|| self.pull_request_inbox.total_count());
        let count_label = if self.has_active_pull_request_filters() {
            format!("Showing {visible_count} of {loaded_count} loaded")
        } else {
            match total_count {
                Some(total_count) => format!("Showing {loaded_count} of {total_count}"),
                None => format!("Showing {loaded_count}"),
            }
        };
        let load_more_error = self
            .pull_request_inbox
            .load_more_error()
            .map(str::to_string);
        let can_load_more = self.pull_request_inbox.has_next_page()
            && !self.pull_request_inbox.is_loading()
            && !self.pull_request_inbox.is_loading_more();

        div()
            .id("pull-request-inbox-page-footer")
            .h(px(76.))
            .w_full()
            .border_t_1()
            .border_color(color::border())
            .px_3()
            .py_1()
            .flex()
            .flex_col()
            .justify_center()
            .gap_1()
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap_2()
                    .child(
                        div()
                            .min_w_0()
                            .truncate()
                            .text_xs()
                            .text_color(color::text_muted())
                            .child(count_label),
                    )
                    .child(
                        Button::new("load-more-pull-requests")
                            .ghost()
                            .small()
                            .compact()
                            .icon(Octicon::ChevronDown)
                            .label("Load more")
                            .tooltip("Load more pull requests")
                            .loading(self.pull_request_inbox.is_loading_more())
                            .disabled(!can_load_more)
                            .on_click(cx.listener(|view, _, _, cx| {
                                view.load_more_pull_requests(cx);
                            })),
                    ),
            )
            .when_some(load_more_error, |element, error| {
                element.child(
                    div()
                        .text_xs()
                        .text_color(color::danger())
                        .child(format!("Load more failed: {error}")),
                )
            })
            .into_any_element()
    }
}
