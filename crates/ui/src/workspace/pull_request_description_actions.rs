use gpui::{Context, Window};

use crate::workspace::{AppView, async_updates::AppViewAsyncUpdateExt};

impl AppView {
    pub(super) fn start_pull_request_description_edit(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(pull_request) = self.selected_pull_request() else {
            self.status = "Select a pull request before editing its description".to_string();
            cx.notify();
            return;
        };
        let body = pull_request.body.clone().unwrap_or_default();

        self.pull_request_description_editing = true;
        self.action_runtime
            .clear_pull_request_description_action_error();
        self.pull_request_description_input.update(cx, |input, cx| {
            input.set_value(body, window, cx);
            input.focus(window, cx);
        });
        self.status = "Editing pull request description".to_string();
        cx.notify();
    }

    pub(super) fn cancel_pull_request_description_edit(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self
            .action_runtime
            .pull_request_description_action_running()
        {
            return;
        }

        self.pull_request_description_editing = false;
        self.action_runtime
            .clear_pull_request_description_action_error();
        self.pull_request_description_input
            .update(cx, |input, cx| input.set_value("", window, cx));
        self.status = "Cancelled pull request description edit".to_string();
        cx.notify();
    }

    pub(super) fn save_pull_request_description(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self
            .action_runtime
            .pull_request_description_action_running()
        {
            self.status = "The pull request description is already being saved".to_string();
            cx.notify();
            return;
        }
        let Some(pull_request) = self.selected_pull_request() else {
            self.status = "Select a pull request before saving its description".to_string();
            cx.notify();
            return;
        };
        let pull_request_node_id = pull_request.node_id.clone();
        let pull_request_number = pull_request.number;
        let body = self
            .pull_request_description_input
            .read(cx)
            .value()
            .trim_end()
            .to_string();

        self.action_runtime.start_pull_request_description_action();
        self.status = format!("Saving description for PR #{pull_request_number}");
        cx.notify();
        let github_api = self.github_api.clone();

        cx.spawn_in(window, async move |this, cx| {
            let result = github_api
                .update_pull_request_body(&pull_request_node_id, &body)
                .await;

            this.update_in_or_log(
                cx,
                "failed to update pull request description state",
                move |view, window, cx| {
                    match result {
                        Ok(()) => {
                            view.action_runtime.finish_pull_request_description_action();
                            if let Some(pull_request) = view
                                .pull_requests
                                .iter_mut()
                                .find(|pull_request| pull_request.node_id == pull_request_node_id)
                            {
                                pull_request.body = (!body.is_empty()).then_some(body);
                            }
                            view.pull_request_description_editing = false;
                            view.pull_request_description_input.update(cx, |input, cx| {
                                input.set_value("", window, cx);
                            });
                            view.status =
                                format!("Saved description for PR #{pull_request_number}");
                        }
                        Err(error) => {
                            let message = format!(
                                "Failed to save PR #{pull_request_number} description: {error}"
                            );
                            view.action_runtime
                                .finish_pull_request_description_action_failure(message.clone());
                            view.status = message;
                        }
                    }

                    cx.notify();
                },
            );
        })
        .detach();
    }
}
