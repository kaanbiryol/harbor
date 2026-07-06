#[path = "header/github_auth.rs"]
mod github_auth;
#[path = "header/open_with.rs"]
mod open_with;
#[path = "header/repository_switcher_rows.rs"]
mod repository_switcher_rows;
#[path = "header/switchers.rs"]
mod switchers;

use gpui::{Context, IntoElement, Window, div, prelude::*, px};
use gpui_component::{StyledExt, TitleBar};

use crate::{visual::color, workspace::AppView};

impl AppView {
    fn header_repository_label(&self) -> String {
        self.current_repository()
            .map(|repository| repository.name.clone())
            .unwrap_or_else(|| "repository".to_string())
    }

    pub(super) fn render_title_bar(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let is_fullscreen = window.is_fullscreen();
        let show_repository_switcher = !self.github_auth_gate_visible();
        let mut left_controls = div().flex().h_full().min_w_0().items_center().gap_1();
        if show_repository_switcher {
            left_controls = left_controls.child(self.render_repository_switcher(is_fullscreen, cx));
        } else {
            left_controls = left_controls.child(
                div()
                    .when(!is_fullscreen, |element| element.pl_2())
                    .font_medium()
                    .text_color(color::text_secondary())
                    .child("Harbor"),
            );
        }

        TitleBar::new()
            .bg(color::app_background())
            .border_color(color::border())
            .when(is_fullscreen, |title_bar| title_bar.pl(px(0.)))
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
                    .child(left_controls)
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(self.render_github_auth_control(cx))
                            .child(self.render_open_with_dropdown()),
                    ),
            )
    }
}
