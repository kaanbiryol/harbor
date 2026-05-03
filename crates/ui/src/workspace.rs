use gpui::{
    App, ClipboardItem, Context, FocusHandle, Focusable, IntoElement, Render, ScrollStrategy,
    UniformListScrollHandle, Window, div, prelude::*, px, rgb, uniform_list,
};
use gpui_component::{Disableable, Sizable, button::Button};
use harbor_domain::{
    CheckRun, DiffFile, PullRequest, PullRequestReview, RepoId, ReviewThread, ReviewThreadState,
    WorkflowJob, WorkflowRun,
};
use harbor_github::{GhCliTransport, GitHubClient};
use harbor_logs::{LogChunk, parse_workflow_log};

use crate::actions::*;
use crate::diff::{ParsedDiff, parse_files};
use crate::fake_data::{
    configured_repo_from_env, fake_files, fake_pull_request_reviews, fake_pull_requests,
    fake_review_threads,
};
use crate::panels::{
    checks_summary_from_runs, diff_hunk_row_index, merge_blocker, render_actions_panel,
    render_changed_file_row, render_checks_panel, render_diff_panel, render_logs_panel,
    render_merge_state, render_pull_request_row, render_review_decision, render_review_panel,
    review_action_blocker, workflow_run_failed, workflow_run_label,
};

pub struct AppView {
    pub(crate) focus_handle: FocusHandle,
    pub(crate) pull_requests: Vec<PullRequest>,
    pub(crate) files: Vec<DiffFile>,
    pub(crate) diffs: Vec<Option<ParsedDiff>>,
    pub(crate) check_runs: Vec<CheckRun>,
    pub(crate) workflow_runs: Vec<WorkflowRun>,
    pub(crate) workflow_jobs: Vec<WorkflowJob>,
    pub(crate) pull_request_reviews: Vec<PullRequestReview>,
    pub(crate) review_threads: Vec<ReviewThread>,
    pub(crate) log_chunk: Option<LogChunk>,
    pub(crate) pr_list_scroll: UniformListScrollHandle,
    pub(crate) file_list_scroll: UniformListScrollHandle,
    pub(crate) diff_list_scroll: UniformListScrollHandle,
    pub(crate) review_list_scroll: UniformListScrollHandle,
    pub(crate) log_list_scroll: UniformListScrollHandle,
    pub(crate) selected_pr: usize,
    pub(crate) active_file: usize,
    pub(crate) active_hunk: usize,
    pub(crate) active_tab: PanelTab,
    pub(crate) command_palette_open: bool,
    pub(crate) configured_repo: Option<RepoId>,
    pub(crate) is_loading_prs: bool,
    pub(crate) is_loading_details: bool,
    pub(crate) is_loading_files: bool,
    pub(crate) is_loading_checks: bool,
    pub(crate) is_loading_workflows: bool,
    pub(crate) is_loading_reviews: bool,
    pub(crate) is_loading_logs: bool,
    pub(crate) is_running_action: bool,
    pub(crate) is_running_pr_action: bool,
    pub(crate) load_error: Option<String>,
    pub(crate) details_error: Option<String>,
    pub(crate) files_error: Option<String>,
    pub(crate) checks_error: Option<String>,
    pub(crate) workflows_error: Option<String>,
    pub(crate) reviews_error: Option<String>,
    pub(crate) logs_error: Option<String>,
    pub(crate) action_error: Option<String>,
    pub(crate) pr_action_error: Option<String>,
    pub(crate) did_focus: bool,
    pub(crate) status: String,
}

impl AppView {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let configured_repo = configured_repo_from_env();
        let pull_requests = if configured_repo.is_some() {
            Vec::new()
        } else {
            fake_pull_requests()
        };
        let files = if configured_repo.is_some() {
            Vec::new()
        } else {
            fake_files()
        };
        let pull_request_reviews = if configured_repo.is_some() {
            Vec::new()
        } else {
            fake_pull_request_reviews()
        };
        let review_threads = if configured_repo.is_some() {
            Vec::new()
        } else {
            fake_review_threads()
        };
        let diffs = parse_files(&files);
        let status = configured_repo
            .as_ref()
            .map(|repo| format!("Loading open pull requests from {}", repo.full_name()))
            .unwrap_or_else(|| {
                "Using fake data. Set HARBOR_REPO=owner/repo to load GitHub PRs.".to_string()
            });

        let mut view = Self {
            focus_handle: cx.focus_handle(),
            pull_requests,
            files,
            diffs,
            check_runs: Vec::new(),
            workflow_runs: Vec::new(),
            workflow_jobs: Vec::new(),
            pull_request_reviews,
            review_threads,
            log_chunk: None,
            pr_list_scroll: UniformListScrollHandle::new(),
            file_list_scroll: UniformListScrollHandle::new(),
            diff_list_scroll: UniformListScrollHandle::new(),
            review_list_scroll: UniformListScrollHandle::new(),
            log_list_scroll: UniformListScrollHandle::new(),
            selected_pr: 0,
            active_file: 0,
            active_hunk: 0,
            active_tab: PanelTab::Diff,
            command_palette_open: false,
            configured_repo,
            is_loading_prs: false,
            is_loading_details: false,
            is_loading_files: false,
            is_loading_checks: false,
            is_loading_workflows: false,
            is_loading_reviews: false,
            is_loading_logs: false,
            is_running_action: false,
            is_running_pr_action: false,
            load_error: None,
            details_error: None,
            files_error: None,
            checks_error: None,
            workflows_error: None,
            reviews_error: None,
            logs_error: None,
            action_error: None,
            pr_action_error: None,
            did_focus: false,
            status,
        };

        if let Some(repo) = view.configured_repo.clone() {
            view.load_pull_requests(repo, cx);
        }

        view
    }

    fn selected_pull_request(&self) -> Option<&PullRequest> {
        self.pull_requests.get(self.selected_pr)
    }

    fn selected_pull_request_number(&self) -> Option<u64> {
        self.selected_pull_request().map(|pr| pr.number)
    }

    fn selected_pr_label(&self) -> String {
        self.selected_pull_request()
            .map(|pr| format!("PR #{}", pr.number))
            .unwrap_or_else(|| "no selected pull request".to_string())
    }

    fn active_file(&self) -> Option<&DiffFile> {
        self.files.get(self.active_file)
    }

    pub(crate) fn active_diff(&self) -> Option<&ParsedDiff> {
        self.diffs
            .get(self.active_file)
            .and_then(Option::as_ref)
            .filter(|diff| !diff.is_empty())
    }

    fn selected_workflow_run_for_logs(&self) -> Option<&WorkflowRun> {
        self.workflow_runs
            .iter()
            .find(|run| workflow_run_failed(run))
            .or_else(|| self.workflow_runs.first())
    }

    pub(crate) fn select_pull_request(&mut self, index: usize, cx: &mut Context<Self>) {
        if index >= self.pull_requests.len() {
            self.status = "No pull requests to select".to_string();
            cx.notify();
            return;
        }

        self.selected_pr = index;
        self.active_file = 0;
        self.active_hunk = 0;
        self.workflow_jobs.clear();
        self.log_chunk = None;
        self.pull_request_reviews.clear();
        self.review_threads.clear();
        self.reviews_error = None;
        self.logs_error = None;
        self.pr_action_error = None;
        self.pr_list_scroll
            .scroll_to_item(index, ScrollStrategy::Center);
        self.file_list_scroll.scroll_to_item(0, ScrollStrategy::Top);
        self.diff_list_scroll.scroll_to_item(0, ScrollStrategy::Top);
        self.review_list_scroll
            .scroll_to_item(0, ScrollStrategy::Top);
        self.status = format!("Selected {}", self.selected_pr_label());

        if self.configured_repo.is_some() {
            self.load_selected_pull_request(cx);
        } else {
            self.pull_request_reviews = fake_pull_request_reviews();
            self.review_threads = fake_review_threads();
            cx.notify();
        }
    }

    pub(crate) fn select_file(&mut self, index: usize, cx: &mut Context<Self>) {
        if let Some(file) = self.files.get(index) {
            self.active_file = index;
            self.active_hunk = 0;
            self.active_tab = PanelTab::Diff;
            self.file_list_scroll
                .scroll_to_item(index, ScrollStrategy::Center);
            self.diff_list_scroll.scroll_to_item(0, ScrollStrategy::Top);
            self.status = format!("Selected {}", file.path);
        }

        cx.notify();
    }

    fn select_next_file(&mut self, _: &SelectNextFile, _: &mut Window, cx: &mut Context<Self>) {
        if self.files.is_empty() {
            self.status = "No changed files to select".to_string();
            cx.notify();
            return;
        }

        self.select_file((self.active_file + 1) % self.files.len(), cx);
    }

    fn select_previous_file(
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

    fn select_next_hunk(&mut self, _: &SelectNextHunk, _: &mut Window, cx: &mut Context<Self>) {
        let Some(hunk_count) = self.active_diff().map(|diff| diff.hunks.len()) else {
            self.status = "No parsed diff hunks for active file".to_string();
            cx.notify();
            return;
        };

        self.active_hunk = (self.active_hunk + 1) % hunk_count;
        if let Some(row_index) = self
            .active_diff()
            .and_then(|diff| diff_hunk_row_index(diff, self.active_hunk))
        {
            self.diff_list_scroll
                .scroll_to_item(row_index, ScrollStrategy::Center);
        }
        self.status = format!("Selected hunk {}", self.active_hunk + 1);
        cx.notify();
    }

    fn select_previous_hunk(
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
        if let Some(row_index) = self
            .active_diff()
            .and_then(|diff| diff_hunk_row_index(diff, self.active_hunk))
        {
            self.diff_list_scroll
                .scroll_to_item(row_index, ScrollStrategy::Center);
        }
        self.status = format!("Selected hunk {}", self.active_hunk + 1);
        cx.notify();
    }

    fn copy_active_file_path(
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

    fn open_active_file_on_github(
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

    fn load_pull_requests(&mut self, repo: RepoId, cx: &mut Context<Self>) {
        self.is_loading_prs = true;
        self.load_error = None;
        self.details_error = None;
        self.files_error = None;
        self.checks_error = None;
        self.workflows_error = None;
        self.reviews_error = None;
        self.logs_error = None;
        self.action_error = None;
        self.pr_action_error = None;
        self.status = format!("Loading open pull requests from {}", repo.full_name());

        let owner = repo.owner.clone();
        let name = repo.name.clone();

        cx.spawn(async move |this, cx| {
            let result = GitHubClient::new(GhCliTransport)
                .list_open_pull_requests(&owner, &name)
                .await;

            _ = this.update(cx, |view, cx| {
                view.is_loading_prs = false;

                match result {
                    Ok(pull_requests) => {
                        let count = pull_requests.len();
                        view.pull_requests = pull_requests;
                        view.files.clear();
                        view.diffs.clear();
                        view.check_runs.clear();
                        view.workflow_runs.clear();
                        view.workflow_jobs.clear();
                        view.pull_request_reviews.clear();
                        view.review_threads.clear();
                        view.log_chunk = None;
                        view.selected_pr = 0;
                        view.active_file = 0;
                        view.active_hunk = 0;
                        view.pr_list_scroll.scroll_to_item(0, ScrollStrategy::Top);
                        view.file_list_scroll.scroll_to_item(0, ScrollStrategy::Top);
                        view.diff_list_scroll.scroll_to_item(0, ScrollStrategy::Top);
                        view.review_list_scroll
                            .scroll_to_item(0, ScrollStrategy::Top);
                        view.log_list_scroll.scroll_to_item(0, ScrollStrategy::Top);
                        view.load_error = None;
                        view.status =
                            format!("Loaded {count} open pull requests from {owner}/{name}");
                        view.load_selected_pull_request(cx);
                    }
                    Err(error) => {
                        view.pull_requests.clear();
                        view.files.clear();
                        view.diffs.clear();
                        view.check_runs.clear();
                        view.workflow_runs.clear();
                        view.workflow_jobs.clear();
                        view.pull_request_reviews.clear();
                        view.review_threads.clear();
                        view.log_chunk = None;
                        view.selected_pr = 0;
                        view.active_file = 0;
                        view.active_hunk = 0;
                        view.pr_list_scroll.scroll_to_item(0, ScrollStrategy::Top);
                        view.file_list_scroll.scroll_to_item(0, ScrollStrategy::Top);
                        view.diff_list_scroll.scroll_to_item(0, ScrollStrategy::Top);
                        view.review_list_scroll
                            .scroll_to_item(0, ScrollStrategy::Top);
                        view.is_loading_details = false;
                        view.is_loading_files = false;
                        view.is_loading_checks = false;
                        view.is_loading_workflows = false;
                        view.is_loading_reviews = false;
                        view.is_loading_logs = false;
                        view.is_running_action = false;
                        view.is_running_pr_action = false;
                        view.load_error = Some(error.to_string());
                        view.status = format!("Failed to load pull requests from {owner}/{name}");
                    }
                }

                cx.notify();
            });
        })
        .detach();
    }

    fn load_selected_pull_request(&mut self, cx: &mut Context<Self>) {
        let Some(repo) = self.configured_repo.clone() else {
            return;
        };
        let Some(number) = self.selected_pull_request_number() else {
            return;
        };
        let head_sha = self
            .selected_pull_request()
            .map(|pull_request| pull_request.head_sha.clone())
            .unwrap_or_default();

        self.is_loading_details = true;
        self.is_loading_files = true;
        self.is_loading_checks = true;
        self.is_loading_workflows = true;
        self.is_loading_reviews = true;
        self.details_error = None;
        self.files_error = None;
        self.checks_error = None;
        self.workflows_error = None;
        self.reviews_error = None;
        self.logs_error = None;
        self.action_error = None;
        self.pr_action_error = None;
        self.files.clear();
        self.diffs.clear();
        self.check_runs.clear();
        self.workflow_runs.clear();
        self.workflow_jobs.clear();
        self.pull_request_reviews.clear();
        self.review_threads.clear();
        self.log_chunk = None;
        self.active_file = 0;
        self.active_hunk = 0;
        self.file_list_scroll.scroll_to_item(0, ScrollStrategy::Top);
        self.diff_list_scroll.scroll_to_item(0, ScrollStrategy::Top);
        self.review_list_scroll
            .scroll_to_item(0, ScrollStrategy::Top);
        self.log_list_scroll.scroll_to_item(0, ScrollStrategy::Top);
        self.status = format!("Loading PR #{number} details and changed files");

        let owner = repo.owner.clone();
        let name = repo.name.clone();

        cx.spawn(async move |this, cx| {
            let client = GitHubClient::new(GhCliTransport);
            let detail_result = client.get_pull_request(&owner, &name, number).await;
            let files_result = client
                .list_pull_request_files(&owner, &name, number)
                .await
                .map(|files| {
                    let diffs = parse_files(&files);
                    (files, diffs)
                });
            let checks_result = if head_sha.is_empty() {
                Ok(Vec::new())
            } else {
                client.list_check_runs(&owner, &name, &head_sha).await
            };
            let workflow_runs_result = if head_sha.is_empty() {
                Ok(Vec::new())
            } else {
                client
                    .list_workflow_runs_for_head(&owner, &name, &head_sha)
                    .await
            };
            let pull_request_reviews_result = client
                .list_pull_request_reviews(&owner, &name, number)
                .await;
            let review_threads_result = client.list_review_threads(&owner, &name, number).await;

            _ = this.update(cx, move |view, cx| {
                if view.selected_pull_request_number() != Some(number) {
                    return;
                }

                view.is_loading_details = false;
                view.is_loading_files = false;
                view.is_loading_checks = false;
                view.is_loading_workflows = false;
                view.is_loading_reviews = false;

                match detail_result {
                    Ok(detail) => {
                        if let Some(selected) = view.pull_requests.get_mut(view.selected_pr) {
                            *selected = detail;
                        }
                        view.details_error = None;
                    }
                    Err(error) => {
                        view.details_error = Some(error.to_string());
                    }
                }

                let mut loaded_file_count = None;

                match files_result {
                    Ok((files, diffs)) => {
                        let count = files.len();
                        view.files = files;
                        view.diffs = diffs;
                        view.active_file = 0;
                        view.active_hunk = 0;
                        view.file_list_scroll.scroll_to_item(0, ScrollStrategy::Top);
                        view.diff_list_scroll.scroll_to_item(0, ScrollStrategy::Top);
                        view.files_error = None;
                        loaded_file_count = Some(count);
                    }
                    Err(error) => {
                        view.files.clear();
                        view.diffs.clear();
                        view.active_file = 0;
                        view.active_hunk = 0;
                        view.file_list_scroll.scroll_to_item(0, ScrollStrategy::Top);
                        view.diff_list_scroll.scroll_to_item(0, ScrollStrategy::Top);
                        view.files_error = Some(error.to_string());
                    }
                }

                match checks_result {
                    Ok(check_runs) => {
                        let summary = checks_summary_from_runs(&check_runs);
                        view.check_runs = check_runs;
                        view.checks_error = None;

                        if let Some(selected) = view.pull_requests.get_mut(view.selected_pr) {
                            selected.checks_summary = summary;
                        }
                    }
                    Err(error) => {
                        view.check_runs.clear();
                        view.checks_error = Some(error.to_string());
                    }
                }

                match workflow_runs_result {
                    Ok(workflow_runs) => {
                        view.workflow_runs = workflow_runs;
                        view.workflows_error = None;
                    }
                    Err(error) => {
                        view.workflow_runs.clear();
                        view.workflows_error = Some(error.to_string());
                    }
                }

                let mut loaded_review_thread_count = None;

                match pull_request_reviews_result {
                    Ok(reviews) => {
                        view.pull_request_reviews = reviews;
                        view.reviews_error = None;
                    }
                    Err(error) => {
                        view.pull_request_reviews.clear();
                        view.reviews_error =
                            Some(format!("Failed to load review history: {error}"));
                    }
                }

                match review_threads_result {
                    Ok(review_threads) => {
                        let unresolved_count = review_threads
                            .iter()
                            .filter(|thread| thread.state == ReviewThreadState::Unresolved)
                            .count();
                        let thread_count = review_threads.len();
                        view.review_threads = review_threads;
                        if let Some(selected) = view.pull_requests.get_mut(view.selected_pr) {
                            selected.unresolved_threads = unresolved_count;
                        }
                        loaded_review_thread_count = Some(thread_count);
                    }
                    Err(error) => {
                        view.review_threads.clear();
                        let message = format!("Failed to load review threads: {error}");
                        view.reviews_error = Some(match view.reviews_error.take() {
                            Some(existing) => format!("{existing}; {message}"),
                            None => message,
                        });
                    }
                }

                view.status = match (
                    view.details_error.as_ref(),
                    view.files_error.as_ref(),
                    loaded_file_count,
                ) {
                    (None, None, Some(count)) => {
                        format!("Loaded PR #{number} details and {count} files")
                    }
                    (Some(_), None, Some(count)) => {
                        format!("Loaded {count} files for PR #{number}, but details failed")
                    }
                    (None, Some(_), _) => {
                        format!("Loaded PR #{number} details, but files failed")
                    }
                    (Some(_), Some(_), _) => {
                        format!("Failed to load PR #{number} details and files")
                    }
                    _ => format!("Loaded PR #{number}"),
                };

                if let Some(count) = loaded_review_thread_count {
                    view.status = format!("{} and {count} review threads", view.status);
                }

                if view.active_tab == PanelTab::Logs
                    && view.logs_error.is_none()
                    && !view.workflow_runs.is_empty()
                {
                    view.load_selected_workflow_logs(cx);
                }

                cx.notify();
            });
        })
        .detach();
    }

    pub(crate) fn load_selected_workflow_logs(&mut self, cx: &mut Context<Self>) {
        let Some(repo) = self.configured_repo.clone() else {
            self.logs_error =
                Some("Workflow logs require HARBOR_REPO=owner/repo and GitHub CLI auth".into());
            self.status = self.logs_error.clone().unwrap_or_default();
            cx.notify();
            return;
        };
        let Some(run) = self.selected_workflow_run_for_logs().cloned() else {
            self.logs_error = Some("No workflow run is available for the selected PR head".into());
            self.status = self.logs_error.clone().unwrap_or_default();
            cx.notify();
            return;
        };

        if self.is_loading_logs {
            self.status = format!("Already loading logs for {}", workflow_run_label(&run));
            cx.notify();
            return;
        }

        self.active_tab = PanelTab::Logs;
        self.is_loading_logs = true;
        self.logs_error = None;
        self.workflow_jobs.clear();
        self.log_chunk = None;
        self.log_list_scroll.scroll_to_item(0, ScrollStrategy::Top);
        self.status = format!("Loading logs for {}", workflow_run_label(&run));

        let owner = repo.owner.clone();
        let name = repo.name.clone();
        let run_id = run.id;
        let run_label = workflow_run_label(&run);

        cx.spawn(async move |this, cx| {
            let client = GitHubClient::new(GhCliTransport);
            let jobs_result = client
                .list_workflow_jobs_for_run(&owner, &name, run_id)
                .await;
            let log_result = client.workflow_run_log(&owner, &name, run_id).await;

            _ = this.update(cx, move |view, cx| {
                if view.selected_workflow_run_for_logs().map(|run| run.id) != Some(run_id) {
                    return;
                }

                view.is_loading_logs = false;

                match jobs_result {
                    Ok(jobs) => {
                        view.workflow_jobs = jobs;
                    }
                    Err(error) => {
                        view.workflow_jobs.clear();
                        view.logs_error = Some(format!("Failed to load workflow jobs: {error}"));
                    }
                }

                match log_result {
                    Ok(text) => {
                        view.log_chunk = Some(parse_workflow_log(run_id, &text));
                        if view.logs_error.is_none() {
                            view.status = format!("Loaded logs for {run_label}");
                        } else {
                            view.status = format!("Loaded logs for {run_label}, but jobs failed");
                        }
                    }
                    Err(error) => {
                        view.log_chunk = None;
                        let message = format!("Failed to load workflow logs: {error}");
                        view.logs_error = Some(message.clone());
                        view.status = message;
                    }
                }

                view.log_list_scroll.scroll_to_item(0, ScrollStrategy::Top);
                cx.notify();
            });
        })
        .detach();
    }

    fn select_next(&mut self, _: &SelectNextPullRequest, _: &mut Window, cx: &mut Context<Self>) {
        if !self.pull_requests.is_empty() {
            let next = (self.selected_pr + 1) % self.pull_requests.len();
            self.select_pull_request(next, cx);
        } else {
            self.status = "No pull requests to select".to_string();
            cx.notify();
        }
    }

    fn select_previous(
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

    fn open_selected(
        &mut self,
        _: &OpenSelectedPullRequest,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.status = format!("Opened {} in the local shell", self.selected_pr_label());
        cx.notify();
    }

    fn cycle_panel_tab(&mut self, _: &CyclePanelTab, _: &mut Window, cx: &mut Context<Self>) {
        self.active_tab = self.active_tab.next();
        self.status = format!("Switched to {} panel", self.active_tab.label());
        cx.notify();
    }

    fn toggle_command_palette(
        &mut self,
        _: &ToggleCommandPalette,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.command_palette_open = !self.command_palette_open;
        self.status = if self.command_palette_open {
            "Command palette opened".to_string()
        } else {
            "Command palette closed".to_string()
        };
        cx.notify();
    }

    fn close_panel(&mut self, _: &ClosePanel, _: &mut Window, cx: &mut Context<Self>) {
        self.command_palette_open = false;
        self.status = "Closed transient UI".to_string();
        cx.notify();
    }

    fn set_placeholder_status(&mut self, label: &str, cx: &mut Context<Self>) {
        self.status = format!(
            "{label} is wired as a command placeholder for {}",
            self.selected_pr_label()
        );
        cx.notify();
    }

    fn workflow_action_request(
        &self,
        action: WorkflowAction,
    ) -> std::result::Result<WorkflowActionRequest, String> {
        let Some(repo) = self.configured_repo.clone() else {
            return Err(
                "Workflow actions require HARBOR_REPO=owner/repo and GitHub CLI auth".into(),
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

    fn pull_request_action_request(
        &self,
        action: PullRequestAction,
    ) -> std::result::Result<PullRequestActionRequest, String> {
        let Some(repo) = self.configured_repo.clone() else {
            return Err(
                "Pull request actions require HARBOR_REPO=owner/repo and GitHub CLI auth".into(),
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

    fn refresh_selected(
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

    fn checkout_pr(&mut self, _: &CheckoutPullRequest, _: &mut Window, cx: &mut Context<Self>) {
        self.set_placeholder_status("Checkout", cx);
    }

    fn open_in_browser(
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

    fn approve_pr(&mut self, _: &ApprovePullRequest, _: &mut Window, cx: &mut Context<Self>) {
        self.run_pull_request_action(PullRequestAction::Approve, cx);
    }

    fn request_changes(&mut self, _: &RequestChanges, _: &mut Window, cx: &mut Context<Self>) {
        self.run_pull_request_action(PullRequestAction::RequestChanges, cx);
    }

    fn merge_pr(&mut self, _: &MergePullRequest, _: &mut Window, cx: &mut Context<Self>) {
        self.run_pull_request_action(PullRequestAction::Merge, cx);
    }

    fn open_logs(&mut self, _: &OpenLogs, _: &mut Window, cx: &mut Context<Self>) {
        self.active_tab = PanelTab::Logs;
        if self.configured_repo.is_some() {
            self.load_selected_workflow_logs(cx);
        } else {
            self.set_placeholder_status("Open logs", cx);
        }
    }

    fn trigger_build(&mut self, _: &TriggerBuild, _: &mut Window, cx: &mut Context<Self>) {
        self.run_workflow_action(WorkflowAction::DispatchBuild, cx);
    }

    fn rerun_failed(&mut self, _: &RerunFailedJobs, _: &mut Window, cx: &mut Context<Self>) {
        self.run_workflow_action(WorkflowAction::RerunFailedJobs, cx);
    }

    fn filter_current_list(
        &mut self,
        _: &FilterCurrentList,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.set_placeholder_status("Filter", cx);
    }
}

impl Focusable for AppView {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for AppView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if !self.did_focus {
            window.focus(&self.focus_handle, cx);
            self.did_focus = true;
        }

        let selected_pr = self.selected_pull_request().cloned();

        div()
            .key_context(KEY_CONTEXT)
            .track_focus(&self.focus_handle(cx))
            .on_action(cx.listener(Self::select_next))
            .on_action(cx.listener(Self::select_previous))
            .on_action(cx.listener(Self::open_selected))
            .on_action(cx.listener(Self::cycle_panel_tab))
            .on_action(cx.listener(Self::toggle_command_palette))
            .on_action(cx.listener(Self::close_panel))
            .on_action(cx.listener(Self::refresh_selected))
            .on_action(cx.listener(Self::checkout_pr))
            .on_action(cx.listener(Self::open_in_browser))
            .on_action(cx.listener(Self::approve_pr))
            .on_action(cx.listener(Self::request_changes))
            .on_action(cx.listener(Self::merge_pr))
            .on_action(cx.listener(Self::open_logs))
            .on_action(cx.listener(Self::trigger_build))
            .on_action(cx.listener(Self::rerun_failed))
            .on_action(cx.listener(Self::filter_current_list))
            .on_action(cx.listener(Self::select_next_file))
            .on_action(cx.listener(Self::select_previous_file))
            .on_action(cx.listener(Self::select_next_hunk))
            .on_action(cx.listener(Self::select_previous_hunk))
            .on_action(cx.listener(Self::copy_active_file_path))
            .on_action(cx.listener(Self::open_active_file_on_github))
            .size_full()
            .flex()
            .flex_col()
            .bg(rgb(0x101214))
            .text_color(rgb(0xe6e8eb))
            .child(self.render_header())
            .when(self.command_palette_open, |element| {
                element.child(self.render_command_palette())
            })
            .child(
                div()
                    .flex()
                    .flex_1()
                    .min_h_0()
                    .min_w_0()
                    .overflow_hidden()
                    .gap_2()
                    .p_2()
                    .child(self.render_inbox(cx))
                    .child(self.render_details(selected_pr.as_ref(), cx))
                    .child(self.render_panel(selected_pr.as_ref(), cx)),
            )
            .child(
                div()
                    .px_3()
                    .py_2()
                    .text_xs()
                    .text_color(rgb(0x9aa4b2))
                    .border_1()
                    .border_color(rgb(0x242a31))
                    .child(self.status.clone()),
            )
    }
}

impl AppView {
    fn render_header(&self) -> impl IntoElement {
        div()
            .flex()
            .items_center()
            .justify_between()
            .px_4()
            .py_3()
            .border_1()
            .border_color(rgb(0x242a31))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_3()
                    .child(div().text_lg().child("Harbor"))
                    .child(
                        div()
                            .text_xs()
                            .text_color(rgb(0x9aa4b2))
                            .child("native GitHub pull request cockpit"),
                    ),
            )
            .child(
                div()
                    .flex()
                    .gap_2()
                    .child(
                        Button::new("repo-switcher")
                            .label("cmd+p")
                            .small()
                            .outline(),
                    )
                    .child(Button::new("command-palette").label("cmd+k").small()),
            )
    }

    fn render_command_palette(&self) -> impl IntoElement {
        div()
            .mx_2()
            .mt_2()
            .p_3()
            .rounded_md()
            .border_1()
            .border_color(rgb(0x3a424c))
            .bg(rgb(0x171b20))
            .child(
                div()
                    .pb_2()
                    .text_sm()
                    .text_color(rgb(0xf1f5f9))
                    .child("Command palette placeholder"),
            )
            .children(COMMANDS.iter().map(|command| {
                div()
                    .flex()
                    .justify_between()
                    .py_1()
                    .text_sm()
                    .child(command.title)
                    .child(
                        div()
                            .text_xs()
                            .text_color(rgb(0x9aa4b2))
                            .child(command.shortcut),
                    )
            }))
    }

    fn render_inbox(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let show_list =
            !self.is_loading_prs && self.load_error.is_none() && !self.pull_requests.is_empty();

        div()
            .w(px(320.))
            .flex()
            .flex_col()
            .min_h_0()
            .rounded_md()
            .border_1()
            .border_color(rgb(0x242a31))
            .bg(rgb(0x15191e))
            .overflow_hidden()
            .child(
                div()
                    .px_3()
                    .py_2()
                    .text_sm()
                    .text_color(rgb(0xf1f5f9))
                    .child("Pull request inbox")
                    .child(
                        div().pt_1().text_xs().text_color(rgb(0x9aa4b2)).child(
                            self.configured_repo
                                .as_ref()
                                .map(RepoId::full_name)
                                .unwrap_or_else(|| "fake data".to_string()),
                        ),
                    ),
            )
            .when(self.is_loading_prs, |element| {
                element.child(
                    div()
                        .flex_1()
                        .px_3()
                        .py_3()
                        .text_sm()
                        .text_color(rgb(0x9aa4b2))
                        .child("Loading open pull requests..."),
                )
            })
            .when(
                !self.is_loading_prs && self.load_error.is_some(),
                |element| {
                    element.child(
                        div()
                            .flex_1()
                            .px_3()
                            .py_3()
                            .text_sm()
                            .text_color(rgb(0xf87171))
                            .child(self.load_error.clone().unwrap_or_default()),
                    )
                },
            )
            .when(
                !self.is_loading_prs && self.load_error.is_none() && self.pull_requests.is_empty(),
                |element| {
                    element.child(
                        div()
                            .flex_1()
                            .px_3()
                            .py_3()
                            .text_sm()
                            .text_color(rgb(0x9aa4b2))
                            .child("No open pull requests"),
                    )
                },
            )
            .when(show_list, |element| {
                element.child(
                    uniform_list(
                        "pull-request-inbox-list",
                        self.pull_requests.len(),
                        cx.processor(|view, range: std::ops::Range<usize>, _window, cx| {
                            let mut rows = Vec::with_capacity(range.len());

                            for index in range {
                                let Some(pr) = view.pull_requests.get(index) else {
                                    continue;
                                };
                                rows.push(render_pull_request_row(
                                    index,
                                    pr,
                                    index == view.selected_pr,
                                    cx,
                                ));
                            }

                            rows
                        }),
                    )
                    .track_scroll(&self.pr_list_scroll)
                    .flex_1()
                    .min_h_0()
                    .w_full(),
                )
            })
    }

    fn render_details(&self, pr: Option<&PullRequest>, cx: &mut Context<Self>) -> impl IntoElement {
        let Some(pr) = pr else {
            return div()
                .w(px(360.))
                .flex()
                .flex_col()
                .min_h_0()
                .rounded_md()
                .border_1()
                .border_color(rgb(0x242a31))
                .bg(rgb(0x15191e))
                .overflow_hidden()
                .p_3()
                .text_sm()
                .text_color(rgb(0x9aa4b2))
                .child("Select a pull request to see details")
                .into_any_element();
        };

        let review_action_disabled = self.configured_repo.is_none()
            || self.is_running_pr_action
            || review_action_blocker(pr).is_some();
        let merge_action_disabled = self.configured_repo.is_none()
            || self.is_running_pr_action
            || merge_blocker(pr).is_some();

        div()
            .w(px(360.))
            .flex()
            .flex_col()
            .min_h_0()
            .rounded_md()
            .border_1()
            .border_color(rgb(0x242a31))
            .bg(rgb(0x15191e))
            .overflow_hidden()
            .child(
                div()
                    .p_3()
                    .border_1()
                    .border_color(rgb(0x242a31))
                    .child(
                        div()
                            .text_sm()
                            .child(format!("#{} {}", pr.number, pr.title)),
                    )
                    .child(
                        div()
                            .pt_1()
                            .text_xs()
                            .text_color(rgb(0x9aa4b2))
                            .child(format!("{} / {}", pr.repo.full_name(), pr.head_sha)),
                    )
                    .when(self.is_loading_details, |element| {
                        element.child(
                            div()
                                .pt_2()
                                .text_xs()
                                .text_color(rgb(0x9aa4b2))
                                .child("Loading latest PR details..."),
                        )
                    })
                    .when_some(self.details_error.clone(), |element, error| {
                        element.child(
                            div()
                                .pt_2()
                                .text_xs()
                                .text_color(rgb(0xf87171))
                                .child(error),
                        )
                    })
                    .child(
                        div()
                            .pt_2()
                            .flex()
                            .gap_2()
                            .child(render_review_decision(pr.review_decision))
                            .child(render_merge_state(pr.merge_state))
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(rgb(0xfbbf24))
                                    .child(format!("{} unresolved", pr.unresolved_threads)),
                            ),
                    )
                    .child(
                        div()
                            .pt_3()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(
                                Button::new("approve-pr")
                                    .label("approve")
                                    .small()
                                    .outline()
                                    .loading(self.is_running_pr_action)
                                    .disabled(review_action_disabled)
                                    .on_click(cx.listener(|view, _, _, cx| {
                                        view.run_pull_request_action(
                                            PullRequestAction::Approve,
                                            cx,
                                        );
                                    })),
                            )
                            .child(
                                Button::new("request-pr-changes")
                                    .label("changes")
                                    .small()
                                    .outline()
                                    .loading(self.is_running_pr_action)
                                    .disabled(review_action_disabled)
                                    .on_click(cx.listener(|view, _, _, cx| {
                                        view.run_pull_request_action(
                                            PullRequestAction::RequestChanges,
                                            cx,
                                        );
                                    })),
                            )
                            .child(
                                Button::new("merge-pr")
                                    .label("merge")
                                    .small()
                                    .outline()
                                    .loading(self.is_running_pr_action)
                                    .disabled(merge_action_disabled)
                                    .on_click(cx.listener(|view, _, _, cx| {
                                        view.run_pull_request_action(PullRequestAction::Merge, cx);
                                    })),
                            ),
                    )
                    .when_some(self.pr_action_error.clone(), |element, error| {
                        element.child(
                            div()
                                .pt_2()
                                .text_xs()
                                .text_color(rgb(0xf87171))
                                .child(error),
                        )
                    }),
            )
            .child(
                div()
                    .px_3()
                    .py_2()
                    .text_xs()
                    .text_color(rgb(0x9aa4b2))
                    .child("Changed files"),
            )
            .when(self.is_loading_files, |element| {
                element.child(
                    div()
                        .flex_1()
                        .px_3()
                        .py_3()
                        .text_sm()
                        .text_color(rgb(0x9aa4b2))
                        .child("Loading changed files..."),
                )
            })
            .when_some(self.files_error.clone(), |element, error| {
                element.child(
                    div()
                        .flex_1()
                        .px_3()
                        .py_3()
                        .text_sm()
                        .text_color(rgb(0xf87171))
                        .child(error),
                )
            })
            .when(
                !self.is_loading_files && self.files_error.is_none() && self.files.is_empty(),
                |element| {
                    element.child(
                        div()
                            .flex_1()
                            .px_3()
                            .py_3()
                            .text_sm()
                            .text_color(rgb(0x9aa4b2))
                            .child("No changed files"),
                    )
                },
            )
            .when(
                !self.is_loading_files && self.files_error.is_none() && !self.files.is_empty(),
                |element| {
                    element.child(
                        uniform_list(
                            "changed-files-list",
                            self.files.len(),
                            cx.processor(|view, range: std::ops::Range<usize>, _window, cx| {
                                let mut rows = Vec::with_capacity(range.len());

                                for index in range {
                                    let Some(file) = view.files.get(index) else {
                                        continue;
                                    };
                                    rows.push(render_changed_file_row(
                                        index,
                                        file,
                                        index == view.active_file,
                                        cx,
                                    ));
                                }

                                rows
                            }),
                        )
                        .track_scroll(&self.file_list_scroll)
                        .flex_1()
                        .min_h_0()
                        .w_full(),
                    )
                },
            )
            .into_any_element()
    }

    fn render_panel(&self, pr: Option<&PullRequest>, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex_1()
            .flex()
            .flex_col()
            .min_h_0()
            .min_w_0()
            .rounded_md()
            .border_1()
            .border_color(rgb(0x242a31))
            .bg(rgb(0x15191e))
            .overflow_hidden()
            .child(
                div()
                    .flex()
                    .gap_2()
                    .p_2()
                    .border_1()
                    .border_color(rgb(0x242a31))
                    .children(PanelTab::ALL.iter().map(|tab| {
                        let active = *tab == self.active_tab;
                        div()
                            .px_3()
                            .py_1()
                            .rounded_sm()
                            .text_sm()
                            .when(active, |element| element.bg(rgb(0x243244)))
                            .child(tab.label())
                    })),
            )
            .child(
                div()
                    .id("panel-content-scroll")
                    .flex_1()
                    .flex()
                    .flex_col()
                    .min_h_0()
                    .min_w_0()
                    .p_3()
                    .text_sm()
                    .child(match self.active_tab {
                        PanelTab::Diff => render_diff_panel(
                            self.active_file(),
                            self.active_diff(),
                            self.is_loading_files,
                            self.files_error.as_deref(),
                            self.diff_list_scroll.clone(),
                            cx,
                        )
                        .into_any_element(),
                        PanelTab::Review => render_review_panel(
                            &self.pull_request_reviews,
                            &self.review_threads,
                            self.is_loading_reviews,
                            self.reviews_error.as_deref(),
                            self.review_list_scroll.clone(),
                            cx,
                        )
                        .into_any_element(),
                        PanelTab::Checks => render_checks_panel(
                            pr.map(|pr| pr.checks_summary).unwrap_or_default(),
                            &self.check_runs,
                            self.is_loading_checks,
                            self.checks_error.as_deref(),
                        )
                        .into_any_element(),
                        PanelTab::Actions => render_actions_panel(
                            pr,
                            &self.workflow_runs,
                            self.is_loading_workflows,
                            self.workflows_error.as_deref(),
                            self.action_error.as_deref(),
                            self.is_running_action,
                            cx,
                        )
                        .into_any_element(),
                        PanelTab::Logs => render_logs_panel(
                            self.selected_workflow_run_for_logs(),
                            &self.workflow_jobs,
                            self.log_chunk.as_ref(),
                            self.is_loading_logs,
                            self.logs_error.as_deref(),
                            self.log_list_scroll.clone(),
                            cx,
                        )
                        .into_any_element(),
                    }),
            )
    }
}
