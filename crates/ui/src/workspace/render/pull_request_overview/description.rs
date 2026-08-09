use super::*;
use crate::icons::Octicon;
use crate::workspace::PullRequestDetailCacheKey;
use gpui::rgb;

impl AppView {
    pub(super) fn prepare_pull_request_description(
        &mut self,
        pr: &PullRequest,
        detail_key: PullRequestDetailCacheKey,
        cx: &mut Context<Self>,
    ) {
        if self.overview_state.description_ready() || self.overview_state.description_preparing() {
            return;
        }

        let Some(body) = pull_request_description_body(pr) else {
            self.overview_state.mark_description_ready(&detail_key);
            return;
        };
        let markdown = overview_markdown_body(body);
        let state = self.ensure_overview_markdown_state(
            pull_request_description_markdown_key(pr.number),
            &markdown,
            cx,
        );
        let subscription = cx.observe(&state, move |view, _, cx| {
            if view.overview_state.mark_description_ready(&detail_key) {
                cx.notify();
            }
        });
        self.overview_state
            .set_description_markdown_subscription(subscription);
    }

    pub(super) fn render_description_card(
        &mut self,
        pr: &PullRequest,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let editing = self.pull_request_description_editing;
        let saving = self
            .action_runtime
            .pull_request_description_action_running();
        let error = self
            .action_runtime
            .pull_request_description_action_error()
            .map(str::to_string);
        let description_input = self.pull_request_description_input.clone();
        let description = if editing {
            None
        } else {
            Some(self.render_pull_request_description(pr, cx))
        };
        let edit_button_style = ButtonCustomVariant::new(cx)
            .color(color::row_selected_subtle().into())
            .foreground(rgb(0xffffff).into())
            .hover(color::row_selected().into())
            .active(color::row_selected_active().into());

        div()
            .debug_selector(|| "pull-request-overview-description".to_string())
            .w_full()
            .min_w_0()
            .rounded_sm()
            .border_1()
            .border_color(color::border())
            .bg(color::content_background())
            .p_4()
            .child(
                div()
                    .pb_3()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap_3()
                    .child(
                        div()
                            .text_lg()
                            .font_semibold()
                            .text_color(color::text_primary())
                            .child("Description"),
                    )
                    .when(!editing, |element| {
                        element.child(
                            Button::new("edit-pull-request-description")
                                .icon(Octicon::Pencil)
                                .xsmall()
                                .custom(edit_button_style)
                                .rounded(px(999.))
                                .border_1()
                                .border_color(color::border_strong())
                                .tooltip("Edit description if your GitHub permissions allow it")
                                .on_click(cx.listener(|view, _, window, cx| {
                                    view.start_pull_request_description_edit(window, cx);
                                })),
                        )
                    }),
            )
            .when_some(description, |element, description| {
                element.child(description)
            })
            .when(editing, |element| {
                element
                    .child(Input::new(&description_input))
                    .when_some(error, |element, error| {
                        element.child(
                            div()
                                .pt_2()
                                .text_xs()
                                .text_color(color::danger())
                                .child(error),
                        )
                    })
                    .child(
                        div()
                            .pt_3()
                            .flex()
                            .items_center()
                            .justify_end()
                            .gap_2()
                            .child(
                                Button::new("cancel-pull-request-description")
                                    .label("Cancel")
                                    .small()
                                    .outline()
                                    .disabled(saving)
                                    .on_click(cx.listener(|view, _, window, cx| {
                                        view.cancel_pull_request_description_edit(window, cx);
                                    })),
                            )
                            .child(
                                Button::new("save-pull-request-description")
                                    .label("Save")
                                    .small()
                                    .loading(saving)
                                    .disabled(saving)
                                    .on_click(cx.listener(|view, _, window, cx| {
                                        view.save_pull_request_description(window, cx);
                                    })),
                            ),
                    )
            })
            .into_any_element()
    }

    fn render_pull_request_description(
        &mut self,
        pr: &PullRequest,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let Some(body) = pull_request_description_body(pr) else {
            return div()
                .text_sm()
                .text_color(color::text_muted())
                .child("No description")
                .into_any_element();
        };
        let markdown = self.render_overview_markdown(
            pull_request_description_markdown_key(pr.number),
            &overview_markdown_body(body),
            cx,
        );

        div()
            .min_w_0()
            .pr_1()
            .text_sm()
            .text_color(color::text_secondary())
            .child(markdown)
            .into_any_element()
    }
}

fn pull_request_description_body(pr: &PullRequest) -> Option<&str> {
    pr.body
        .as_deref()
        .map(str::trim)
        .filter(|body| !body.is_empty())
}

fn pull_request_description_markdown_key(number: u64) -> String {
    format!("pull-request-description-{number}")
}
