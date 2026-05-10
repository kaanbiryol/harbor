#[path = "header/open_with.rs"]
mod open_with;
#[path = "header/switchers.rs"]
mod switchers;

use gpui::{Context, IntoElement, div, prelude::*, rgb};
use gpui_component::TitleBar;

use crate::workspace::AppView;

impl AppView {
    fn header_repository_label(&self) -> String {
        self.current_repository()
            .map(|repository| repository.name.clone())
            .unwrap_or_else(|| "repository".to_string())
    }

    pub(super) fn render_title_bar(&self, cx: &mut Context<Self>) -> impl IntoElement {
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
                            .child(self.render_repository_switcher(cx)),
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
