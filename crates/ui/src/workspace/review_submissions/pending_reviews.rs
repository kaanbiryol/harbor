use gpui::{Context, Window};
use harbor_github::SubmitPullRequestReviewEvent;

use crate::{
    actions::DEFAULT_REQUEST_CHANGES_BODY,
    workspace::{AppView, async_updates::AppViewAsyncUpdateExt},
};

impl AppView {
    pub(crate) fn submit_pending_pull_request_review(
        &mut self,
        event: SubmitPullRequestReviewEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.review_state.is_submitting_pending_review()
            || self.action_runtime.pull_request_action_running()
        {
            self.status = "A pull request action is already running".to_string();
            cx.notify();
            return;
        }

        let Some(pending_review) = self.review_state.pending_review_cloned() else {
            self.review_state
                .set_pending_review_error("No pending review to submit");
            self.status = "No pending review to submit".to_string();
            cx.notify();
            return;
        };
        let Some(pr) = self.selected_pull_request().cloned() else {
            self.review_state
                .set_pending_review_error("Select a pull request before submitting a review");
            self.status = "Select a pull request before submitting a review".to_string();
            cx.notify();
            return;
        };

        let body = self
            .review_state
            .review_composer_state
            .pending_review_body_input
            .read(cx)
            .value()
            .to_string();
        if event == SubmitPullRequestReviewEvent::Comment
            && pending_review.comment_count == 0
            && body.trim().is_empty()
        {
            self.review_state
                .set_pending_review_error("Add a review summary or at least one pending comment");
            self.status = "Add a review summary or at least one pending comment".to_string();
            cx.notify();
            return;
        }

        let body = match event {
            SubmitPullRequestReviewEvent::RequestChanges if body.trim().is_empty() => {
                Some(DEFAULT_REQUEST_CHANGES_BODY.to_string())
            }
            _ => {
                let trimmed = body.trim();
                (!trimmed.is_empty()).then(|| trimmed.to_string())
            }
        };

        self.review_state.start_pending_review_submission();
        self.action_runtime.start_pull_request_action();
        self.status = format!("Submitting pending review on PR #{}", pr.number);
        cx.notify();
        let github_api = self.github_api.clone();

        cx.spawn_in(window, async move |this, cx| {
            let result = github_api
                .submit_pull_request_review(&pending_review.node_id, event, body.as_deref())
                .await;

            this.update_in_or_log(
                cx,
                "failed to update pending review submission state",
                move |view, window, cx| {
                    view.review_state.finish_pending_review_submission();

                    match result {
                        Ok(()) => {
                            view.action_runtime.finish_pull_request_action();
                            view.review_state.clear_pending_review();
                            view.review_state.clear_pending_review_error();
                            view.review_state
                                .review_composer_state
                                .pending_review_body_input
                                .update(cx, |input, cx| {
                                    input.set_value("", window, cx);
                                });
                            view.status = format!("Submitted pending review on PR #{}", pr.number);
                            view.reload_pull_request_inbox(cx);
                        }
                        Err(error) => {
                            view.action_runtime.finish_pull_request_action();
                            let message = format!("Failed to submit pending review: {error}");
                            view.review_state.set_pending_review_error(message.clone());
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
