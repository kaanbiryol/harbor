use gpui::Context;
use harbor_github::GitHubError;

use crate::workspace::{
    AppView, PendingReviewSession, PullRequestDetailCacheKey, ReviewCommentSubmission,
    async_updates::AppViewAsyncUpdateExt, reviews::increment_pending_review_comment_count,
};

impl AppView {
    pub(crate) fn submit_review_comment(
        &mut self,
        submission: ReviewCommentSubmission,
        cx: &mut Context<Self>,
    ) {
        if self.review_state.is_submitting_review_comment {
            self.status = "A review comment is already being submitted".to_string();
            cx.notify();
            return;
        }

        let Some(composer) = self
            .review_state
            .review_composer_state
            .inline_composer()
            .cloned()
        else {
            self.review_state.review_comment_error =
                Some("Select diff lines before commenting".to_string());
            self.status = "Select diff lines before commenting".to_string();
            cx.notify();
            return;
        };
        let line_selection = self
            .review_state
            .review_composer_state
            .line_selection()
            .cloned();
        let Some(pr) = self.selected_pull_request().cloned() else {
            self.review_state.review_comment_error =
                Some("Select a pull request before commenting".to_string());
            self.status = "Select a pull request before commenting".to_string();
            cx.notify();
            return;
        };

        let body = self
            .review_state
            .review_composer_state
            .comment_input
            .read(cx)
            .value()
            .to_string();
        let body = body.trim().to_string();
        if body.is_empty() {
            self.review_state.review_comment_error =
                Some("Enter a comment before sending".to_string());
            self.status = "Enter a comment before sending".to_string();
            cx.notify();
            return;
        }

        let pending_review_node_id = match submission {
            ReviewCommentSubmission::AddToReview => {
                let Some(pending_review) = self.review_state.pending_review.clone() else {
                    self.review_state.review_comment_error =
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
            self.review_state.review_comment_error =
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
            self.review_state.pending_review.clone()
        } else {
            None
        };
        if increments_pending_review_count {
            increment_pending_review_comment_count(&mut self.review_state.pending_review);
        }

        self.review_state.is_submitting_review_comment =
            submission == ReviewCommentSubmission::StartReview;
        self.review_state.review_composer_state.clear();
        self.review_state.review_comment_error = None;
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
        let github_api = self.github_api.clone();

        cx.spawn(async move |this, cx| {
            let result = match submission {
                ReviewCommentSubmission::SingleComment => github_api
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
                ReviewCommentSubmission::StartReview => github_api
                    .start_pull_request_review(&pr.node_id, &pr.head_sha, &composer.range, &body)
                    .await
                    .map(Some),
                ReviewCommentSubmission::AddToReview => {
                    if let Some(pending_review_node_id) = pending_review_node_id {
                        github_api
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

            this.update_or_log(
                cx,
                "failed to update review comment submission state",
                move |view, cx| {
                    if submission == ReviewCommentSubmission::StartReview {
                        view.review_state.is_submitting_review_comment = false;
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

                            if view.selected_pull_request_detail_key().as_ref() == Some(&detail_key)
                            {
                                view.review_state.review_comment_error = None;
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
                            if view.selected_pull_request_detail_key().as_ref() == Some(&detail_key)
                            {
                                if view
                                    .review_state
                                    .review_composer_state
                                    .inline_composer()
                                    .is_none()
                                    && let Some(line_selection) = line_selection
                                {
                                    view.review_state
                                        .review_composer_state
                                        .open_inline(composer, line_selection);
                                }
                                view.review_state.review_comment_error = Some(message.clone());
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
