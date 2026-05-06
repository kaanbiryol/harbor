use gpui::{Context, Entity, Window};
use gpui_component::input::{InputEvent, InputState};
use harbor_domain::{ReactionContent, ReviewThreadState};
use harbor_github::{GhCliTransport, GitHubClient};

use crate::{
    actions::PanelTab,
    workspace::{
        AppView, PullRequestDetailCacheKey, ReviewCommentUiError, ReviewLineSelection,
        ReviewLineTarget, ReviewReactionAction, ReviewThreadUiError,
        reviews::{
            ReviewReactionKey, increment_pending_review_comment_count, is_local_review_thread_id,
            review_comment_range_label, review_composer_from_selection, review_reaction,
        },
    },
};

impl AppView {
    pub(crate) fn start_review_line_selection(
        &mut self,
        target: ReviewLineTarget,
        cx: &mut Context<Self>,
    ) {
        self.review_line_selection = Some(ReviewLineSelection {
            anchor: target.clone(),
            current: target,
        });
        self.review_composer = None;
        self.review_comment_error = None;
        self.active_tab = PanelTab::Diff;
        self.status = "Started review line selection".to_string();
        cx.notify();
    }

    pub(crate) fn extend_review_line_selection(
        &mut self,
        target: ReviewLineTarget,
        cx: &mut Context<Self>,
    ) {
        if let Some(selection) = self.review_line_selection.as_mut() {
            selection.current = target;
        }
        cx.notify();
    }

    pub(crate) fn finish_review_line_selection(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(selection) = self.review_line_selection.take() else {
            return;
        };

        match review_composer_from_selection(&selection.anchor, &selection.current) {
            Ok(composer) => {
                let range = composer.range.clone();
                let label = review_comment_range_label(&range);
                self.review_comment_input.update(cx, |input, cx| {
                    input.set_value("", window, cx);
                    input.focus(window, cx);
                });
                self.review_composer = Some(composer);
                self.review_comment_error = None;
                self.status = format!("Opened review composer for {label}");
            }
            Err(message) => {
                self.review_composer = None;
                self.review_comment_error = Some(message.clone());
                self.status = message;
            }
        }

        cx.notify();
    }

    pub(crate) fn cancel_review_composer(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.clear_review_composer_state();
        self.review_comment_input.update(cx, |input, cx| {
            input.set_value("", window, cx);
        });
        self.status = "Cancelled review comment".to_string();
        cx.notify();
    }

    pub(crate) fn open_review_thread_reply(
        &mut self,
        thread_id: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.review_thread_reply_thread_id = Some(thread_id);
        self.review_thread_reply_error = None;
        self.review_thread_reply_input.update(cx, |input, cx| {
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
        self.review_thread_reply_thread_id = None;
        self.review_thread_reply_error = None;
        self.review_thread_reply_input.update(cx, |input, cx| {
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

        let body = self.review_thread_reply_input.read(cx).value().to_string();
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
        self.review_thread_reply_thread_id = None;
        self.review_thread_reply_error = None;
        self.status = format!("Added reply locally on PR #{}; syncing", pr.number);
        cx.notify();

        cx.spawn(async move |this, cx| {
            let result = GitHubClient::new(GhCliTransport)
                .add_review_thread_reply(&thread_id, pending_review_node_id.as_deref(), &body)
                .await;

            if let Err(error) = this.update(cx, move |view, cx| {
                match result {
                    Ok(()) => {
                        if view.selected_pull_request_detail_key().as_ref() == Some(&detail_key) {
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
                        if view.selected_pull_request_detail_key().as_ref() == Some(&detail_key) {
                            if view.review_thread_reply_thread_id.is_none() {
                                view.review_thread_reply_thread_id = Some(thread_id.clone());
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
            }) {
                eprintln!("failed to update review thread reply state: {error}");
            }
        })
        .detach();
    }

    pub(crate) fn set_review_thread_resolved(
        &mut self,
        thread_id: String,
        resolved: bool,
        cx: &mut Context<Self>,
    ) {
        if self.review_thread_action_thread_id.is_some() {
            self.status = "A review thread action is already running".to_string();
            cx.notify();
            return;
        }

        let Some(pr) = self.selected_pull_request().cloned() else {
            self.review_thread_action_error = Some(ReviewThreadUiError {
                thread_id,
                message: "Select a pull request before updating a thread".to_string(),
            });
            self.status = "Select a pull request before updating a thread".to_string();
            cx.notify();
            return;
        };

        let desired_state = if resolved {
            ReviewThreadState::Resolved
        } else {
            ReviewThreadState::Unresolved
        };
        let previous_state = self
            .review_threads
            .iter()
            .find(|thread| thread.id == thread_id)
            .map(|thread| thread.state);
        self.set_review_thread_state(&thread_id, desired_state);
        self.review_thread_state_overrides
            .insert(thread_id.clone(), desired_state);
        self.sync_unresolved_thread_count();
        self.review_thread_action_thread_id = Some(thread_id.clone());
        self.review_thread_action_error = None;
        self.status = if resolved {
            format!("Resolving review thread on PR #{}", pr.number)
        } else {
            format!("Reopening review thread on PR #{}", pr.number)
        };
        cx.notify();

        cx.spawn(async move |this, cx| {
            let client = GitHubClient::new(GhCliTransport);
            let result = if resolved {
                client.resolve_review_thread(&thread_id).await
            } else {
                client.unresolve_review_thread(&thread_id).await
            };

            if let Err(error) = this.update(cx, move |view, cx| {
                view.review_thread_action_thread_id = None;

                match result {
                    Ok(()) => {
                        view.set_review_thread_state(&thread_id, desired_state);
                        view.sync_unresolved_thread_count();
                        view.review_thread_action_error = None;
                        view.status = if resolved {
                            format!("Resolved review thread on PR #{}", pr.number)
                        } else {
                            format!("Reopened review thread on PR #{}", pr.number)
                        };
                        view.load_selected_review_data(cx);
                    }
                    Err(error) => {
                        view.review_thread_state_overrides.remove(&thread_id);
                        if let Some(previous_state) = previous_state {
                            view.set_review_thread_state(&thread_id, previous_state);
                            view.sync_unresolved_thread_count();
                        }
                        let message = if resolved {
                            format!("Failed to resolve review thread: {error}")
                        } else {
                            format!("Failed to reopen review thread: {error}")
                        };
                        view.review_thread_action_error = Some(ReviewThreadUiError {
                            thread_id,
                            message: message.clone(),
                        });
                        view.status = message;
                    }
                }

                cx.notify();
            }) {
                eprintln!("failed to update review thread action state: {error}");
            }
        })
        .detach();
    }

    pub(crate) fn open_review_comment_edit(
        &mut self,
        comment_id: String,
        body: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.review_comment_edit_comment_id = Some(comment_id);
        self.review_comment_edit_error = None;
        self.review_comment_edit_input.update(cx, |input, cx| {
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
        self.review_comment_edit_comment_id = None;
        self.review_comment_edit_error = None;
        self.review_comment_edit_input.update(cx, |input, cx| {
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
        if self.is_submitting_review_comment_edit {
            self.status = "A review comment edit is already being submitted".to_string();
            cx.notify();
            return;
        }

        let Some(pr) = self.selected_pull_request().cloned() else {
            self.review_comment_edit_error = Some(ReviewCommentUiError {
                comment_id,
                message: "Select a pull request before editing".to_string(),
            });
            self.status = "Select a pull request before editing".to_string();
            cx.notify();
            return;
        };

        let Some(comment) = self.review_comment(&comment_id) else {
            self.review_comment_edit_error = Some(ReviewCommentUiError {
                comment_id,
                message: "Review comment is no longer loaded".to_string(),
            });
            self.status = "Review comment is no longer loaded".to_string();
            cx.notify();
            return;
        };

        if !comment.viewer_can_update {
            self.review_comment_edit_error = Some(ReviewCommentUiError {
                comment_id,
                message: "GitHub does not allow you to edit this comment".to_string(),
            });
            self.status = "GitHub does not allow you to edit this comment".to_string();
            cx.notify();
            return;
        }

        let body = self.review_comment_edit_input.read(cx).value().to_string();
        let body = body.trim().to_string();
        if body.is_empty() {
            self.review_comment_edit_error = Some(ReviewCommentUiError {
                comment_id,
                message: "Enter a comment before saving".to_string(),
            });
            self.status = "Enter a comment before saving".to_string();
            cx.notify();
            return;
        }

        self.is_submitting_review_comment_edit = true;
        self.review_comment_edit_comment_id = Some(comment_id.clone());
        self.review_comment_edit_error = None;
        self.status = format!("Updating review comment on PR #{}", pr.number);
        cx.notify();

        cx.spawn(async move |this, cx| {
            let result = GitHubClient::new(GhCliTransport)
                .update_review_comment(&comment_id, &body)
                .await;

            if let Err(error) = this.update(cx, move |view, cx| {
                view.is_submitting_review_comment_edit = false;

                match result {
                    Ok(()) => {
                        if let Some(comment) = view.review_comment_mut(&comment_id) {
                            comment.body = body;
                        }
                        view.review_comment_edit_comment_id = None;
                        view.review_comment_edit_error = None;
                        view.status = format!("Updated review comment on PR #{}", pr.number);
                        view.load_selected_review_data(cx);
                    }
                    Err(error) => {
                        let message = format!("Failed to update review comment: {error}");
                        view.review_comment_edit_error = Some(ReviewCommentUiError {
                            comment_id,
                            message: message.clone(),
                        });
                        view.status = message;
                    }
                }

                cx.notify();
            }) {
                eprintln!("failed to update review comment edit state: {error}");
            }
        })
        .detach();
    }

    pub(crate) fn delete_review_comment(&mut self, comment_id: String, cx: &mut Context<Self>) {
        if self.review_comment_action_comment_id.is_some() {
            self.status = "A review comment action is already running".to_string();
            cx.notify();
            return;
        }

        let Some(pr) = self.selected_pull_request().cloned() else {
            self.review_comment_action_error = Some(ReviewCommentUiError {
                comment_id,
                message: "Select a pull request before deleting".to_string(),
            });
            self.status = "Select a pull request before deleting".to_string();
            cx.notify();
            return;
        };

        let Some(comment) = self.review_comment(&comment_id) else {
            self.review_comment_action_error = Some(ReviewCommentUiError {
                comment_id,
                message: "Review comment is no longer loaded".to_string(),
            });
            self.status = "Review comment is no longer loaded".to_string();
            cx.notify();
            return;
        };

        if !comment.viewer_can_delete {
            self.review_comment_action_error = Some(ReviewCommentUiError {
                comment_id,
                message: "GitHub does not allow you to delete this comment".to_string(),
            });
            self.status = "GitHub does not allow you to delete this comment".to_string();
            cx.notify();
            return;
        }

        self.review_comment_action_comment_id = Some(comment_id.clone());
        self.review_comment_action_error = None;
        self.status = format!("Deleting review comment on PR #{}", pr.number);
        cx.notify();

        cx.spawn(async move |this, cx| {
            let result = GitHubClient::new(GhCliTransport)
                .delete_review_comment(&comment_id)
                .await;

            if let Err(error) = this.update(cx, move |view, cx| {
                view.review_comment_action_comment_id = None;

                match result {
                    Ok(()) => {
                        view.remove_review_comment(&comment_id);
                        view.review_comment_edit_comment_id = view
                            .review_comment_edit_comment_id
                            .take()
                            .filter(|active_id| active_id != &comment_id);
                        view.review_comment_action_error = None;
                        view.sync_unresolved_thread_count();
                        view.status = format!("Deleted review comment on PR #{}", pr.number);
                        view.load_selected_review_data(cx);
                    }
                    Err(error) => {
                        let message = format!("Failed to delete review comment: {error}");
                        view.review_comment_action_error = Some(ReviewCommentUiError {
                            comment_id,
                            message: message.clone(),
                        });
                        view.status = message;
                    }
                }

                cx.notify();
            }) {
                eprintln!("failed to update review comment action state: {error}");
            }
        })
        .detach();
    }

    pub(crate) fn toggle_review_comment_reaction(
        &mut self,
        comment_id: String,
        content: ReactionContent,
        cx: &mut Context<Self>,
    ) {
        if self.review_reaction_action.is_some() {
            self.status = "A review reaction action is already running".to_string();
            cx.notify();
            return;
        }

        let Some(pr) = self.selected_pull_request().cloned() else {
            self.review_reaction_error = Some(ReviewCommentUiError {
                comment_id,
                message: "Select a pull request before reacting".to_string(),
            });
            self.status = "Select a pull request before reacting".to_string();
            cx.notify();
            return;
        };

        let Some(comment) = self.review_comment(&comment_id) else {
            self.review_reaction_error = Some(ReviewCommentUiError {
                comment_id,
                message: "Review comment is no longer loaded".to_string(),
            });
            self.status = "Review comment is no longer loaded".to_string();
            cx.notify();
            return;
        };

        if !comment.viewer_can_react {
            self.review_reaction_error = Some(ReviewCommentUiError {
                comment_id,
                message: "GitHub does not allow you to react to this comment".to_string(),
            });
            self.status = "GitHub does not allow you to react to this comment".to_string();
            cx.notify();
            return;
        }

        let had_reacted =
            review_reaction(comment, content).is_some_and(|reaction| reaction.viewer_has_reacted);
        let viewer_has_reacted = !had_reacted;
        let reaction_key = ReviewReactionKey::new(comment_id.clone(), content);
        self.set_review_comment_reaction(&comment_id, content, viewer_has_reacted);
        self.review_reaction_overrides
            .insert(reaction_key.clone(), viewer_has_reacted);
        self.review_reaction_action = Some(ReviewReactionAction {
            comment_id: comment_id.clone(),
            content,
        });
        self.review_reaction_error = None;
        self.status = format!("Updating reaction on PR #{}", pr.number);
        cx.notify();

        cx.spawn(async move |this, cx| {
            let client = GitHubClient::new(GhCliTransport);
            let result = if had_reacted {
                client
                    .remove_review_comment_reaction(&comment_id, content)
                    .await
            } else {
                client
                    .add_review_comment_reaction(&comment_id, content)
                    .await
            };

            if let Err(error) = this.update(cx, move |view, cx| {
                view.review_reaction_action = None;

                match result {
                    Ok(()) => {
                        view.review_reaction_error = None;
                        view.status = format!("Updated reaction on PR #{}", pr.number);
                        view.load_selected_review_data(cx);
                    }
                    Err(error) => {
                        view.review_reaction_overrides.remove(&reaction_key);
                        view.set_review_comment_reaction(&comment_id, content, had_reacted);
                        let message = format!("Failed to update reaction: {error}");
                        view.review_reaction_error = Some(ReviewCommentUiError {
                            comment_id,
                            message: message.clone(),
                        });
                        view.status = message;
                    }
                }

                cx.notify();
            }) {
                eprintln!("failed to update review reaction state: {error}");
            }
        })
        .detach();
    }

    pub(super) fn on_review_input_event(
        &mut self,
        _: &Entity<InputState>,
        event: &InputEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if matches!(event, InputEvent::Change) {
            cx.notify();
        }
    }
}
