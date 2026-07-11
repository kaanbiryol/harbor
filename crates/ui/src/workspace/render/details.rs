use crate::{
    visual::{color, layout},
    workspace::AppView,
};
use gpui::{Context, IntoElement, div, prelude::*, px};

impl AppView {
    pub(super) fn render_changed_files_sidebar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .debug_selector(|| "changed-files-sidebar".to_string())
            .w(px(layout::PULL_REQUEST_DETAILS_WIDTH))
            .flex()
            .flex_col()
            .min_h_0()
            .border_1()
            .border_color(color::border())
            .bg(color::panel_background())
            .overflow_hidden()
            .child(self.render_changed_files_header(cx))
            .child(self.render_changed_files_body(cx))
    }
}
