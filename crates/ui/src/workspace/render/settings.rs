use gpui::{Context, IntoElement, div, prelude::*, px, rgba};
use gpui_component::{
    Sizable, StyledExt,
    button::{Button, ButtonVariants},
};

use crate::{actions::CloseSettings, icons::Octicon, visual::color, workspace::AppView};

impl AppView {
    pub(super) fn render_settings_overlay(&self, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .absolute()
            .inset_0()
            .occlude()
            .on_any_mouse_down(|_, _, cx| {
                cx.stop_propagation();
            })
            .bg(rgba(0x00000080))
            .flex()
            .items_center()
            .justify_center()
            .child(
                div()
                    .w(px(620.))
                    .max_h(px(560.))
                    .border_1()
                    .border_color(color::border_strong())
                    .bg(color::panel_background())
                    .rounded_xs()
                    .shadow_lg()
                    .flex()
                    .flex_col()
                    .child(self.render_settings_header(cx))
                    .child(self.render_github_settings(cx)),
            )
    }

    fn render_settings_header(&self, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .h(px(44.))
            .flex()
            .items_center()
            .justify_between()
            .border_b_1()
            .border_color(color::border())
            .px_3()
            .child(
                div()
                    .text_base()
                    .font_semibold()
                    .text_color(color::text_primary())
                    .child("Settings"),
            )
            .child(
                Button::new("close-settings")
                    .ghost()
                    .small()
                    .compact()
                    .icon(Octicon::X)
                    .tooltip("Close settings")
                    .on_click(cx.listener(|view, _, window, cx| {
                        view.close_settings(&CloseSettings, window, cx);
                    })),
            )
    }

    fn render_github_settings(&self, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .min_w_0()
            .p_3()
            .flex()
            .flex_col()
            .gap_3()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(
                        div()
                            .text_sm()
                            .font_medium()
                            .text_color(color::text_primary())
                            .child("GitHub authentication"),
                    )
                    .child(
                        div()
                            .text_sm()
                            .text_color(color::text_muted())
                            .child("Choose how Harbor talks to GitHub."),
                    ),
            )
            .child(self.render_github_settings_account(cx))
            .when_some(self.auth_switch_status().cloned(), |element, status| {
                element.child(self.render_auth_switch_status(status, cx))
            })
            .child(self.render_auth_method_rows(cx))
    }
}
