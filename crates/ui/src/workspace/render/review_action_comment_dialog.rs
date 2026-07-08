use gpui::{Context, IntoElement, div, prelude::*, px, rgba};
use gpui_component::{
    Disableable, Sizable, StyledExt,
    button::{Button, ButtonVariants},
    input::Input,
};

use crate::{icons::Octicon, visual::color, workspace::AppView};

impl AppView {
    pub(super) fn render_review_action_comment_dialog(
        &self,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let Some(target) = self.review_action_comment_target else {
            return div();
        };

        let body_empty = self
            .review_action_comment_input
            .read(cx)
            .value()
            .trim()
            .is_empty();
        let submit_disabled = self.action_runtime.pull_request_action_running()
            || (target.requires_body() && body_empty);
        let input = self.review_action_comment_input.clone();

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
                    .w(px(440.))
                    .border_1()
                    .border_color(color::border_strong())
                    .bg(color::panel_background())
                    .rounded_xs()
                    .shadow_lg()
                    .flex()
                    .flex_col()
                    .child(
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
                                    .child(target.title()),
                            )
                            .child(
                                Button::new("close-review-action-comment")
                                    .ghost()
                                    .small()
                                    .compact()
                                    .icon(Octicon::X)
                                    .tooltip("Close")
                                    .on_click(cx.listener(|view, _, _, cx| {
                                        view.close_review_action_comment_dialog(cx);
                                    })),
                            ),
                    )
                    .child(
                        div().p_3().child(
                            Input::new(&input)
                                .small()
                                .w_full()
                                .appearance(false)
                                .bordered(true)
                                .focus_bordered(true),
                        ),
                    )
                    .child(
                        div()
                            .border_t_1()
                            .border_color(color::border())
                            .px_3()
                            .py_2()
                            .flex()
                            .items_center()
                            .justify_end()
                            .gap_2()
                            .child(
                                Button::new("cancel-review-action-comment")
                                    .label("Cancel")
                                    .small()
                                    .outline()
                                    .on_click(cx.listener(|view, _, _, cx| {
                                        view.close_review_action_comment_dialog(cx);
                                    })),
                            )
                            .child(
                                Button::new("submit-review-action-comment")
                                    .label(target.submit_label())
                                    .small()
                                    .primary()
                                    .loading(self.action_runtime.pull_request_action_running())
                                    .disabled(submit_disabled)
                                    .on_click(cx.listener(|view, _, window, cx| {
                                        view.submit_review_action_comment_dialog(window, cx);
                                    })),
                            ),
                    ),
            )
    }
}
