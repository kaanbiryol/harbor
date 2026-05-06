use gpui::{Context, Window};
use harbor_github::{GhCliTransport, GitHubClient, SubmitPullRequestReviewEvent};

use crate::{
    actions::{
        ApprovePullRequest, DEFAULT_REQUEST_CHANGES_BODY, MergePullRequest, PanelTab,
        PullRequestAction, PullRequestActionRequest, RequestChanges, RerunFailedJobs, TriggerBuild,
        WorkflowAction, WorkflowActionRequest,
    },
    panels::{merge_blocker, review_action_blocker, workflow_run_failed, workflow_run_label},
    workspace::AppView,
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
                    .workflow_runs
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
                    .workflow_runs
                    .iter()
                    .find(|run| workflow_run_failed(run))
                    .or_else(|| self.workflow_runs.first())
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

        if self.is_running_action {
            self.status = "A workflow action is already running".to_string();
            cx.notify();
            return;
        }

        let request = match self.workflow_action_request(action) {
            Ok(request) => request,
            Err(message) => {
                self.action_error = Some(message.clone());
                self.status = message;
                cx.notify();
                return;
            }
        };

        self.is_running_action = true;
        self.action_error = None;
        self.status = request.start_status();
        cx.notify();

        cx.spawn(async move |this, cx| {
            let client = GitHubClient::new(GhCliTransport);
            let result = match &request {
                WorkflowActionRequest::DispatchBuild {
                    owner,
                    repo,
                    workflow_id,
                    git_ref,
                    ..
                } => {
                    client
                        .dispatch_workflow(owner, repo, *workflow_id, git_ref)
                        .await
                }
                WorkflowActionRequest::RerunFailedJobs {
                    owner,
                    repo,
                    run_id,
                    ..
                } => client.rerun_failed_jobs(owner, repo, *run_id).await,
            };

            _ = this.update(cx, move |view, cx| {
                view.is_running_action = false;

                match result {
                    Ok(()) => {
                        view.action_error = None;
                        view.refresh_selected_pull_request(cx);
                        view.status = request.success_status();
                    }
                    Err(error) => {
                        let message = format!("Failed to {}: {error}", request.failure_label());
                        view.action_error = Some(message.clone());
                        view.status = message;
                    }
                }

                cx.notify();
            });
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
            PullRequestAction::Merge => {
                if let Some(blocker) = merge_blocker(pr) {
                    return Err(blocker);
                }

                Ok(PullRequestActionRequest::Merge {
                    owner: repo.owner,
                    repo: repo.name,
                    number: pr.number,
                    head_sha: pr.head_sha.clone(),
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
        if self.pending_review.is_some() {
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
                PullRequestAction::Merge => {}
            }
        }

        if self.is_running_pr_action {
            self.status = "A pull request action is already running".to_string();
            cx.notify();
            return;
        }

        let request = match self.pull_request_action_request(action) {
            Ok(request) => request,
            Err(message) => {
                self.pr_action_error = Some(message.clone());
                self.status = message;
                cx.notify();
                return;
            }
        };

        self.is_running_pr_action = true;
        self.pr_action_error = None;
        self.status = request.start_status();
        cx.notify();

        cx.spawn(async move |this, cx| {
            let client = GitHubClient::new(GhCliTransport);
            let result = match &request {
                PullRequestActionRequest::Approve {
                    owner,
                    repo,
                    number,
                } => client.approve_pull_request(owner, repo, *number).await,
                PullRequestActionRequest::RequestChanges {
                    owner,
                    repo,
                    number,
                    body,
                } => {
                    client
                        .request_pull_request_changes(owner, repo, *number, body)
                        .await
                }
                PullRequestActionRequest::Merge {
                    owner,
                    repo,
                    number,
                    head_sha,
                } => {
                    client
                        .merge_pull_request(owner, repo, *number, head_sha)
                        .await
                }
            };

            _ = this.update(cx, move |view, cx| {
                view.is_running_pr_action = false;

                match result {
                    Ok(()) => {
                        let status = request.success_status();
                        view.pr_action_error = None;
                        view.reload_pull_request_inbox(cx);
                        view.status = status;
                    }
                    Err(error) => {
                        let message = format!("Failed to {}: {error}", request.failure_label());
                        view.pr_action_error = Some(message.clone());
                        view.status = message;
                    }
                }

                cx.notify();
            });
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
        self.run_pull_request_action(PullRequestAction::Merge, window, cx);
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
