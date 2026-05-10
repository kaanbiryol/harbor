use gpui::Context;
use harbor_domain::ReviewThreadState;
use harbor_github::{GhCliTransport, GitHubClient};

use crate::workspace::{AppView, ReviewThreadUiError, async_updates::AppViewAsyncUpdateExt};

impl AppView {
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

            this.update_or_log(
                cx,
                "failed to update review thread action state",
                move |view, cx| {
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
                },
            );
        })
        .detach();
    }
}
