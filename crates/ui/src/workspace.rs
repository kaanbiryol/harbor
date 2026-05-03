mod commands;
mod loaders;
mod render;

use gpui::{
    AppContext, Context, Entity, FocusHandle, ScrollStrategy, Subscription, Task,
    UniformListScrollHandle, Window,
};
use gpui_component::input::{InputEvent, InputState};
use harbor_domain::{
    CheckRun, DiffFile, PullRequest, PullRequestReview, RepoId, ReviewThread, WorkflowJob,
    WorkflowRun,
};
use harbor_logs::LogChunk;
use harbor_storage::SqliteStore;

use crate::actions::PanelTab;
use crate::diff::{ParsedDiff, parse_files};
use crate::panels::workflow_run_failed;
pub struct AppView {
    focus_handle: FocusHandle,
    pull_requests: Vec<PullRequest>,
    repositories: Vec<RepoId>,
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
    repository_task: Option<Task<()>>,
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
    repository_switcher_open: bool,
    pull_request_switcher_open: bool,
    repository_search_input: Entity<InputState>,
    pull_request_search_input: Entity<InputState>,
    configured_repo: Option<RepoId>,
    repository_store: Option<SqliteStore>,
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
    repository_error: Option<String>,
    action_error: Option<String>,
    pr_action_error: Option<String>,
    did_focus: bool,
    status: String,
    _subscriptions: Vec<Subscription>,
}

impl AppView {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let configured_repo = configured_repo_from_env();
        let pull_requests = Vec::new();
        let files = Vec::new();
        let pull_request_reviews = Vec::new();
        let review_threads = Vec::new();
        let repository_search_input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("Search repositories...")
                .clean_on_escape()
        });
        let pull_request_search_input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("Search pull requests...")
                .clean_on_escape()
        });
        let subscriptions = vec![
            cx.subscribe_in(
                &repository_search_input,
                window,
                Self::on_switcher_search_event,
            ),
            cx.subscribe_in(
                &pull_request_search_input,
                window,
                Self::on_switcher_search_event,
            ),
        ];
        let diffs = parse_files(&files);
        let repositories = initial_repositories(configured_repo.as_ref(), &pull_requests);
        let status = configured_repo
            .as_ref()
            .map(|repo| format!("Loading open pull requests from {}", repo.full_name()))
            .unwrap_or_else(|| "Loading repositories from GitHub CLI".to_string());

        let mut view = Self {
            focus_handle: cx.focus_handle(),
            pull_requests,
            repositories,
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
            repository_task: None,
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
            repository_switcher_open: false,
            pull_request_switcher_open: false,
            repository_search_input,
            pull_request_search_input,
            configured_repo,
            repository_store: None,
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
            repository_error: None,
            action_error: None,
            pr_action_error: None,
            did_focus: false,
            status,
            _subscriptions: subscriptions,
        };

        view.load_recent_repositories(cx);

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

    pub(crate) fn active_file(&self) -> Option<&DiffFile> {
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

    pub(crate) fn remember_repository(&mut self, repository: RepoId) {
        self.repositories.retain(|existing| existing != &repository);
        self.repositories.insert(0, repository);
    }

    fn on_switcher_search_event(
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

fn initial_repositories(
    configured_repo: Option<&RepoId>,
    pull_requests: &[PullRequest],
) -> Vec<RepoId> {
    let mut repositories = Vec::new();

    if let Some(repository) = configured_repo {
        repositories.push(repository.clone());
    }

    for pull_request in pull_requests {
        if !repositories
            .iter()
            .any(|repository| repository == &pull_request.repo)
        {
            repositories.push(pull_request.repo.clone());
        }
    }

    repositories
}

pub(crate) fn configured_repo_from_env() -> Option<RepoId> {
    std::env::var("HARBOR_REPO")
        .ok()
        .or_else(|| std::env::var("GH_REPO").ok())
        .and_then(|value| parse_repo_id(&value))
}

pub(crate) fn parse_repo_id(value: &str) -> Option<RepoId> {
    let (owner, name) = value.split_once('/')?;

    if owner.is_empty() || name.is_empty() || name.contains('/') {
        None
    } else {
        Some(RepoId::new(owner, name))
    }
}
