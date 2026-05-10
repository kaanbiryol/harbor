use gpui::{Context, Window};

use crate::workspace::{
    AppView, PullRequestDetailCacheKey, ReviewThreadUiError,
    async_updates::AppViewAsyncUpdateExt,
    reviews::{increment_pending_review_comment_count, is_local_review_thread_id},
};

impl AppView {
    pub(crate) fn open_review_thread_reply(
        &mut self,
        thread_id: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.review_composer_state.thread_reply_thread_id = Some(thread_id);
        self.review_thread_reply_error = None;
        self.review_composer_state
            .thread_reply_input
            .update(cx, |input, cx| {
                input.set_value("", window, cx);
                input.focus(window, cx);
            });
        self.status = "Opened review thread reply".to_string();
        cx.notify();
    }

    pub(crate) fn cancel_review_thread_reply(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.review_composer_state.thread_reply_thread_id = None;
        self.review_thread_reply_error = None;
        self.review_composer_state
            .thread_reply_input
            .update(cx, |input, cx| {
                input.set_value("", window, cx);
            });
        self.status = "Cancelled review thread reply".to_string();
        cx.notify();
    }

    pub(crate) fn submit_review_thread_reply(&mut self, thread_id: String, cx: &mut Context<Self>) {
        let Some(pr) = self.selected_pull_request().cloned() else {
            self.review_thread_reply_error = Some(ReviewThreadUiError {
                thread_id,
                message: "Select a pull request before replying".to_string(),
            });
            self.status = "Select a pull request before replying".to_string();
            cx.notify();
            return;
        };

        let body = self
            .review_composer_state
            .thread_reply_input
            .read(cx)
            .value()
            .to_string();
        let body = body.trim().to_string();
        if body.is_empty() {
            self.review_thread_reply_error = Some(ReviewThreadUiError {
                thread_id,
                message: "Enter a reply before sending".to_string(),
            });
            self.status = "Enter a reply before sending".to_string();
            cx.notify();
            return;
        }

        if is_local_review_thread_id(&thread_id) {
            self.review_thread_reply_error = Some(ReviewThreadUiError {
                thread_id,
                message: "Wait for the review thread to finish syncing before replying".to_string(),
            });
            self.status =
                "Wait for the review thread to finish syncing before replying".to_string();
            cx.notify();
            return;
        }

        if !self
            .review_threads
            .iter()
            .any(|thread| thread.id == thread_id)
        {
            self.review_thread_reply_error = Some(ReviewThreadUiError {
                thread_id,
                message: "Review thread is no longer loaded".to_string(),
            });
            self.status = "Review thread is no longer loaded".to_string();
            cx.notify();
            return;
        }

        let pending_review_node_id = self
            .pending_review
            .as_ref()
            .map(|pending_review| pending_review.node_id.clone());
        let increments_pending_review_count = pending_review_node_id.is_some();
        let pending_review_before_increment = if increments_pending_review_count {
            self.pending_review.clone()
        } else {
            None
        };
        let detail_key =
            PullRequestDetailCacheKey::new(pr.repo.clone(), pr.number, pr.head_sha.clone());
        let Some(optimistic_comment) =
            self.append_optimistic_review_reply(&thread_id, body.clone())
        else {
            self.review_thread_reply_error = Some(ReviewThreadUiError {
                thread_id,
                message: "Review thread is no longer loaded".to_string(),
            });
            self.status = "Review thread is no longer loaded".to_string();
            cx.notify();
            return;
        };

        if increments_pending_review_count {
            increment_pending_review_comment_count(&mut self.pending_review);
        }

        self.is_submitting_review_thread_reply = false;
        self.review_composer_state.thread_reply_thread_id = None;
        self.review_thread_reply_error = None;
        self.status = format!("Added reply locally on PR #{}; syncing", pr.number);
        cx.notify();
        let github_api = self.github_api.clone();

        cx.spawn(async move |this, cx| {
            let result = github_api
                .add_review_thread_reply(&thread_id, pending_review_node_id.as_deref(), &body)
                .await;

            this.update_or_log(
                cx,
                "failed to update review thread reply state",
                move |view, cx| {
                    match result {
                        Ok(()) => {
                            if view.selected_pull_request_detail_key().as_ref() == Some(&detail_key)
                            {
                                view.review_thread_reply_error = None;
                                view.status = format!("Posted reply on PR #{}", pr.number);
                                view.load_selected_review_data(cx);
                            }
                        }
                        Err(error) => {
                            view.remove_optimistic_review_comment_for_detail(
                                &detail_key,
                                &optimistic_comment.comment_id,
                            );
                            if increments_pending_review_count {
                                view.rollback_pending_review_comment_count_for_detail(
                                    &detail_key,
                                    pending_review_before_increment.as_ref(),
                                );
                            }

                            let message = format!("Failed to post reply: {error}");
                            if view.selected_pull_request_detail_key().as_ref() == Some(&detail_key)
                            {
                                if view.review_composer_state.thread_reply_thread_id.is_none() {
                                    view.review_composer_state.thread_reply_thread_id =
                                        Some(thread_id.clone());
                                }
                                view.review_thread_reply_error = Some(ReviewThreadUiError {
                                    thread_id,
                                    message: message.clone(),
                                });
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
