use crate::{visual::color, workspace::AppView};
use gpui::{Context, IntoElement, div, prelude::*};

impl AppView {
    pub(super) fn render_changed_files_sidebar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .debug_selector(|| "changed-files-sidebar".to_string())
            .size_full()
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
