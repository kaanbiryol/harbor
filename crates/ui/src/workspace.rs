mod commands;
mod loaders;
mod render;

use gpui::{Context, FocusHandle, ScrollStrategy, Task, UniformListScrollHandle};
use harbor_domain::{
    CheckRun, DiffFile, PullRequest, PullRequestReview, RepoId, ReviewThread, WorkflowJob,
    WorkflowRun,
};
use harbor_logs::LogChunk;

use crate::actions::PanelTab;
use crate::diff::{ParsedDiff, parse_files};
use crate::fake_data::{
    configured_repo_from_env, fake_files, fake_pull_request_reviews, fake_pull_requests,
    fake_review_threads,
};
use crate::panels::workflow_run_failed;
pub struct AppView {
    focus_handle: FocusHandle,
    pull_requests: Vec<PullRequest>,
    files: Vec<DiffFile>,
    diffs: Vec<Option<ParsedDiff>>,
    check_runs: Vec<CheckRun>,
    workflow_runs: Vec<WorkflowRun>,
    workflow_jobs: Vec<WorkflowJob>,
    pull_request_reviews: Vec<PullRequestReview>,
    pub(crate) review_threads: Vec<ReviewThread>,
    pub(crate) log_chunk: Option<LogChunk>,
    pr_list_task: Option<Task<()>>,
    pr_detail_task: Option<Task<()>>,
    log_task: Option<Task<()>>,
    pr_list_scroll: UniformListScrollHandle,
    file_list_scroll: UniformListScrollHandle,
    diff_list_scroll: UniformListScrollHandle,
    review_list_scroll: UniformListScrollHandle,
    log_list_scroll: UniformListScrollHandle,
    selected_pr: usize,
    active_file: usize,
    pub(crate) active_hunk: usize,
    active_tab: PanelTab,
    command_palette_open: bool,
    configured_repo: Option<RepoId>,
    is_loading_prs: bool,
    is_loading_details: bool,
    is_loading_files: bool,
    is_loading_checks: bool,
    is_loading_workflows: bool,
    is_loading_reviews: bool,
    is_loading_logs: bool,
    is_running_action: bool,
    is_running_pr_action: bool,
    load_error: Option<String>,
    details_error: Option<String>,
    files_error: Option<String>,
    checks_error: Option<String>,
    workflows_error: Option<String>,
    reviews_error: Option<String>,
    logs_error: Option<String>,
    action_error: Option<String>,
    pr_action_error: Option<String>,
    did_focus: bool,
    status: String,
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
            pr_list_task: None,
            pr_detail_task: None,
            log_task: None,
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
}
