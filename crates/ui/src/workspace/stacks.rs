use std::sync::Arc;

use gpui::{AppContext, Context};
use harbor_domain::{PullRequest, StackMergeAction, StackMergeOutcome};

use crate::{
    actions::PullRequestActionKind,
    workspace::{AppView, SelectedPullRequestTaskKind, async_updates::AppViewAsyncUpdateExt},
};

impl AppView {
    pub(super) fn load_selected_pull_request_stack(&mut self, cx: &mut Context<Self>) {
        let Some(pull_request) = self.selected_pull_request() else {
            return;
        };
        let Some(membership) = pull_request.stack.as_ref() else {
            self.detail_state.stack = None;
            self.detail_state.stack_load.reset();
            self.detail_state.stack_expanded = false;
            return;
        };
        let number = membership.number;
        if self.detail_state.stack_load.is_loading() {
            return;
        }
        let Some(detail_key) = self.selected_pull_request_detail_key() else {
            return;
        };
        let repository = pull_request.repo.clone();
        let github_api = self.github_api_for_request();
        self.detail_state.stack_load.start();
        self.spawn_selected_pull_request_load(
            SelectedPullRequestTaskKind::Stack,
            detail_key,
            "failed to update pull request stack",
            async move { github_api.get_pull_request_stack(&repository, number).await },
            move |view, result, cx| {
                if view
                    .selected_pull_request()
                    .and_then(|pull_request| pull_request.stack.as_ref())
                    .map(|membership| membership.number)
                    != Some(number)
                {
                    view.detail_state.stack_load.reset();
                    view.load_selected_pull_request_stack(cx);
                    return;
                }
                match result {
                    Ok(stack) => {
                        view.detail_state.stack = Some(Arc::new(stack));
                        view.detail_state.stack_load.succeed();
                    }
                    Err(error) => {
                        view.detail_state.stack = None;
                        view.detail_state.stack_load.fail(error.to_string());
                    }
                }
            },
            cx,
        );
    }

    pub(super) fn select_stack_pull_request(
        &mut self,
        pull_request: PullRequest,
        cx: &mut Context<Self>,
    ) {
        let stack = self.detail_state.stack.clone();
        let index = match self
            .pull_requests
            .iter_mut()
            .enumerate()
            .find(|(_, existing)| {
                existing.repo == pull_request.repo && existing.number == pull_request.number
            }) {
            Some((index, existing)) => {
                existing.stack = pull_request.stack;
                index
            }
            None => {
                self.pull_requests.push(pull_request);
                self.pull_requests.len() - 1
            }
        };
        self.select_pull_request(index, cx);
        self.detail_state.stack = stack;
        self.detail_state.stack_expanded = false;
        cx.notify();
    }

    pub(super) fn select_stack_down(
        &mut self,
        _: &crate::actions::SelectStackDown,
        _: &mut gpui::Window,
        cx: &mut Context<Self>,
    ) {
        self.select_adjacent_stack_pull_request(false, cx);
    }

    pub(super) fn select_stack_up(
        &mut self,
        _: &crate::actions::SelectStackUp,
        _: &mut gpui::Window,
        cx: &mut Context<Self>,
    ) {
        self.select_adjacent_stack_pull_request(true, cx);
    }

    fn select_adjacent_stack_pull_request(&mut self, up: bool, cx: &mut Context<Self>) {
        let Some(selected) = self.selected_pull_request() else {
            return;
        };
        let Some(stack) = self.detail_state.stack.as_ref() else {
            return;
        };
        let Some(position) = stack
            .pull_requests
            .iter()
            .position(|member| member.number == selected.number)
        else {
            return;
        };
        let adjacent = if up {
            position.checked_add(1)
        } else {
            position.checked_sub(1)
        };
        let Some(member) = adjacent
            .and_then(|index| stack.pull_requests.get(index))
            .cloned()
        else {
            return;
        };
        let expanded = self.detail_state.stack_expanded;
        self.select_stack_pull_request(member, cx);
        self.detail_state.stack_expanded = expanded;
    }

    pub(super) fn run_stack_merge(&mut self, cx: &mut Context<Self>) {
        if self.action_runtime.pull_request_action_running() {
            return;
        }
        let Some(stack) = self.detail_state.stack.as_ref() else {
            return;
        };
        if self.detail_state.stack_load.is_loading() {
            return;
        }
        if let Some(blocker) = stack.merge_blocker() {
            self.action_runtime
                .set_pull_request_action_error(blocker.clone());
            self.status = blocker;
            cx.notify();
            return;
        }
        let Some(top) = stack.top_open_pull_request() else {
            return;
        };
        let repository = top.repo.clone();
        let number = top.number;
        let head_sha = top.head_sha.clone();
        let stack_number = stack.number;
        let action = if stack.uses_merge_queue() {
            StackMergeAction::Enqueue
        } else {
            StackMergeAction::Merge(harbor_domain::MergeMethod::Squash)
        };
        let github_api = self.github_api_for_request();
        let auth_generation = self.auth_generation;
        self.action_runtime
            .start_pull_request_action(PullRequestActionKind::Merge);
        self.status = match action {
            StackMergeAction::Enqueue => format!("Enqueueing stack #{stack_number}…"),
            StackMergeAction::Merge(_) => format!("Merging stack #{stack_number}…"),
        };
        cx.notify();
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_spawn(async move {
                    github_api
                        .merge_pull_request_stack(&repository, number, &head_sha, action)
                        .await
                })
                .await;
            this.update_in_session_or_log(
                cx,
                auth_generation,
                "failed to update stack merge state",
                |view, cx| {
                    match result {
                        Ok(outcome) => {
                            view.action_runtime.finish_pull_request_action();
                            view.detail_state.stack_expanded = false;
                            view.reload_pull_request_inbox(cx);
                            view.status = match outcome {
                                StackMergeOutcome::Merged => {
                                    format!("Merged stack #{stack_number}")
                                }
                                StackMergeOutcome::Enqueued => {
                                    format!("Stack #{stack_number} entered the merge queue")
                                }
                            };
                        }
                        Err(error) => {
                            let message = format!("Stack #{stack_number}: {error}");
                            view.action_runtime
                                .finish_pull_request_action_failure(message.clone());
                            view.status = message;
                        }
                    }
                    view.refresh_selected_pull_request_metadata_only(cx);
                    cx.notify();
                },
            );
        })
        .detach();
    }
}
