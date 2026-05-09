use gpui::{Context, Window};
use harbor_github::{GhCliTransport, GitHubClient, GitHubError, SubmitPullRequestReviewEvent};

use crate::{
    actions::DEFAULT_REQUEST_CHANGES_BODY,
    workspace::{
        AppView, PendingReviewSession, PullRequestDetailCacheKey, ReviewCommentSubmission,
        reviews::increment_pending_review_comment_count,
    },
};

impl AppView {
    pub(crate) fn submit_review_comment(
        &mut self,
        submission: ReviewCommentSubmission,
        cx: &mut Context<Self>,
    ) {
        if self.is_submitting_review_comment {
            self.status = "A review comment is already being submitted".to_string();
            cx.notify();
            return;
        }

        let Some(composer) = self.review_composer.clone() else {
            self.review_comment_error = Some("Select diff lines before commenting".to_string());
            self.status = "Select diff lines before commenting".to_string();
            cx.notify();
            return;
        };
        let Some(pr) = self.selected_pull_request().cloned() else {
            self.review_comment_error = Some("Select a pull request before commenting".to_string());
            self.status = "Select a pull request before commenting".to_string();
            cx.notify();
            return;
        };

        let body = self.review_comment_input.read(cx).value().to_string();
        let body = body.trim().to_string();
        if body.is_empty() {
            self.review_comment_error = Some("Enter a comment before sending".to_string());
            self.status = "Enter a comment before sending".to_string();
            cx.notify();
            return;
        }

        let pending_review_node_id = match submission {
            ReviewCommentSubmission::AddToReview => {
                let Some(pending_review) = self.pending_review.clone() else {
                    self.review_comment_error =
                        Some("Start a review before adding a review comment".to_string());
                    self.status = "Start a review before adding a review comment".to_string();
                    cx.notify();
                    return;
                };
                Some(pending_review.node_id)
            }
            ReviewCommentSubmission::SingleComment | ReviewCommentSubmission::StartReview => None,
        };

        if submission == ReviewCommentSubmission::StartReview && pr.node_id.is_empty() {
            self.review_comment_error =
                Some("GitHub did not return a pull request node id".to_string());
            self.status = "Cannot start review without a pull request node id".to_string();
            cx.notify();
            return;
        }

        let detail_key =
            PullRequestDetailCacheKey::new(pr.repo.clone(), pr.number, pr.head_sha.clone());
        let optimistic_comment =
            self.insert_optimistic_review_thread(composer.range.clone(), body.clone());
        let increments_pending_review_count = submission == ReviewCommentSubmission::AddToReview;
        let pending_review_before_increment = if increments_pending_review_count {
            self.pending_review.clone()
        } else {
            None
        };
        if increments_pending_review_count {
            increment_pending_review_comment_count(&mut self.pending_review);
        }

        self.is_submitting_review_comment = submission == ReviewCommentSubmission::StartReview;
        self.review_composer = None;
        self.review_line_selection = None;
        self.review_comment_error = None;
        self.status = match submission {
            ReviewCommentSubmission::SingleComment => {
                format!("Added comment locally on PR #{}; syncing", pr.number)
            }
            ReviewCommentSubmission::StartReview => {
                format!(
                    "Started pending review locally on PR #{}; syncing",
                    pr.number
                )
            }
            ReviewCommentSubmission::AddToReview => {
                format!("Added review comment locally on PR #{}; syncing", pr.number)
            }
        };
        cx.notify();

        cx.spawn(async move |this, cx| {
            let client = GitHubClient::new(GhCliTransport);
            let result = match submission {
                ReviewCommentSubmission::SingleComment => client
                    .create_pull_request_review_comment(
                        &pr.repo.owner,
                        &pr.repo.name,
                        pr.number,
                        &pr.head_sha,
                        &composer.range,
                        &body,
                    )
                    .await
                    .map(|()| None),
                ReviewCommentSubmission::StartReview => client
                    .start_pull_request_review(&pr.node_id, &pr.head_sha, &composer.range, &body)
                    .await
                    .map(Some),
                ReviewCommentSubmission::AddToReview => {
                    if let Some(pending_review_node_id) = pending_review_node_id {
                        client
                            .add_pending_review_thread(
                                &pending_review_node_id,
                                &composer.range,
                                &body,
                            )
                            .await
                            .map(|()| None)
                    } else {
                        Err(GitHubError::Transport(
                            "missing pending review id".to_string(),
                        ))
                    }
                }
            };

            if let Err(error) = this.update(cx, move |view, cx| {
                if submission == ReviewCommentSubmission::StartReview {
                    view.is_submitting_review_comment = false;
                }

                match result {
                    Ok(new_pending_review_node_id) => {
                        if let (ReviewCommentSubmission::StartReview, Some(node_id)) =
                            (submission, new_pending_review_node_id)
                        {
                            view.set_pending_review_for_detail(
                                &detail_key,
                                PendingReviewSession {
                                    node_id,
                                    comment_count: 1,
                                },
                            );
                        }

                        if view.selected_pull_request_detail_key().as_ref() == Some(&detail_key) {
                            view.review_comment_error = None;
                            view.status = match submission {
                                ReviewCommentSubmission::SingleComment => {
                                    format!("Posted comment on PR #{}", pr.number)
                                }
                                ReviewCommentSubmission::StartReview => {
                                    format!("Started pending review on PR #{}", pr.number)
                                }
                                ReviewCommentSubmission::AddToReview => {
                                    format!("Added review comment on PR #{}", pr.number)
                                }
                            };
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

                        let message = format!("Failed to submit review comment: {error}");
                        if view.selected_pull_request_detail_key().as_ref() == Some(&detail_key) {
                            if view.review_composer.is_none() {
                                view.review_composer = Some(composer);
                            }
                            view.review_comment_error = Some(message.clone());
                            view.status = message;
                        }
                    }
                }

                cx.notify();
            }) {
                crate::workspace::log_entity_update_error(
                    "failed to update review comment submission state",
                    error,
                );
            }
        })
        .detach();
    }

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

        let body = self.pending_review_body_input.read(cx).value().to_string();
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

        cx.spawn_in(window, async move |this, cx| {
            let result = GitHubClient::new(GhCliTransport)
                .submit_pull_request_review(&pending_review.node_id, event, body.as_deref())
                .await;

            if let Err(error) = this.update_in(cx, move |view, window, cx| {
                view.is_submitting_pending_review = false;
                view.is_running_pr_action = false;

                match result {
                    Ok(()) => {
                        view.pending_review = None;
                        view.pending_review_error = None;
                        view.pending_review_body_input.update(cx, |input, cx| {
                            input.set_value("", window, cx);
                        });
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
            }) {
                crate::workspace::log_entity_update_error(
                    "failed to update pending review submission state",
                    error,
                );
            }
        })
        .detach();
    }
}
