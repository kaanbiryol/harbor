use std::path::{Path, PathBuf};

use gpui::{AppContext, ClipboardItem, Context, PathPromptOptions, ScrollStrategy, Window};
use harbor_domain::{DiffFile, FileStatus, PullRequest, RepoId};
use harbor_git::{ExternalApp, ExternalAppKind, OpenTarget};
use harbor_github::{GhCliTransport, GitHubClient, SubmitPullRequestReviewEvent};

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
        let visible_files = self.visible_file_indices(cx);
        if visible_files.is_empty() {
            self.status = "No changed files to select".to_string();
            cx.notify();
            return;
        }

        let current_position = visible_files
            .iter()
            .position(|file_index| *file_index == self.active_file)
            .unwrap_or(visible_files.len().saturating_sub(1));
        let next_position = (current_position + 1) % visible_files.len();
        self.select_file(visible_files[next_position], cx);
    }

    pub(super) fn select_previous_file(
        &mut self,
        _: &SelectPreviousFile,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let visible_files = self.visible_file_indices(cx);
        if visible_files.is_empty() {
            self.status = "No changed files to select".to_string();
            cx.notify();
            return;
        }

        let current_position = visible_files
            .iter()
            .position(|file_index| *file_index == self.active_file)
            .unwrap_or(0);
        let previous_position = if current_position == 0 {
            visible_files.len() - 1
        } else {
            current_position - 1
        };
        self.select_file(visible_files[previous_position], cx);
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
        let Some(pr) = self.selected_pull_request() else {
            self.status = "No pull request selected".to_string();
            cx.notify();
            return;
        };

        let Some(file) = self.active_file() else {
            cx.open_url(&format!("{}/files", pr.url));
            self.status = format!("Opened GitHub files view for PR #{}", pr.number);
            cx.notify();
            return;
        };

        let url = github_file_url(pr, file).unwrap_or_else(|| format!("{}/files", pr.url));
        let path = file.path.clone();
        cx.open_url(&url);
        self.status = if file.status == FileStatus::Removed {
            format!("Opened GitHub files view because {path} was removed")
        } else {
            format!("Opened {path} on GitHub")
        };
        cx.notify();
    }

    pub(super) fn choose_local_checkout(
        &mut self,
        _: &ChooseLocalCheckout,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(repository) = self.current_repository().cloned() else {
            self.status = "Select a repository before choosing a local checkout".to_string();
            cx.notify();
            return;
        };

        let selected_path = cx.prompt_for_paths(PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: Some(format!("Select local checkout for {}", repository.full_name()).into()),
        });
        let view = cx.entity().clone();

        cx.spawn_in(window, async move |_, window| {
            let Ok(Ok(Some(paths))) = selected_path.await else {
                return;
            };
            let Some(path) = paths.into_iter().next() else {
                return;
            };

            if let Err(error) = window.update(|_, cx| {
                view.update(cx, |view, cx| {
                    view.validate_and_store_local_checkout(repository, path, cx);
                })
            }) {
                eprintln!("failed to start local checkout validation: {error}");
            }
        })
        .detach();
    }

    pub(super) fn open_with_vs_code(
        &mut self,
        _: &OpenWithVsCode,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.open_with_app(ExternalApp::VsCode, cx);
    }

    pub(super) fn open_with_cursor(
        &mut self,
        _: &OpenWithCursor,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.open_with_app(ExternalApp::Cursor, cx);
    }

    pub(super) fn open_with_zed(
        &mut self,
        _: &OpenWithZed,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.open_with_app(ExternalApp::Zed, cx);
    }

    pub(super) fn open_with_finder(
        &mut self,
        _: &OpenWithFinder,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.open_with_app(ExternalApp::Finder, cx);
    }

    pub(super) fn open_with_terminal(
        &mut self,
        _: &OpenWithTerminal,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.open_with_app(ExternalApp::Terminal, cx);
    }

    pub(super) fn open_with_ghostty(
        &mut self,
        _: &OpenWithGhostty,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.open_with_app(ExternalApp::Ghostty, cx);
    }

    pub(super) fn open_with_warp(
        &mut self,
        _: &OpenWithWarp,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.open_with_app(ExternalApp::Warp, cx);
    }

    pub(super) fn open_with_xcode(
        &mut self,
        _: &OpenWithXcode,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.open_with_app(ExternalApp::Xcode, cx);
    }

    fn validate_and_store_local_checkout(
        &mut self,
        repository: RepoId,
        path: PathBuf,
        cx: &mut Context<Self>,
    ) {
        let store = self.repository_store.clone();
        let repository_for_task = repository.clone();
        let owner = repository.owner.clone();
        let repo_name = repository.name.clone();
        let path_for_status = path.display().to_string();

        self.status = format!(
            "Validating local checkout for {} at {path_for_status}",
            repository.full_name()
        );
        cx.notify();

        let task = cx.background_spawn(async move {
            let local_repository = harbor_git::validate_repository_path(&path, &owner, &repo_name)
                .map_err(|error| error.to_string())?;

            if let Some(store) = store {
                store
                    .set_repository_local_path(&repository_for_task, &local_repository.repo_path)
                    .await
                    .map_err(|error| error.to_string())?;
            }

            Ok::<PathBuf, String>(local_repository.repo_path)
        });

        self.local_task = Some(cx.spawn(async move |this, cx| {
            let result = task.await;

            if let Err(error) = this.update(cx, move |view, cx| {
                match result {
                    Ok(repo_path) => {
                        view.set_repository_local_path(repository.clone(), repo_path.clone());
                        view.repository_error = None;
                        view.status = format!(
                            "Saved local checkout for {} at {}",
                            repository.full_name(),
                            repo_path.display()
                        );
                        view.refresh_owned_file_filters(cx);
                    }
                    Err(error) => {
                        view.repository_error = Some(error.clone());
                        view.status = format!("Failed to save local checkout: {error}");
                    }
                }

                view.local_task = None;
                cx.notify();
            }) {
                eprintln!("failed to update local checkout state: {error}");
            }
        }));
    }

    fn open_with_app(&mut self, app: ExternalApp, cx: &mut Context<Self>) {
        let Some(pr) = self.selected_pull_request().cloned() else {
            self.status = "Select a pull request before using Open With".to_string();
            cx.notify();
            return;
        };

        let Some(repo_path) = self.repository_local_paths.get(&pr.repo).cloned() else {
            self.status = format!(
                "Choose a local checkout for {} before opening with {}",
                pr.repo.full_name(),
                app.label()
            );
            cx.notify();
            return;
        };

        if !app.is_available() {
            self.status = format!("{} is not installed", app.label());
            cx.notify();
            return;
        }

        let active_file = self.active_file().cloned();
        let app_label = app.label();
        self.status = format!("Preparing PR #{} worktree for {app_label}", pr.number);
        cx.notify();

        let task = cx.background_spawn(async move {
            let worktree_path = harbor_git::create_or_update_pr_worktree(
                &repo_path,
                &pr.repo.owner,
                &pr.repo.name,
                pr.number,
            )
            .map_err(|error| error.to_string())?;
            let (target, target_status) =
                open_target_for_app(app, &worktree_path, active_file.as_ref());

            harbor_git::open_external_app(app, target).map_err(|error| error.to_string())?;

            Ok::<String, String>(open_with_status(
                app,
                &pr,
                active_file.as_ref(),
                target_status,
            ))
        });

        self.local_task = Some(cx.spawn(async move |this, cx| {
            let result = task.await;

            if let Err(error) = this.update(cx, move |view, cx| {
                view.status = match result {
                    Ok(status) => status,
                    Err(error) => format!("Failed to open with {app_label}: {error}"),
                };
                view.local_task = None;
                cx.notify();
            }) {
                eprintln!("failed to update open-with state: {error}");
            }
        }));
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
        self.select_panel_tab(self.active_tab.next(), cx);
    }

    pub(crate) fn select_panel_tab(&mut self, tab: PanelTab, cx: &mut Context<Self>) {
        if self.active_tab == tab {
            return;
        }

        self.active_tab = tab;
        self.status = format!("Switched to {} panel", tab.label());
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
            self.file_filter_popover_open = false;
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
            self.file_filter_popover_open = false;
            self.repository_search_input.update(cx, |input, cx| {
                input.set_value("", window, cx);
                input.focus(window, cx);
            });
            self.reset_repository_switcher_selection(cx);
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
        self.file_filter_popover_open = false;
        self.status = "Closed transient UI".to_string();
        cx.notify();
    }

    pub(crate) fn select_repository_from_switcher(
        &mut self,
        repository: RepoId,
        cx: &mut Context<Self>,
    ) {
        let selected_repository = repository.full_name();
        if self.configured_repo.as_ref() == Some(&repository) {
            self.status = format!("Selected repository {selected_repository}");
            cx.notify();
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

    pub(super) fn refresh_selected(
        &mut self,
        _: &RefreshSelectedPullRequest,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.selected_pull_request_number().is_some() {
            self.load_selected_pull_request(cx);
        } else if let Some(repo) = self.configured_repo.clone() {
            self.load_pull_requests(repo, cx);
        } else {
            self.status =
                "Select a repository from the header before refreshing pull requests".to_string();
            cx.notify();
        }
    }

    pub(super) fn checkout_pr(
        &mut self,
        _: &CheckoutPullRequest,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(pr) = self.selected_pull_request().cloned() else {
            self.status = "Select a pull request before checkout".to_string();
            cx.notify();
            return;
        };

        let Some(repo_path) = self.repository_local_paths.get(&pr.repo).cloned() else {
            self.status = format!(
                "Choose a local checkout for {} before checkout",
                pr.repo.full_name()
            );
            cx.notify();
            return;
        };

        self.status = format!("Preparing PR #{} worktree", pr.number);
        cx.notify();

        let task = cx.background_spawn(async move {
            harbor_git::create_or_update_pr_worktree(
                &repo_path,
                &pr.repo.owner,
                &pr.repo.name,
                pr.number,
            )
            .map(|path| format!("Prepared PR #{} worktree at {}", pr.number, path.display()))
            .map_err(|error| error.to_string())
        });

        self.local_task = Some(cx.spawn(async move |this, cx| {
            let result = task.await;

            if let Err(error) = this.update(cx, move |view, cx| {
                view.status = match result {
                    Ok(status) => status,
                    Err(error) => format!("Failed to prepare PR worktree: {error}"),
                };
                view.local_task = None;
                cx.notify();
            }) {
                eprintln!("failed to update checkout state: {error}");
            }
        }));
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

    pub(super) fn open_logs(&mut self, _: &OpenLogs, _: &mut Window, cx: &mut Context<Self>) {
        self.active_tab = PanelTab::Logs;
        if self.selected_pull_request().is_some() {
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
        self.file_filter_popover_open = !self.file_filter_popover_open;
        self.command_palette_open = false;
        self.repository_switcher_open = false;
        self.pull_request_switcher_open = false;
        self.status = if self.file_filter_popover_open {
            "Opened changed-file filters".to_string()
        } else {
            "Closed changed-file filters".to_string()
        };
        cx.notify();
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum OpenTargetStatus {
    Root,
    ActiveFile,
    RemovedFile,
    MissingFile,
}

pub(crate) fn open_target_for_app(
    app: ExternalApp,
    worktree_path: &Path,
    active_file: Option<&DiffFile>,
) -> (OpenTarget, OpenTargetStatus) {
    if app.kind() == ExternalAppKind::Terminal {
        return (
            OpenTarget::Directory(worktree_path.to_path_buf()),
            OpenTargetStatus::Root,
        );
    }

    let Some(file) = active_file else {
        return (
            OpenTarget::Directory(worktree_path.to_path_buf()),
            OpenTargetStatus::Root,
        );
    };

    if file.status == FileStatus::Removed {
        return (
            OpenTarget::Directory(worktree_path.to_path_buf()),
            OpenTargetStatus::RemovedFile,
        );
    }

    let file_path = worktree_path.join(&file.path);
    if !file_path.exists() {
        return (
            OpenTarget::Directory(worktree_path.to_path_buf()),
            OpenTargetStatus::MissingFile,
        );
    }

    if app.kind() == ExternalAppKind::Finder {
        (OpenTarget::Reveal(file_path), OpenTargetStatus::ActiveFile)
    } else {
        (OpenTarget::File(file_path), OpenTargetStatus::ActiveFile)
    }
}

fn open_with_status(
    app: ExternalApp,
    pr: &PullRequest,
    active_file: Option<&DiffFile>,
    target_status: OpenTargetStatus,
) -> String {
    match target_status {
        OpenTargetStatus::ActiveFile => {
            let path = active_file
                .map(|file| file.path.as_str())
                .unwrap_or("active file");
            format!("Opened {path} from PR #{} in {}", pr.number, app.label())
        }
        OpenTargetStatus::Root => {
            format!("Opened PR #{} worktree in {}", pr.number, app.label())
        }
        OpenTargetStatus::RemovedFile => {
            format!(
                "Opened PR #{} worktree in {}; selected file was removed",
                pr.number,
                app.label()
            )
        }
        OpenTargetStatus::MissingFile => {
            format!(
                "Opened PR #{} worktree in {}; active file was unavailable",
                pr.number,
                app.label()
            )
        }
    }
}

pub(crate) fn github_file_url(pr: &PullRequest, file: &DiffFile) -> Option<String> {
    if file.status == FileStatus::Removed || pr.head_sha.is_empty() || file.path.is_empty() {
        return None;
    }

    Some(format!(
        "https://github.com/{}/{}/blob/{}/{}",
        encode_path_component(&pr.repo.owner),
        encode_path_component(&pr.repo.name),
        encode_path_component(&pr.head_sha),
        encode_github_path(&file.path)
    ))
}

fn encode_github_path(path: &str) -> String {
    path.split('/')
        .map(encode_path_component)
        .collect::<Vec<_>>()
        .join("/")
}

fn encode_path_component(value: &str) -> String {
    let mut encoded = String::with_capacity(value.len());

    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'~') {
            encoded.push(byte as char);
        } else {
            encoded.push_str(&format!("%{byte:02X}"));
        }
    }

    encoded
}
