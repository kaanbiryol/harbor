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
        if self.is_submitting_pending_review || self.is_running_pr_action {
            self.status = "A pull request action is already running".to_string();
            cx.notify();
            return;
        }

        let Some(pending_review) = self.pending_review.clone() else {
            self.pending_review_error = Some("No pending review to submit".to_string());
            self.status = "No pending review to submit".to_string();
            cx.notify();
            return;
        };
        let Some(pr) = self.selected_pull_request().cloned() else {
            self.pending_review_error =
                Some("Select a pull request before submitting a review".to_string());
            self.status = "Select a pull request before submitting a review".to_string();
            cx.notify();
            return;
        };

        let body = self
            .review_composer_state
            .pending_review_body_input
            .read(cx)
            .value()
            .to_string();
        if event == SubmitPullRequestReviewEvent::Comment
            && pending_review.comment_count == 0
            && body.trim().is_empty()
        {
            self.pending_review_error =
                Some("Add a review summary or at least one pending comment".to_string());
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

        self.is_submitting_pending_review = true;
        self.is_running_pr_action = true;
        self.pending_review_error = None;
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
                    view.is_submitting_pending_review = false;
                    view.is_running_pr_action = false;

                    match result {
                        Ok(()) => {
                            view.pending_review = None;
                            view.pending_review_error = None;
                            view.review_composer_state.pending_review_body_input.update(
                                cx,
                                |input, cx| {
                                    input.set_value("", window, cx);
                                },
                            );
                            view.status = format!("Submitted pending review on PR #{}", pr.number);
                            view.reload_pull_request_inbox(cx);
                        }
                        Err(error) => {
                            let message = format!("Failed to submit pending review: {error}");
                            view.pending_review_error = Some(message.clone());
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
