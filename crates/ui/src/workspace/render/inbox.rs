use gpui::{Context, IntoElement, div, prelude::*};

use crate::{visual::color, workspace::AppView};

impl AppView {
    pub(super) fn render_inbox(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let current_mode = self.pull_request_inbox.mode();

        div()
            .debug_selector(|| "pull-request-inbox".to_string())
            .size_full()
            .flex()
            .flex_col()
            .min_h_0()
            .border_1()
            .border_color(color::border())
            .bg(color::panel_background())
            .overflow_hidden()
            .child(self.render_pull_request_inbox_header(current_mode, cx))
            .children(self.render_pull_request_inbox_body(current_mode, cx))
    }
}
