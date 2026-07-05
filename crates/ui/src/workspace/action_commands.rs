use gpui::{Context, Window};
use harbor_domain::MergeMethod;
use harbor_github::SubmitPullRequestReviewEvent;

use crate::{
    actions::{
        ApprovePullRequest, DEFAULT_REQUEST_CHANGES_BODY, MergePullRequest,
        MergePullRequestWithMergeCommit, PanelTab, PullRequestAction, PullRequestActionRequest,
        RebasePullRequest, RequestChanges, RerunFailedJobs, TriggerBuild, WorkflowAction,
        WorkflowActionRequest,
    },
    panels::{merge_blocker, review_action_blocker, workflow_run_failed, workflow_run_label},
    workspace::{AppView, async_updates::AppViewAsyncUpdateExt},
};

impl AppView {
    pub(super) fn workflow_action_request(
        &self,
        action: WorkflowAction,
    ) -> std::result::Result<WorkflowActionRequest, String> {
        let Some(pr) = self.selected_pull_request() else {
            return Err("Select a pull request before running a workflow action".into());
        };
        let repo = pr.repo.clone();

        match action {
            WorkflowAction::DispatchBuild => {
                let Some(run) = self
                    .detail_state
                    .workflow_runs()
                    .iter()
                    .find(|run| run.workflow_id.is_some())
                else {
                    return Err(
                        "No workflow id is available for the selected pull request head".into(),
                    );
                };
                let Some(workflow_id) = run.workflow_id else {
                    return Err(
                        "No workflow id is available for the selected pull request head".into(),
                    );
                };

                Ok(WorkflowActionRequest::DispatchBuild {
                    owner: repo.owner,
                    repo: repo.name,
                    workflow_id,
                    git_ref: pr.head_ref.clone(),
                    workflow_name: workflow_run_label(run),
                })
            }
            WorkflowAction::RerunFailedJobs => {
                let Some(run) = self
                    .detail_state
                    .workflow_runs()
                    .iter()
                    .find(|run| workflow_run_failed(run))
                    .or_else(|| self.detail_state.workflow_runs().first())
                else {
                    return Err(
                        "No workflow run is available for the selected pull request head".into(),
                    );
                };

                Ok(WorkflowActionRequest::RerunFailedJobs {
                    owner: repo.owner,
                    repo: repo.name,
                    run_id: run.id,
                    workflow_name: workflow_run_label(run),
                })
            }
        }
    }

    pub(crate) fn run_workflow_action(&mut self, action: WorkflowAction, cx: &mut Context<Self>) {
        self.active_tab = PanelTab::Actions;

        if self.action_runtime.workflow_action_running() {
            self.status = "A workflow action is already running".to_string();
            cx.notify();
            return;
        }

        let request = match self.workflow_action_request(action) {
            Ok(request) => request,
            Err(message) => {
                self.action_runtime
                    .set_workflow_action_error(message.clone());
                self.status = message;
                cx.notify();
                return;
            }
        };

        self.action_runtime.start_workflow_action();
        self.status = request.start_status();
        cx.notify();
        let github_api = self.github_api.clone();

        cx.spawn(async move |this, cx| {
            let result = match &request {
                WorkflowActionRequest::DispatchBuild {
                    owner,
                    repo,
                    workflow_id,
                    git_ref,
                    ..
                } => {
                    github_api
                        .dispatch_workflow(owner, repo, *workflow_id, git_ref)
                        .await
                }
                WorkflowActionRequest::RerunFailedJobs {
                    owner,
                    repo,
                    run_id,
                    ..
                } => github_api.rerun_failed_jobs(owner, repo, *run_id).await,
            };

            this.update_or_log(
                cx,
                "failed to update workflow action state",
                move |view, cx| {
                    match result {
                        Ok(()) => {
                            view.action_runtime.finish_workflow_action_success();
                            view.refresh_selected_pull_request(cx);
                            view.status = request.success_status();
                        }
                        Err(error) => {
                            let message = format!("Failed to {}: {error}", request.failure_label());
                            view.action_runtime
                                .finish_workflow_action_failure(message.clone());
                            view.status = message;
                        }
                    }

                    cx.notify();
                },
            );
        })
        .detach();
    }

    pub(super) fn pull_request_action_request(
        &self,
        action: PullRequestAction,
    ) -> std::result::Result<PullRequestActionRequest, String> {
        let Some(pr) = self.selected_pull_request() else {
            return Err("Select a pull request before running a pull request action".into());
        };
        let repo = pr.repo.clone();

        match action {
            PullRequestAction::Approve => {
                if let Some(blocker) = review_action_blocker(pr) {
                    return Err(blocker);
                }

                Ok(PullRequestActionRequest::Approve {
                    owner: repo.owner,
                    repo: repo.name,
                    number: pr.number,
                })
            }
            PullRequestAction::RequestChanges => {
                if let Some(blocker) = review_action_blocker(pr) {
                    return Err(blocker);
                }

                Ok(PullRequestActionRequest::RequestChanges {
                    owner: repo.owner,
                    repo: repo.name,
                    number: pr.number,
                    body: DEFAULT_REQUEST_CHANGES_BODY.to_string(),
                })
            }
            PullRequestAction::Merge(method) => {
                if let Some(blocker) = merge_blocker(pr) {
                    return Err(blocker);
                }

                Ok(PullRequestActionRequest::Merge {
                    owner: repo.owner,
                    repo: repo.name,
                    number: pr.number,
                    head_sha: pr.head_sha.clone(),
                    method,
                })
            }
        }
    }

    pub(crate) fn run_pull_request_action(
        &mut self,
        action: PullRequestAction,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.review_state.has_pending_review() {
            match action {
                PullRequestAction::Approve => {
                    self.submit_pending_pull_request_review(
                        SubmitPullRequestReviewEvent::Approve,
                        window,
                        cx,
                    );
                    return;
                }
                PullRequestAction::RequestChanges => {
                    self.submit_pending_pull_request_review(
                        SubmitPullRequestReviewEvent::RequestChanges,
                        window,
                        cx,
                    );
                    return;
                }
                PullRequestAction::Merge(_) => {}
            }
        }

        if self.action_runtime.pull_request_action_running() {
            self.status = "A pull request action is already running".to_string();
            cx.notify();
            return;
        }

        let request = match self.pull_request_action_request(action) {
            Ok(request) => request,
            Err(message) => {
                self.action_runtime
                    .set_pull_request_action_error(message.clone());
                self.status = message;
                cx.notify();
                return;
            }
        };

        self.action_runtime.start_pull_request_action();
        self.status = request.start_status();
        cx.notify();
        let github_api = self.github_api.clone();

        cx.spawn(async move |this, cx| {
            let result = match &request {
                PullRequestActionRequest::Approve {
                    owner,
                    repo,
                    number,
                } => github_api.approve_pull_request(owner, repo, *number).await,
                PullRequestActionRequest::RequestChanges {
                    owner,
                    repo,
                    number,
                    body,
                } => {
                    github_api
                        .request_pull_request_changes(owner, repo, *number, body)
                        .await
                }
                PullRequestActionRequest::Merge {
                    owner,
                    repo,
                    number,
                    head_sha,
                    method,
                } => {
                    github_api
                        .merge_pull_request(owner, repo, *number, head_sha, *method)
                        .await
                }
            };

            this.update_or_log(
                cx,
                "failed to update pull request action state",
                move |view, cx| {
                    match result {
                        Ok(()) => {
                            let status = request.success_status();
                            view.action_runtime.finish_pull_request_action();
                            view.reload_pull_request_inbox(cx);
                            view.status = status;
                        }
                        Err(error) => {
                            let message = format!("Failed to {}: {error}", request.failure_label());
                            view.action_runtime
                                .finish_pull_request_action_failure(message.clone());
                            view.status = message;
                        }
                    }

                    cx.notify();
                },
            );
        })
        .detach();
    }

    pub(super) fn approve_pr(
        &mut self,
        _: &ApprovePullRequest,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.run_pull_request_action(PullRequestAction::Approve, window, cx);
    }

    pub(super) fn request_changes(
        &mut self,
        _: &RequestChanges,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.run_pull_request_action(PullRequestAction::RequestChanges, window, cx);
    }

    pub(super) fn merge_pr(
        &mut self,
        _: &MergePullRequest,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.run_pull_request_action(PullRequestAction::Merge(MergeMethod::Squash), window, cx);
    }

    pub(super) fn merge_pr_with_merge_commit(
        &mut self,
        _: &MergePullRequestWithMergeCommit,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.run_pull_request_action(PullRequestAction::Merge(MergeMethod::Merge), window, cx);
    }

    pub(super) fn rebase_pr(
        &mut self,
        _: &RebasePullRequest,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.run_pull_request_action(PullRequestAction::Merge(MergeMethod::Rebase), window, cx);
    }

    pub(super) fn trigger_build(
        &mut self,
        _: &TriggerBuild,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.run_workflow_action(WorkflowAction::DispatchBuild, cx);
    }

    pub(super) fn rerun_failed(
        &mut self,
        _: &RerunFailedJobs,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.run_workflow_action(WorkflowAction::RerunFailedJobs, cx);
    }
}
