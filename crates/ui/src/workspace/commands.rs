use gpui::{ClipboardItem, Context, ScrollStrategy, Window};
use harbor_domain::RepoId;
use harbor_github::{GhCliTransport, GitHubClient};

use crate::actions::*;
use crate::diff_reviews::diff_hunk_row_index_with_reviews;
use crate::panels::{
    merge_blocker, review_action_blocker, workflow_run_failed, workflow_run_label,
};
use crate::workspace::AppView;

impl AppView {
    pub(super) fn select_next_file(
        &mut self,
        _: &SelectNextFile,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.files.is_empty() {
            self.status = "No changed files to select".to_string();
            cx.notify();
            return;
        }

        self.select_file((self.active_file + 1) % self.files.len(), cx);
    }

    pub(super) fn select_previous_file(
        &mut self,
        _: &SelectPreviousFile,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.files.is_empty() {
            self.status = "No changed files to select".to_string();
            cx.notify();
            return;
        }

        let previous = if self.active_file == 0 {
            self.files.len() - 1
        } else {
            self.active_file - 1
        };
        self.select_file(previous, cx);
    }

    pub(super) fn select_next_hunk(
        &mut self,
        _: &SelectNextHunk,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(hunk_count) = self.active_diff().map(|diff| diff.hunks.len()) else {
            self.status = "No parsed diff hunks for active file".to_string();
            cx.notify();
            return;
        };

        self.active_hunk = (self.active_hunk + 1) % hunk_count;
        if let (Some(diff), Some(file)) = (self.active_diff(), self.active_file())
            && let Some(row_index) =
                diff_hunk_row_index_with_reviews(diff, self.active_hunk, file, &self.review_threads)
        {
            self.diff_list_scroll
                .scroll_to_item(row_index, ScrollStrategy::Center);
        }
        self.status = format!("Selected hunk {}", self.active_hunk + 1);
        cx.notify();
    }

    pub(super) fn select_previous_hunk(
        &mut self,
        _: &SelectPreviousHunk,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(hunk_count) = self.active_diff().map(|diff| diff.hunks.len()) else {
            self.status = "No parsed diff hunks for active file".to_string();
            cx.notify();
            return;
        };

        self.active_hunk = if self.active_hunk == 0 {
            hunk_count - 1
        } else {
            self.active_hunk - 1
        };
        if let (Some(diff), Some(file)) = (self.active_diff(), self.active_file())
            && let Some(row_index) =
                diff_hunk_row_index_with_reviews(diff, self.active_hunk, file, &self.review_threads)
        {
            self.diff_list_scroll
                .scroll_to_item(row_index, ScrollStrategy::Center);
        }
        self.status = format!("Selected hunk {}", self.active_hunk + 1);
        cx.notify();
    }

    pub(super) fn copy_active_file_path(
        &mut self,
        _: &CopyActiveFilePath,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(path) = self.active_file().map(|file| file.path.clone()) else {
            self.status = "No active file path to copy".to_string();
            cx.notify();
            return;
        };

        cx.write_to_clipboard(ClipboardItem::new_string(path.clone()));
        self.status = format!("Copied {path}");
        cx.notify();
    }

    pub(super) fn open_active_file_on_github(
        &mut self,
        _: &OpenActiveFileOnGitHub,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(pr_url) = self.selected_pull_request().map(|pr| pr.url.clone()) else {
            self.status = "No pull request selected".to_string();
            cx.notify();
            return;
        };
        let Some(path) = self.active_file().map(|file| file.path.clone()) else {
            self.status = "No active file to open".to_string();
            cx.notify();
            return;
        };

        cx.open_url(&format!("{pr_url}/files"));
        self.status = format!("Opened GitHub files view for {path}");
        cx.notify();
    }

    pub(super) fn select_next(
        &mut self,
        _: &SelectNextPullRequest,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.pull_requests.is_empty() {
            let next = (self.selected_pr + 1) % self.pull_requests.len();
            self.select_pull_request(next, cx);
        } else {
            self.status = "No pull requests to select".to_string();
            cx.notify();
        }
    }

    pub(super) fn select_previous(
        &mut self,
        _: &SelectPreviousPullRequest,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.pull_requests.is_empty() {
            let previous = if self.selected_pr == 0 {
                self.pull_requests.len() - 1
            } else {
                self.selected_pr - 1
            };
            self.select_pull_request(previous, cx);
        } else {
            self.status = "No pull requests to select".to_string();
            cx.notify();
        }
    }

    pub(super) fn open_selected(
        &mut self,
        _: &OpenSelectedPullRequest,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.status = format!("Opened {} in the local shell", self.selected_pr_label());
        cx.notify();
    }

    pub(super) fn cycle_panel_tab(
        &mut self,
        _: &CyclePanelTab,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.active_tab = self.active_tab.next();
        self.status = format!("Switched to {} panel", self.active_tab.label());
        cx.notify();
    }

    pub(super) fn toggle_command_palette(
        &mut self,
        _: &ToggleCommandPalette,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.command_palette_open = !self.command_palette_open;
        if self.command_palette_open {
            self.repository_switcher_open = false;
            self.pull_request_switcher_open = false;
        }
        self.status = if self.command_palette_open {
            "Command palette opened".to_string()
        } else {
            "Command palette closed".to_string()
        };
        cx.notify();
    }

    pub(super) fn toggle_repository_switcher(
        &mut self,
        _: &ToggleRepositorySwitcher,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.repository_switcher_open = !self.repository_switcher_open;
        if self.repository_switcher_open {
            self.command_palette_open = false;
            self.pull_request_switcher_open = false;
            self.repository_search_input.update(cx, |input, cx| {
                input.set_value("", window, cx);
                input.focus(window, cx);
            });
        }
        self.status = if self.repository_switcher_open {
            "Repository switcher opened".to_string()
        } else {
            "Repository switcher closed".to_string()
        };
        cx.notify();
    }

    pub(super) fn close_panel(&mut self, _: &ClosePanel, _: &mut Window, cx: &mut Context<Self>) {
        self.command_palette_open = false;
        self.repository_switcher_open = false;
        self.pull_request_switcher_open = false;
        self.status = "Closed transient UI".to_string();
        cx.notify();
    }

    pub(crate) fn select_repository_from_switcher(
        &mut self,
        repository: RepoId,
        cx: &mut Context<Self>,
    ) {
        let selected_repository = repository.full_name();
        if self
            .selected_pull_request()
            .is_some_and(|pull_request| pull_request.repo == repository)
        {
            self.status = format!("Selected repository {selected_repository}");
            cx.notify();
            return;
        }

        if let Some(index) = self
            .pull_requests
            .iter()
            .position(|pull_request| pull_request.repo == repository)
        {
            self.select_pull_request(index, cx);
            return;
        }

        self.load_pull_requests(repository, cx);
    }

    pub(super) fn set_placeholder_status(&mut self, label: &str, cx: &mut Context<Self>) {
        self.status = format!(
            "{label} is wired as a command placeholder for {}",
            self.selected_pr_label()
        );
        cx.notify();
    }

    pub(super) fn workflow_action_request(
        &self,
        action: WorkflowAction,
    ) -> std::result::Result<WorkflowActionRequest, String> {
        let Some(repo) = self.configured_repo.clone() else {
            return Err(
                "Workflow actions require a selected repository and GitHub CLI auth".into(),
            );
        };

        match action {
            WorkflowAction::DispatchBuild => {
                let Some(pr) = self.selected_pull_request() else {
                    return Err("Select a pull request before dispatching a workflow".into());
                };
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
                        view.load_selected_pull_request(cx);
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
        let Some(repo) = self.configured_repo.clone() else {
            return Err(
                "Pull request actions require a selected repository and GitHub CLI auth".into(),
            );
        };
        let Some(pr) = self.selected_pull_request() else {
            return Err("Select a pull request before running a pull request action".into());
        };

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
        cx: &mut Context<Self>,
    ) {
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
                        match &request {
                            PullRequestActionRequest::Merge { .. } => {
                                if let Some(repo) = view.configured_repo.clone() {
                                    view.load_pull_requests(repo, cx);
                                }
                            }
                            PullRequestActionRequest::Approve { .. }
                            | PullRequestActionRequest::RequestChanges { .. } => {
                                view.load_selected_pull_request(cx);
                            }
                        }
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

    pub(super) fn refresh_selected(
        &mut self,
        _: &RefreshSelectedPullRequest,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.configured_repo.is_some() && self.selected_pull_request_number().is_some() {
            self.load_selected_pull_request(cx);
        } else if let Some(repo) = self.configured_repo.clone() {
            self.load_pull_requests(repo, cx);
        } else {
            self.set_placeholder_status("Refresh", cx);
        }
    }

    pub(super) fn checkout_pr(
        &mut self,
        _: &CheckoutPullRequest,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.set_placeholder_status("Checkout", cx);
    }

    pub(super) fn open_in_browser(
        &mut self,
        _: &OpenPullRequestInBrowser,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(pr) = self.selected_pull_request() else {
            self.status = "No pull request selected".to_string();
            cx.notify();
            return;
        };

        let url = pr.url.clone();
        let number = pr.number;
        cx.open_url(&url);
        self.status = format!("Opened PR #{number} in browser");
        cx.notify();
    }

    pub(super) fn approve_pr(
        &mut self,
        _: &ApprovePullRequest,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.run_pull_request_action(PullRequestAction::Approve, cx);
    }

    pub(super) fn request_changes(
        &mut self,
        _: &RequestChanges,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.run_pull_request_action(PullRequestAction::RequestChanges, cx);
    }

    pub(super) fn merge_pr(
        &mut self,
        _: &MergePullRequest,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.run_pull_request_action(PullRequestAction::Merge, cx);
    }

    pub(super) fn open_logs(&mut self, _: &OpenLogs, _: &mut Window, cx: &mut Context<Self>) {
        self.active_tab = PanelTab::Logs;
        if self.configured_repo.is_some() {
            self.load_selected_workflow_logs(cx);
        } else {
            self.set_placeholder_status("Open logs", cx);
        }
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

    pub(super) fn filter_current_list(
        &mut self,
        _: &FilterCurrentList,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.set_placeholder_status("Filter", cx);
    }
}
