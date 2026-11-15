use std::ops::Range;

use gpui::{Context, Window};

use crate::{
    panels::set_task_checked,
    workspace::{AppView, PullRequestDetailCacheKey, async_updates::AppViewAsyncUpdateExt},
};

impl AppView {
    pub(crate) fn start_pull_request_description_edit(
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

    pub(crate) fn cancel_pull_request_description_edit(
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

    pub(crate) fn save_pull_request_description(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let body = self
            .pull_request_description_input
            .read(cx)
            .value()
            .trim_end()
            .to_string();
        self.submit_pull_request_description(body, None, window, cx);
    }

    /// Sets a description task list item to `checked`, showing the change
    /// before GitHub confirms it.
    pub(crate) fn toggle_pull_request_description_task(
        &mut self,
        block_index: usize,
        marker_range: Range<usize>,
        checked: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self
            .action_runtime
            .pull_request_description_action_running()
        {
            self.set_status("The pull request description is already being saved", cx);
            return;
        }
        let Some(pull_request) = self.selected_pull_request() else {
            return;
        };
        let node_id = pull_request.node_id.clone();
        let previous_body = pull_request.body.clone();
        let Some(body) = self
            .overview_state
            .description_task_index(block_index, marker_range)
            .and_then(|task_index| {
                set_task_checked(
                    previous_body.as_deref().unwrap_or_default(),
                    task_index,
                    checked,
                )
            })
        else {
            self.report_error(
                "The description changed since it was shown; reload it and try again",
                cx,
            );
            return;
        };

        let detail_key = self.selected_pull_request_detail_key();
        self.set_pull_request_body(&node_id, detail_key.as_ref(), Some(body.clone()));
        self.submit_pull_request_description(body, Some(previous_body), window, cx);
    }

    fn set_pull_request_body(
        &mut self,
        node_id: &str,
        detail_key: Option<&PullRequestDetailCacheKey>,
        body: Option<String>,
    ) {
        if let Some(pull_request) = self
            .pull_requests
            .iter_mut()
            .find(|pull_request| pull_request.node_id == node_id)
        {
            pull_request.body = body;
        }
        if let Some(detail_key) = detail_key {
            self.overview_state.invalidate_description(detail_key);
        }
    }

    /// Saves `body` as the selected pull request's description. A failed save
    /// restores `restore_body` when it is set.
    fn submit_pull_request_description(
        &mut self,
        body: String,
        restore_body: Option<Option<String>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let auth_generation = self.auth_generation;
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
        let detail_key = self.selected_pull_request_detail_key();

        self.action_runtime.start_pull_request_description_action();
        self.status = format!("Saving description for PR #{pull_request_number}");
        cx.notify();
        let github_api = self.github_api_for_request();

        cx.spawn_in(window, async move |this, cx| {
            let result = github_api
                .update_pull_request_body(&pull_request_node_id, &body)
                .await;

            this.update_window_in_session_or_log(
                cx,
                auth_generation,
                "failed to update pull request description state",
                move |view, window, cx| {
                    match result {
                        Ok(()) => {
                            view.action_runtime.finish_pull_request_description_action();
                            // An optimistic save already shows `body`.
                            if restore_body.is_none() {
                                view.set_pull_request_body(
                                    &pull_request_node_id,
                                    detail_key.as_ref(),
                                    (!body.is_empty()).then_some(body),
                                );
                                view.pull_request_description_editing = false;
                                view.pull_request_description_input.update(cx, |input, cx| {
                                    input.set_value("", window, cx);
                                });
                            }
                            view.status =
                                format!("Saved description for PR #{pull_request_number}");
                        }
                        Err(error) => {
                            let message = format!(
                                "Failed to save PR #{pull_request_number} description: {error}"
                            );
                            view.action_runtime
                                .finish_pull_request_description_action_failure(message.clone());
                            if let Some(restore_body) = restore_body {
                                view.set_pull_request_body(
                                    &pull_request_node_id,
                                    detail_key.as_ref(),
                                    restore_body,
                                );
                                view.report_error(message, cx);
                            } else {
                                view.status = message;
                            }
                        }
                    }

                    cx.notify();
                },
            );
        })
        .detach();
    }
}
