use gpui::{Context, Window};

use crate::workspace::{AppView, async_updates::AppViewAsyncUpdateExt};

impl AppView {
    pub(crate) fn open_review_comment_edit(
        &mut self,
        comment_id: String,
        body: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.review_state
            .review_composer_state
            .open_comment_edit(comment_id);
        self.review_state.clear_review_comment_edit_error();
        self.review_state
            .review_composer_state
            .comment_edit_input
            .update(cx, |input, cx| {
                input.set_value(body, window, cx);
                input.focus(window, cx);
            });
        self.status = "Opened review comment editor".to_string();
        cx.notify();
    }

    pub(crate) fn cancel_review_comment_edit(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.review_state.review_composer_state.clear();
        self.review_state.clear_review_comment_edit_error();
        self.review_state
            .review_composer_state
            .comment_edit_input
            .update(cx, |input, cx| {
                input.set_value("", window, cx);
            });
        self.status = "Cancelled review comment edit".to_string();
        cx.notify();
    }

    pub(crate) fn submit_review_comment_edit(
        &mut self,
        comment_id: String,
        cx: &mut Context<Self>,
    ) {
        if self.review_state.is_submitting_review_comment_edit() {
            self.status = "A review comment edit is already being submitted".to_string();
            cx.notify();
            return;
        }

        let Some(pr) = self.selected_pull_request().cloned() else {
            self.review_state
                .set_review_comment_edit_error(comment_id, "Select a pull request before editing");
            self.status = "Select a pull request before editing".to_string();
            cx.notify();
            return;
        };

        let Some(comment) = self.review_comment(&comment_id) else {
            self.review_state
                .set_review_comment_edit_error(comment_id, "Review comment is no longer loaded");
            self.status = "Review comment is no longer loaded".to_string();
            cx.notify();
            return;
        };

        if !comment.viewer_can_update {
            self.review_state.set_review_comment_edit_error(
                comment_id,
                "GitHub does not allow you to edit this comment",
            );
            self.status = "GitHub does not allow you to edit this comment".to_string();
            cx.notify();
            return;
        }

        let body = self
            .review_state
            .review_composer_state
            .comment_edit_input
            .read(cx)
            .value()
            .to_string();
        let body = body.trim().to_string();
        if body.is_empty() {
            self.review_state
                .set_review_comment_edit_error(comment_id, "Enter a comment before saving");
            self.status = "Enter a comment before saving".to_string();
            cx.notify();
            return;
        }

        self.review_state
            .start_review_comment_edit_submission(comment_id.clone());
        self.status = format!("Updating review comment on PR #{}", pr.number);
        cx.notify();
        let github_api = self.github_api.clone();

        cx.spawn(async move |this, cx| {
            let result = github_api.update_review_comment(&comment_id, &body).await;

            this.update_or_log(
                cx,
                "failed to update review comment edit state",
                move |view, cx| {
                    view.review_state.finish_review_comment_edit_submission();

                    match result {
                        Ok(()) => {
                            if let Some(comment) = view.review_comment_mut(&comment_id) {
                                comment.body = body;
                            }
                            view.review_state.review_composer_state.clear();
                            view.review_state.clear_review_comment_edit_error();
                            view.status = format!("Updated review comment on PR #{}", pr.number);
                            view.load_selected_review_data(cx);
                        }
                        Err(error) => {
                            let message = format!("Failed to update review comment: {error}");
                            view.review_state
                                .set_review_comment_edit_error(comment_id, message.clone());
                            view.status = message;
                        }
                    }

                    cx.notify();
                },
            );
        })
        .detach();
    }

    pub(crate) fn delete_review_comment(&mut self, comment_id: String, cx: &mut Context<Self>) {
        if self.review_state.comment_action_running() {
            self.status = "A review comment action is already running".to_string();
            cx.notify();
            return;
        }

        let Some(pr) = self.selected_pull_request().cloned() else {
            self.review_state.set_review_comment_action_error(
                comment_id,
                "Select a pull request before deleting",
            );
            self.status = "Select a pull request before deleting".to_string();
            cx.notify();
            return;
        };

        let Some(comment) = self.review_comment(&comment_id) else {
            self.review_state
                .set_review_comment_action_error(comment_id, "Review comment is no longer loaded");
            self.status = "Review comment is no longer loaded".to_string();
            cx.notify();
            return;
        };

        if !comment.viewer_can_delete {
            self.review_state.set_review_comment_action_error(
                comment_id,
                "GitHub does not allow you to delete this comment",
            );
            self.status = "GitHub does not allow you to delete this comment".to_string();
            cx.notify();
            return;
        }

        self.review_state
            .start_review_comment_action(comment_id.clone());
        self.status = format!("Deleting review comment on PR #{}", pr.number);
        cx.notify();
        let github_api = self.github_api.clone();

        cx.spawn(async move |this, cx| {
            let result = github_api.delete_review_comment(&comment_id).await;

            this.update_or_log(
                cx,
                "failed to update review comment action state",
                move |view, cx| {
                    view.review_state.finish_review_comment_action();

                    match result {
                        Ok(()) => {
                            view.remove_review_comment(&comment_id);
                            view.review_state
                                .review_composer_state
                                .take_active_comment_edit_if(|active_id| active_id == comment_id);
                            view.review_state.clear_review_comment_action_error();
                            view.sync_unresolved_thread_count();
                            view.status = format!("Deleted review comment on PR #{}", pr.number);
                            view.load_selected_review_data(cx);
                        }
                        Err(error) => {
                            let message = format!("Failed to delete review comment: {error}");
                            view.review_state
                                .set_review_comment_action_error(comment_id, message.clone());
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
