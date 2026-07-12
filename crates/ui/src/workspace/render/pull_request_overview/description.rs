use super::*;

impl AppView {
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
                                .secondary()
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
        let Some(body) = pr
            .body
            .as_deref()
            .map(str::trim)
            .filter(|body| !body.is_empty())
        else {
            return div()
                .text_sm()
                .text_color(color::text_muted())
                .child("No description")
                .into_any_element();
        };
        let markdown = self.render_overview_markdown(
            format!("pull-request-description-{}", pr.number),
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
