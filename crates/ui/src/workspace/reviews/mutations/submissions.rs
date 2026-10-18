use gpui::{Context, Window};
use harbor_domain::SubmitPullRequestReviewEvent;
use harbor_github::GitHubError;

use crate::{
    actions::DEFAULT_REQUEST_CHANGES_BODY,
    workspace::{
        AppView, PendingReviewSession, PullRequestDetailCacheKey, ReviewCommentSubmission,
        async_updates::AppViewAsyncUpdateExt,
    },
};

impl AppView {
    pub(crate) fn submit_review_comment(
        &mut self,
        submission: ReviewCommentSubmission,
        cx: &mut Context<Self>,
    ) {
        let auth_generation = self.auth_generation;
        if self.review_state.is_submitting_review_comment() {
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
            self.review_state
                .set_review_comment_error("Select diff lines before commenting");
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
            self.review_state
                .set_review_comment_error("Select a pull request before commenting");
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
            self.review_state
                .set_review_comment_error("Enter a comment before sending");
            self.status = "Enter a comment before sending".to_string();
            cx.notify();
            return;
        }

        let pending_review_node_id = match submission {
            ReviewCommentSubmission::AddToReview => {
                let Some(pending_review) = self.review_state.pending_review_cloned() else {
                    self.review_state
                        .set_review_comment_error("Start a review before adding a review comment");
                    self.status = "Start a review before adding a review comment".to_string();
                    cx.notify();
                    return;
                };
                Some(pending_review.node_id)
            }
            ReviewCommentSubmission::SingleComment | ReviewCommentSubmission::StartReview => None,
        };

        if submission == ReviewCommentSubmission::StartReview && pr.node_id.is_empty() {
            self.review_state
                .set_review_comment_error("GitHub did not return a pull request node id");
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
            self.review_state.pending_review_cloned()
        } else {
            None
        };
        if increments_pending_review_count {
            self.review_state.increment_pending_review_comment_count();
        }

        self.review_state
            .start_review_comment_submission(submission == ReviewCommentSubmission::StartReview);
        self.sync_diff_list_items(cx);
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
        let github_api = self.github_api_for_request();

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

            this.update_in_session_or_log(
                cx,
                auth_generation,
                "failed to update review comment submission state",
                move |view, cx| {
                    if submission == ReviewCommentSubmission::StartReview {
                        view.review_state.finish_review_comment_submission();
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
                                view.review_state.clear_review_comment_error();
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
                                view.sync_diff_list_items(cx);
                                view.review_state.set_review_comment_error(message.clone());
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

impl AppView {
    pub(crate) fn submit_pending_pull_request_review(
        &mut self,
        event: SubmitPullRequestReviewEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let auth_generation = self.auth_generation;
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
        self.action_runtime
            .start_pull_request_action(crate::actions::PullRequestActionKind::Review);
        self.status = format!("Submitting pending review on PR #{}", pr.number);
        cx.notify();
        let github_api = self.github_api_for_request();

        cx.spawn_in(window, async move |this, cx| {
            let result = github_api
                .submit_pull_request_review(&pending_review.node_id, event, body.as_deref())
                .await;

            this.update_window_in_session_or_log(
                cx,
                auth_generation,
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
