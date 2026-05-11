use gpui::Context;
use harbor_domain::ReviewThreadState;

use crate::workspace::{AppView, async_updates::AppViewAsyncUpdateExt};

impl AppView {
    pub(crate) fn set_review_thread_resolved(
        &mut self,
        thread_id: String,
        resolved: bool,
        cx: &mut Context<Self>,
    ) {
        if self.review_state.thread_action_running() {
            self.status = "A review thread action is already running".to_string();
            cx.notify();
            return;
        }

        let Some(pr) = self.selected_pull_request().cloned() else {
            self.review_state.set_review_thread_action_error(
                thread_id,
                "Select a pull request before updating a thread",
            );
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
            .review_state
            .review_threads
            .iter()
            .find(|thread| thread.id == thread_id)
            .map(|thread| thread.state);
        self.set_review_thread_state(&thread_id, desired_state);
        self.review_state
            .set_review_thread_state_override(thread_id.clone(), desired_state);
        self.sync_unresolved_thread_count();
        self.review_state
            .start_review_thread_action(thread_id.clone());
        self.status = if resolved {
            format!("Resolving review thread on PR #{}", pr.number)
        } else {
            format!("Reopening review thread on PR #{}", pr.number)
        };
        cx.notify();
        let github_api = self.github_api.clone();

        cx.spawn(async move |this, cx| {
            let result = if resolved {
                github_api.resolve_review_thread(&thread_id).await
            } else {
                github_api.unresolve_review_thread(&thread_id).await
            };

            this.update_or_log(
                cx,
                "failed to update review thread action state",
                move |view, cx| {
                    view.review_state.finish_review_thread_action();

                    match result {
                        Ok(()) => {
                            view.set_review_thread_state(&thread_id, desired_state);
                            view.sync_unresolved_thread_count();
                            view.review_state.clear_review_thread_action_error();
                            view.status = if resolved {
                                format!("Resolved review thread on PR #{}", pr.number)
                            } else {
                                format!("Reopened review thread on PR #{}", pr.number)
                            };
                            view.load_selected_review_data(cx);
                        }
                        Err(error) => {
                            view.review_state
                                .remove_review_thread_state_override(&thread_id);
                            if let Some(previous_state) = previous_state {
                                view.set_review_thread_state(&thread_id, previous_state);
                                view.sync_unresolved_thread_count();
                            }
                            let message = if resolved {
                                format!("Failed to resolve review thread: {error}")
                            } else {
                                format!("Failed to reopen review thread: {error}")
                            };
                            view.review_state
                                .set_review_thread_action_error(thread_id, message.clone());
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
