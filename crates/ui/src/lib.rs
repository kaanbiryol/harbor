mod diff;

use gpui::{
    AnyElement, App, ClipboardItem, Context, FocusHandle, Focusable, IntoElement, KeyBinding,
    ListHorizontalSizingBehavior, Render, ScrollStrategy, UniformListScrollHandle, Window, actions,
    div, prelude::*, px, rgb, uniform_list,
};
use gpui_component::{Disableable, Sizable, button::Button};
use harbor_domain::{
    CheckConclusion, CheckRun, CheckStatus, ChecksSummary, DiffFile, FileStatus, Label, MergeState,
    PullRequest, PullRequestReview, PullRequestReviewState, PullRequestState, RepoId,
    ReviewComment, ReviewCommentPosition, ReviewDecision, ReviewSide, ReviewThread,
    ReviewThreadState, WorkflowConclusion, WorkflowJob, WorkflowRun, WorkflowStatus, WorkflowStep,
};
use harbor_github::{GhCliTransport, GitHubClient};
use harbor_logs::{LogChunk, LogLine, LogSeverity, parse_workflow_log};

use crate::diff::{DiffHunk, DiffLine, DiffLineKind, ParsedDiff, parse_files};

const KEY_CONTEXT: &str = "HarborWorkspace";

actions!(
    harbor,
    [
        SelectNextPullRequest,
        SelectPreviousPullRequest,
        OpenSelectedPullRequest,
        CyclePanelTab,
        ToggleCommandPalette,
        ClosePanel,
        RefreshSelectedPullRequest,
        CheckoutPullRequest,
        OpenPullRequestInBrowser,
        ApprovePullRequest,
        RequestChanges,
        MergePullRequest,
        OpenLogs,
        TriggerBuild,
        RerunFailedJobs,
        FilterCurrentList,
        SelectNextFile,
        SelectPreviousFile,
        SelectNextHunk,
        SelectPreviousHunk,
        CopyActiveFilePath,
        OpenActiveFileOnGitHub
    ]
);

pub fn bind_keys(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("j", SelectNextPullRequest, Some(KEY_CONTEXT)),
        KeyBinding::new("k", SelectPreviousPullRequest, Some(KEY_CONTEXT)),
        KeyBinding::new("enter", OpenSelectedPullRequest, Some(KEY_CONTEXT)),
        KeyBinding::new("tab", CyclePanelTab, Some(KEY_CONTEXT)),
        KeyBinding::new("cmd-k", ToggleCommandPalette, Some(KEY_CONTEXT)),
        KeyBinding::new("cmd-p", ToggleCommandPalette, Some(KEY_CONTEXT)),
        KeyBinding::new("escape", ClosePanel, Some(KEY_CONTEXT)),
        KeyBinding::new("r", RefreshSelectedPullRequest, Some(KEY_CONTEXT)),
        KeyBinding::new("c", CheckoutPullRequest, Some(KEY_CONTEXT)),
        KeyBinding::new("o", OpenPullRequestInBrowser, Some(KEY_CONTEXT)),
        KeyBinding::new("a", ApprovePullRequest, Some(KEY_CONTEXT)),
        KeyBinding::new("shift-a", RequestChanges, Some(KEY_CONTEXT)),
        KeyBinding::new("m", MergePullRequest, Some(KEY_CONTEXT)),
        KeyBinding::new("l", OpenLogs, Some(KEY_CONTEXT)),
        KeyBinding::new("b", TriggerBuild, Some(KEY_CONTEXT)),
        KeyBinding::new("shift-r", RerunFailedJobs, Some(KEY_CONTEXT)),
        KeyBinding::new("/", FilterCurrentList, Some(KEY_CONTEXT)),
        KeyBinding::new("]", SelectNextFile, Some(KEY_CONTEXT)),
        KeyBinding::new("[", SelectPreviousFile, Some(KEY_CONTEXT)),
        KeyBinding::new("shift-]", SelectNextHunk, Some(KEY_CONTEXT)),
        KeyBinding::new("shift-[", SelectPreviousHunk, Some(KEY_CONTEXT)),
        KeyBinding::new("y", CopyActiveFilePath, Some(KEY_CONTEXT)),
        KeyBinding::new("g", OpenActiveFileOnGitHub, Some(KEY_CONTEXT)),
    ]);
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PanelTab {
    Diff,
    Review,
    Checks,
    Actions,
    Logs,
}

impl PanelTab {
    const ALL: [Self; 5] = [
        Self::Diff,
        Self::Review,
        Self::Checks,
        Self::Actions,
        Self::Logs,
    ];

    fn label(self) -> &'static str {
        match self {
            Self::Diff => "Diff",
            Self::Review => "Review",
            Self::Checks => "Checks",
            Self::Actions => "Actions",
            Self::Logs => "Logs",
        }
    }

    fn next(self) -> Self {
        let index = Self::ALL
            .iter()
            .position(|tab| *tab == self)
            .expect("active tab must be present");
        Self::ALL[(index + 1) % Self::ALL.len()]
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct CommandSpec {
    shortcut: &'static str,
    title: &'static str,
}

#[derive(Clone, Copy, Debug)]
enum WorkflowAction {
    DispatchBuild,
    RerunFailedJobs,
}

#[derive(Clone, Copy, Debug)]
enum PullRequestAction {
    Approve,
    RequestChanges,
    Merge,
}

#[derive(Clone, Debug)]
enum WorkflowActionRequest {
    DispatchBuild {
        owner: String,
        repo: String,
        workflow_id: u64,
        git_ref: String,
        workflow_name: String,
    },
    RerunFailedJobs {
        owner: String,
        repo: String,
        run_id: u64,
        workflow_name: String,
    },
}

impl WorkflowActionRequest {
    fn start_status(&self) -> String {
        match self {
            Self::DispatchBuild {
                workflow_name,
                git_ref,
                ..
            } => format!("Dispatching {workflow_name} on {git_ref}"),
            Self::RerunFailedJobs { workflow_name, .. } => {
                format!("Requesting failed job rerun for {workflow_name}")
            }
        }
    }

    fn success_status(&self) -> String {
        match self {
            Self::DispatchBuild {
                workflow_name,
                git_ref,
                ..
            } => format!("Dispatched {workflow_name} on {git_ref}"),
            Self::RerunFailedJobs { workflow_name, .. } => {
                format!("Requested failed job rerun for {workflow_name}")
            }
        }
    }

    fn failure_label(&self) -> &'static str {
        match self {
            Self::DispatchBuild { .. } => "dispatch workflow",
            Self::RerunFailedJobs { .. } => "rerun failed jobs",
        }
    }
}

#[derive(Clone, Debug)]
enum PullRequestActionRequest {
    Approve {
        owner: String,
        repo: String,
        number: u64,
    },
    RequestChanges {
        owner: String,
        repo: String,
        number: u64,
        body: String,
    },
    Merge {
        owner: String,
        repo: String,
        number: u64,
        head_sha: String,
    },
}

impl PullRequestActionRequest {
    fn number(&self) -> u64 {
        match self {
            Self::Approve { number, .. }
            | Self::RequestChanges { number, .. }
            | Self::Merge { number, .. } => *number,
        }
    }

    fn start_status(&self) -> String {
        match self {
            Self::Approve { .. } => format!("Approving PR #{}", self.number()),
            Self::RequestChanges { .. } => {
                format!("Requesting changes on PR #{}", self.number())
            }
            Self::Merge { .. } => format!("Merging PR #{}", self.number()),
        }
    }

    fn success_status(&self) -> String {
        match self {
            Self::Approve { .. } => format!("Approved PR #{}", self.number()),
            Self::RequestChanges { .. } => {
                format!("Requested changes on PR #{}", self.number())
            }
            Self::Merge { .. } => format!("Merged PR #{}", self.number()),
        }
    }

    fn failure_label(&self) -> &'static str {
        match self {
            Self::Approve { .. } => "approve pull request",
            Self::RequestChanges { .. } => "request changes",
            Self::Merge { .. } => "merge pull request",
        }
    }
}

const DEFAULT_REQUEST_CHANGES_BODY: &str = "Changes requested from Harbor.";

const COMMANDS: &[CommandSpec] = &[
    CommandSpec {
        shortcut: "cmd+k",
        title: "Open command palette",
    },
    CommandSpec {
        shortcut: "cmd+p",
        title: "Switch repository or pull request",
    },
    CommandSpec {
        shortcut: "j/k",
        title: "Move pull request selection",
    },
    CommandSpec {
        shortcut: "enter",
        title: "Open selected pull request",
    },
    CommandSpec {
        shortcut: "tab",
        title: "Cycle right panel",
    },
    CommandSpec {
        shortcut: "r",
        title: "Refresh selected pull request",
    },
    CommandSpec {
        shortcut: "l",
        title: "Open logs",
    },
    CommandSpec {
        shortcut: "shift+r",
        title: "Rerun failed jobs",
    },
    CommandSpec {
        shortcut: "[ / ]",
        title: "Move between changed files",
    },
    CommandSpec {
        shortcut: "shift+[ / shift+]",
        title: "Move between diff hunks",
    },
    CommandSpec {
        shortcut: "y",
        title: "Copy active file path",
    },
    CommandSpec {
        shortcut: "g",
        title: "Open active file on GitHub",
    },
];

pub struct AppView {
    focus_handle: FocusHandle,
    pull_requests: Vec<PullRequest>,
    files: Vec<DiffFile>,
    diffs: Vec<Option<ParsedDiff>>,
    check_runs: Vec<CheckRun>,
    workflow_runs: Vec<WorkflowRun>,
    workflow_jobs: Vec<WorkflowJob>,
    pull_request_reviews: Vec<PullRequestReview>,
    review_threads: Vec<ReviewThread>,
    log_chunk: Option<LogChunk>,
    pr_list_scroll: UniformListScrollHandle,
    file_list_scroll: UniformListScrollHandle,
    diff_list_scroll: UniformListScrollHandle,
    review_list_scroll: UniformListScrollHandle,
    log_list_scroll: UniformListScrollHandle,
    selected_pr: usize,
    active_file: usize,
    active_hunk: usize,
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

    fn active_diff(&self) -> Option<&ParsedDiff> {
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

    fn select_pull_request(&mut self, index: usize, cx: &mut Context<Self>) {
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

    fn select_file(&mut self, index: usize, cx: &mut Context<Self>) {
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

    fn load_selected_workflow_logs(&mut self, cx: &mut Context<Self>) {
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

    fn run_workflow_action(&mut self, action: WorkflowAction, cx: &mut Context<Self>) {
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

    fn run_pull_request_action(&mut self, action: PullRequestAction, cx: &mut Context<Self>) {
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

fn render_checks_summary(summary: ChecksSummary) -> impl IntoElement {
    let color = if summary.failed > 0 {
        rgb(0xf87171)
    } else if summary.pending > 0 {
        rgb(0xfbbf24)
    } else {
        rgb(0x34d399)
    };

    div()
        .text_xs()
        .text_color(color)
        .child(format!("{}/{}", summary.passed, summary.total))
}

fn checks_summary_from_runs(check_runs: &[CheckRun]) -> ChecksSummary {
    let mut summary = ChecksSummary {
        total: check_runs.len(),
        ..ChecksSummary::default()
    };

    for check_run in check_runs {
        match (check_run.status, check_run.conclusion) {
            (CheckStatus::Completed, Some(CheckConclusion::Success)) => summary.passed += 1,
            (CheckStatus::Completed, Some(CheckConclusion::Skipped)) => summary.skipped += 1,
            (CheckStatus::Completed, Some(CheckConclusion::Neutral)) => summary.skipped += 1,
            (CheckStatus::Completed, Some(CheckConclusion::Cancelled)) => summary.failed += 1,
            (CheckStatus::Completed, Some(CheckConclusion::Failure)) => summary.failed += 1,
            (CheckStatus::Completed, Some(CheckConclusion::TimedOut)) => summary.failed += 1,
            (CheckStatus::Completed, Some(CheckConclusion::ActionRequired)) => summary.failed += 1,
            (CheckStatus::Completed, None) => summary.failed += 1,
            (CheckStatus::InProgress | CheckStatus::Queued, _) => summary.pending += 1,
        }
    }

    summary
}

fn review_action_blocker(pr: &PullRequest) -> Option<String> {
    if pr.state != PullRequestState::Open {
        Some(format!("PR #{} is not open", pr.number))
    } else {
        None
    }
}

fn merge_blocker(pr: &PullRequest) -> Option<String> {
    if pr.state != PullRequestState::Open {
        return Some(format!("PR #{} is not open", pr.number));
    }

    if pr.is_draft {
        return Some(format!("PR #{} is still a draft", pr.number));
    }

    if pr.head_sha.is_empty() {
        return Some(format!("PR #{} is missing a head SHA", pr.number));
    }

    match pr.merge_state {
        Some(MergeState::Clean) => {}
        Some(MergeState::Dirty) => {
            return Some(format!("PR #{} has merge conflicts", pr.number));
        }
        Some(MergeState::Blocked) => {
            return Some(format!("PR #{} is blocked by repository rules", pr.number));
        }
        Some(MergeState::Behind) => {
            return Some(format!("PR #{} is behind the base branch", pr.number));
        }
        Some(MergeState::Unknown) | None => {
            return Some(format!(
                "PR #{} is not confirmed mergeable by GitHub",
                pr.number
            ));
        }
    }

    if pr.checks_summary.failed > 0 {
        return Some(format!("PR #{} still has failing checks", pr.number));
    }

    if pr.checks_summary.pending > 0 {
        return Some(format!("PR #{} still has pending checks", pr.number));
    }

    if pr.unresolved_threads > 0 {
        return Some(format!(
            "PR #{} still has {} unresolved review threads",
            pr.number, pr.unresolved_threads
        ));
    }

    None
}

fn render_review_decision(decision: Option<ReviewDecision>) -> impl IntoElement {
    let label = match decision {
        Some(ReviewDecision::Approved) => "approved",
        Some(ReviewDecision::ChangesRequested) => "changes requested",
        Some(ReviewDecision::ReviewRequired) => "review required",
        None => "no review",
    };

    div().text_xs().text_color(rgb(0x93c5fd)).child(label)
}

fn render_merge_state(state: Option<MergeState>) -> impl IntoElement {
    let label = match state {
        Some(MergeState::Clean) => "mergeable",
        Some(MergeState::Dirty) => "dirty",
        Some(MergeState::Blocked) => "blocked",
        Some(MergeState::Behind) => "behind",
        Some(MergeState::Unknown) | None => "unknown",
    };

    div().text_xs().text_color(rgb(0x9aa4b2)).child(label)
}

fn render_pull_request_row(
    index: usize,
    pr: &PullRequest,
    selected: bool,
    cx: &mut Context<AppView>,
) -> AnyElement {
    div()
        .id(("pr-row", index))
        .h(px(76.))
        .flex()
        .flex_col()
        .justify_center()
        .px_3()
        .py_2()
        .border_1()
        .border_color(rgb(0x20252b))
        .when(selected, |element| element.bg(rgb(0x243244)))
        .hover(|style| style.bg(rgb(0x202a35)))
        .on_click(cx.listener(move |view, _, _, cx| {
            view.select_pull_request(index, cx);
        }))
        .child(
            div()
                .flex()
                .justify_between()
                .items_center()
                .gap_2()
                .text_sm()
                .child(
                    div()
                        .min_w_0()
                        .flex_1()
                        .truncate()
                        .child(format!("#{} {}", pr.number, pr.title)),
                )
                .child(render_checks_summary(pr.checks_summary)),
        )
        .child(
            div()
                .pt_1()
                .text_xs()
                .text_color(rgb(0x9aa4b2))
                .truncate()
                .child(format!(
                    "{} into {} by {}",
                    pr.head_ref, pr.base_ref, pr.author
                )),
        )
        .into_any_element()
}

fn render_changed_file_row(
    index: usize,
    file: &DiffFile,
    selected: bool,
    cx: &mut Context<AppView>,
) -> AnyElement {
    div()
        .id(("file-row", index))
        .h(px(72.))
        .flex()
        .flex_col()
        .justify_center()
        .px_3()
        .py_2()
        .border_1()
        .border_color(rgb(0x20252b))
        .when(selected, |element| element.bg(rgb(0x243244)))
        .hover(|style| style.bg(rgb(0x202a35)))
        .on_click(cx.listener(move |view, _, _, cx| {
            view.select_file(index, cx);
        }))
        .child(
            div()
                .flex()
                .justify_between()
                .items_center()
                .gap_2()
                .text_sm()
                .child(div().min_w_0().flex_1().truncate().child(file.path.clone()))
                .child(
                    div()
                        .flex_none()
                        .child(format!("+{} -{}", file.additions, file.deletions)),
                ),
        )
        .child(
            div()
                .pt_1()
                .text_xs()
                .text_color(rgb(0x9aa4b2))
                .child(format!("{:?}", file.status)),
        )
        .into_any_element()
}

fn render_diff_panel(
    file: Option<&DiffFile>,
    parsed_diff: Option<&ParsedDiff>,
    is_loading: bool,
    error: Option<&str>,
    scroll_handle: UniformListScrollHandle,
    cx: &mut Context<AppView>,
) -> impl IntoElement {
    if is_loading {
        return div()
            .flex()
            .flex_col()
            .flex_1()
            .min_h_0()
            .gap_2()
            .child(
                div()
                    .text_color(rgb(0xf1f5f9))
                    .child("Unified diff preview"),
            )
            .child(
                div()
                    .rounded_sm()
                    .border_1()
                    .border_color(rgb(0x242a31))
                    .bg(rgb(0x0c0f12))
                    .p_3()
                    .text_color(rgb(0x9aa4b2))
                    .child("Loading diff..."),
            )
            .into_any_element();
    }

    if let Some(error) = error {
        return div()
            .flex()
            .flex_col()
            .flex_1()
            .min_h_0()
            .gap_2()
            .child(
                div()
                    .text_color(rgb(0xf1f5f9))
                    .child("Unified diff preview"),
            )
            .child(
                div()
                    .rounded_sm()
                    .border_1()
                    .border_color(rgb(0x242a31))
                    .bg(rgb(0x0c0f12))
                    .p_3()
                    .text_color(rgb(0xf87171))
                    .child(error.to_string()),
            )
            .into_any_element();
    }

    let Some(file) = file else {
        return div()
            .flex()
            .flex_col()
            .flex_1()
            .min_h_0()
            .gap_2()
            .child(
                div()
                    .text_color(rgb(0xf1f5f9))
                    .child("Unified diff preview"),
            )
            .child(
                div()
                    .rounded_sm()
                    .border_1()
                    .border_color(rgb(0x242a31))
                    .bg(rgb(0x0c0f12))
                    .p_3()
                    .text_color(rgb(0x9aa4b2))
                    .child("Select a changed file to preview its diff"),
            )
            .into_any_element();
    };

    let Some(parsed_diff) = parsed_diff else {
        return div()
            .flex()
            .flex_col()
            .flex_1()
            .min_h_0()
            .gap_2()
            .child(render_diff_file_header(file, None))
            .child(
                div()
                    .rounded_sm()
                    .border_1()
                    .border_color(rgb(0x242a31))
                    .bg(rgb(0x0c0f12))
                    .p_3()
                    .text_color(rgb(0xfbbf24))
                    .child(
                        "Diff unavailable via GitHub API. Local checkout fallback will be added.",
                    ),
            )
            .into_any_element();
    };

    let row_count = diff_row_count(parsed_diff);

    div()
        .id("diff-panel")
        .flex()
        .flex_col()
        .flex_1()
        .min_h_0()
        .min_w_0()
        .gap_2()
        .child(render_diff_file_header(file, Some(parsed_diff.hunks.len())))
        .child(
            div()
                .flex()
                .flex_col()
                .flex_1()
                .min_h_0()
                .min_w_0()
                .rounded_sm()
                .border_1()
                .border_color(rgb(0x242a31))
                .bg(rgb(0x0c0f12))
                .overflow_hidden()
                .child(
                    uniform_list(
                        "diff-lines-list",
                        row_count,
                        cx.processor(|view, range: std::ops::Range<usize>, _window, _cx| {
                            let Some(parsed_diff) = view.active_diff() else {
                                return Vec::new();
                            };
                            let mut rows = Vec::with_capacity(range.len());

                            for row_index in range {
                                if let Some(row) =
                                    render_diff_row(parsed_diff, row_index, view.active_hunk)
                                {
                                    rows.push(row);
                                }
                            }

                            rows
                        }),
                    )
                    .with_horizontal_sizing_behavior(ListHorizontalSizingBehavior::Unconstrained)
                    .track_scroll(&scroll_handle)
                    .flex_1()
                    .min_h_0()
                    .min_w_0()
                    .font_family("Menlo")
                    .text_xs(),
                ),
        )
        .into_any_element()
}

fn render_diff_file_header(file: &DiffFile, hunk_count: Option<usize>) -> impl IntoElement {
    let hunk_label = hunk_count.map_or_else(
        || "no parsed hunks".to_string(),
        |count| format!("{count} hunks"),
    );

    div()
        .flex()
        .items_center()
        .justify_between()
        .gap_3()
        .text_color(rgb(0xf1f5f9))
        .child(file.path.clone())
        .child(div().text_xs().text_color(rgb(0x9aa4b2)).child(format!(
            "{:?}  +{} -{}  {}",
            file.status, file.additions, file.deletions, hunk_label
        )))
}

enum DiffRow<'a> {
    Hunk { hunk: &'a DiffHunk, index: usize },
    Line(&'a DiffLine),
}

fn diff_row_count(diff: &ParsedDiff) -> usize {
    diff.hunks.iter().map(|hunk| hunk.lines.len() + 1).sum()
}

fn diff_hunk_row_index(diff: &ParsedDiff, hunk_index: usize) -> Option<usize> {
    let mut row_index = 0;

    for (index, hunk) in diff.hunks.iter().enumerate() {
        if index == hunk_index {
            return Some(row_index);
        }

        row_index += hunk.lines.len() + 1;
    }

    None
}

fn diff_row_at(diff: &ParsedDiff, row_index: usize) -> Option<DiffRow<'_>> {
    let mut cursor = 0;

    for (index, hunk) in diff.hunks.iter().enumerate() {
        if row_index == cursor {
            return Some(DiffRow::Hunk { hunk, index });
        }

        cursor += 1;
        let line_offset = row_index.checked_sub(cursor)?;
        if line_offset < hunk.lines.len() {
            return Some(DiffRow::Line(&hunk.lines[line_offset]));
        }

        cursor += hunk.lines.len();
    }

    None
}

fn render_diff_row(diff: &ParsedDiff, row_index: usize, active_hunk: usize) -> Option<AnyElement> {
    match diff_row_at(diff, row_index)? {
        DiffRow::Hunk { hunk, index } => {
            Some(render_diff_hunk_row(hunk, index, index == active_hunk).into_any_element())
        }
        DiffRow::Line(line) => Some(render_diff_line(line).into_any_element()),
    }
}

fn render_diff_hunk_row(hunk: &DiffHunk, index: usize, active: bool) -> impl IntoElement {
    div()
        .h(px(24.))
        .flex()
        .items_center()
        .gap_2()
        .px_2()
        .border_1()
        .border_color(if active { rgb(0x3b82f6) } else { rgb(0x1a2029) })
        .bg(if active { rgb(0x172033) } else { rgb(0x1a2029) })
        .text_color(rgb(0x93c5fd))
        .whitespace_nowrap()
        .child(format!("hunk {}  {}", index + 1, hunk.header))
}

fn render_diff_line(line: &DiffLine) -> impl IntoElement {
    let (prefix, bg, text_color) = match line.kind {
        DiffLineKind::Context => (" ", rgb(0x0c0f12), rgb(0xcbd5e1)),
        DiffLineKind::Added => ("+", rgb(0x10231a), rgb(0xa7f3d0)),
        DiffLineKind::Removed => ("-", rgb(0x291516), rgb(0xfca5a5)),
        DiffLineKind::Metadata => ("\\", rgb(0x111827), rgb(0x9aa4b2)),
    };

    div()
        .h(px(24.))
        .flex()
        .items_start()
        .bg(bg)
        .text_color(text_color)
        .whitespace_nowrap()
        .child(render_line_number(line.old_line))
        .child(render_line_number(line.new_line))
        .child(
            div()
                .w(px(20.))
                .flex_none()
                .text_color(text_color)
                .child(prefix),
        )
        .child(div().flex_none().child(line.text.clone()))
}

fn render_line_number(line: Option<u32>) -> impl IntoElement {
    div()
        .w(px(52.))
        .flex_none()
        .pr_2()
        .text_right()
        .text_color(rgb(0x64748b))
        .child(line.map_or_else(String::new, |line| line.to_string()))
}

fn render_review_panel(
    reviews: &[PullRequestReview],
    threads: &[ReviewThread],
    is_loading: bool,
    error: Option<&str>,
    scroll_handle: UniformListScrollHandle,
    cx: &mut Context<AppView>,
) -> impl IntoElement {
    let (unresolved, resolved, outdated) = review_thread_counts(threads);

    div()
        .id("review-panel")
        .flex()
        .flex_col()
        .flex_1()
        .min_h_0()
        .gap_2()
        .child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .gap_3()
                .child("Review")
                .child(div().text_xs().text_color(rgb(0x9aa4b2)).child(format!(
                    "{} reviews  {} threads",
                    reviews.len(),
                    threads.len()
                ))),
        )
        .child(
            div()
                .flex()
                .gap_3()
                .text_xs()
                .text_color(rgb(0x9aa4b2))
                .child(format!("unresolved {unresolved}"))
                .child(format!("resolved {resolved}"))
                .child(format!("outdated {outdated}")),
        )
        .when(!reviews.is_empty(), |element| {
            element
                .child(
                    div()
                        .pt_1()
                        .text_xs()
                        .text_color(rgb(0x9aa4b2))
                        .child("latest reviews"),
                )
                .children(reviews.iter().rev().take(3).map(render_pull_request_review))
        })
        .when(is_loading, |element| {
            element.child(
                div()
                    .rounded_sm()
                    .border_1()
                    .border_color(rgb(0x242a31))
                    .bg(rgb(0x0c0f12))
                    .p_3()
                    .text_color(rgb(0x9aa4b2))
                    .child("Loading review threads..."),
            )
        })
        .when_some(error.map(str::to_string), |element, error| {
            element.child(
                div()
                    .rounded_sm()
                    .border_1()
                    .border_color(rgb(0x7f1d1d))
                    .bg(rgb(0x2a1212))
                    .p_3()
                    .text_color(rgb(0xf87171))
                    .child(error),
            )
        })
        .when(
            !is_loading && error.is_none() && threads.is_empty(),
            |element| {
                element.child(
                    div()
                        .rounded_sm()
                        .border_1()
                        .border_color(rgb(0x242a31))
                        .bg(rgb(0x0c0f12))
                        .p_3()
                        .text_color(rgb(0x9aa4b2))
                        .child("No review threads found for this pull request"),
                )
            },
        )
        .when(!threads.is_empty(), |element| {
            element.child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_h_0()
                    .min_w_0()
                    .rounded_sm()
                    .border_1()
                    .border_color(rgb(0x242a31))
                    .bg(rgb(0x0c0f12))
                    .overflow_hidden()
                    .child(
                        uniform_list(
                            "review-thread-list",
                            threads.len(),
                            cx.processor(|view, range: std::ops::Range<usize>, _window, _cx| {
                                let mut rows = Vec::with_capacity(range.len());

                                for index in range {
                                    let Some(thread) = view.review_threads.get(index) else {
                                        continue;
                                    };
                                    rows.push(render_review_thread_row(index, thread));
                                }

                                rows
                            }),
                        )
                        .track_scroll(&scroll_handle)
                        .flex_1()
                        .min_h_0()
                        .min_w_0(),
                    ),
            )
        })
}

fn render_pull_request_review(review: &PullRequestReview) -> impl IntoElement {
    let (label, color) = pull_request_review_state_label(review.state);

    div()
        .flex()
        .items_center()
        .justify_between()
        .gap_3()
        .rounded_sm()
        .border_1()
        .border_color(rgb(0x242a31))
        .bg(rgb(0x0c0f12))
        .px_3()
        .py_2()
        .child(
            div()
                .min_w_0()
                .flex_1()
                .child(format!("{} by {}", label, review.author))
                .when_some(
                    review.body.as_ref().map(|body| single_line(body)),
                    |element, body| {
                        element.child(
                            div()
                                .pt_1()
                                .text_xs()
                                .text_color(rgb(0x9aa4b2))
                                .truncate()
                                .child(body),
                        )
                    },
                ),
        )
        .child(
            div()
                .text_xs()
                .text_color(color)
                .child(review_time_label(review)),
        )
}

fn render_review_thread_row(index: usize, thread: &ReviewThread) -> AnyElement {
    let (label, color) = review_thread_state_label(thread.state);
    let latest_comment = thread.comments.last();
    let location = review_thread_location(thread);
    let preview = latest_comment
        .map(|comment| single_line(&comment.body))
        .unwrap_or_else(|| "No comments in this thread".to_string());

    div()
        .id(("review-thread-row", index))
        .h(px(136.))
        .flex()
        .flex_col()
        .justify_center()
        .gap_2()
        .px_3()
        .py_2()
        .border_1()
        .border_color(rgb(0x20252b))
        .hover(|style| style.bg(rgb(0x202a35)))
        .child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .gap_3()
                .child(
                    div()
                        .min_w_0()
                        .flex_1()
                        .truncate()
                        .child(thread.path.clone()),
                )
                .child(div().text_xs().text_color(color).child(label)),
        )
        .child(div().text_xs().text_color(rgb(0x9aa4b2)).child(format!(
            "{}  {} comments",
            location,
            thread.comments.len()
        )))
        .when_some(latest_comment, |element, comment| {
            element.child(
                div()
                    .text_xs()
                    .text_color(rgb(0x9aa4b2))
                    .truncate()
                    .child(format!("{}: {}", comment.author, preview)),
            )
        })
        .into_any_element()
}

fn review_thread_counts(threads: &[ReviewThread]) -> (usize, usize, usize) {
    let mut unresolved = 0;
    let mut resolved = 0;
    let mut outdated = 0;

    for thread in threads {
        match thread.state {
            ReviewThreadState::Unresolved => unresolved += 1,
            ReviewThreadState::Resolved => resolved += 1,
            ReviewThreadState::Outdated => outdated += 1,
        }
    }

    (unresolved, resolved, outdated)
}

fn review_thread_location(thread: &ReviewThread) -> String {
    thread
        .comments
        .iter()
        .find_map(|comment| comment.position.as_ref())
        .and_then(|position| position.line.or(position.original_line))
        .map_or_else(|| "file".to_string(), |line| format!("line {line}"))
}

fn single_line(value: &str) -> String {
    value
        .lines()
        .find(|line| !line.trim().is_empty())
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .unwrap_or("empty comment")
        .to_string()
}

fn pull_request_review_state_label(state: PullRequestReviewState) -> (&'static str, gpui::Hsla) {
    match state {
        PullRequestReviewState::Pending => ("pending", rgb(0xfbbf24).into()),
        PullRequestReviewState::Commented => ("commented", rgb(0x93c5fd).into()),
        PullRequestReviewState::Approved => ("approved", rgb(0x34d399).into()),
        PullRequestReviewState::ChangesRequested => ("changes requested", rgb(0xf87171).into()),
        PullRequestReviewState::Dismissed => ("dismissed", rgb(0x9aa4b2).into()),
    }
}

fn review_thread_state_label(state: ReviewThreadState) -> (&'static str, gpui::Hsla) {
    match state {
        ReviewThreadState::Unresolved => ("unresolved", rgb(0xfbbf24).into()),
        ReviewThreadState::Resolved => ("resolved", rgb(0x34d399).into()),
        ReviewThreadState::Outdated => ("outdated", rgb(0x9aa4b2).into()),
    }
}

fn review_time_label(review: &PullRequestReview) -> String {
    review
        .submitted_at
        .map(|submitted_at| submitted_at.format("%Y-%m-%d %H:%M").to_string())
        .unwrap_or_else(|| "not submitted".to_string())
}

fn render_checks_panel(
    summary: ChecksSummary,
    check_runs: &[CheckRun],
    is_loading: bool,
    error: Option<&str>,
) -> impl IntoElement {
    div()
        .id("checks-panel-scroll")
        .flex()
        .flex_col()
        .flex_1()
        .min_h_0()
        .overflow_y_scroll()
        .gap_2()
        .child("Checks summary")
        .child(
            div()
                .flex()
                .gap_3()
                .text_xs()
                .text_color(rgb(0x9aa4b2))
                .child(format!("total {}", summary.total))
                .child(format!("passed {}", summary.passed))
                .child(format!("failed {}", summary.failed))
                .child(format!("pending {}", summary.pending))
                .child(format!("skipped {}", summary.skipped)),
        )
        .when(is_loading, |element| {
            element.child(
                div()
                    .rounded_sm()
                    .border_1()
                    .border_color(rgb(0x242a31))
                    .bg(rgb(0x0c0f12))
                    .p_3()
                    .text_color(rgb(0x9aa4b2))
                    .child("Loading check runs..."),
            )
        })
        .when_some(error.map(str::to_string), |element, error| {
            element.child(
                div()
                    .rounded_sm()
                    .border_1()
                    .border_color(rgb(0x242a31))
                    .bg(rgb(0x0c0f12))
                    .p_3()
                    .text_color(rgb(0xf87171))
                    .child(error),
            )
        })
        .when(
            !is_loading && error.is_none() && check_runs.is_empty(),
            |element| {
                element.child(
                    div()
                        .rounded_sm()
                        .border_1()
                        .border_color(rgb(0x242a31))
                        .bg(rgb(0x0c0f12))
                        .p_3()
                        .text_color(rgb(0x9aa4b2))
                        .child("No check runs found for this PR head"),
                )
            },
        )
        .children(check_runs.iter().map(render_check_run))
}

fn render_check_run(check_run: &CheckRun) -> impl IntoElement {
    div()
        .flex()
        .items_center()
        .justify_between()
        .gap_3()
        .rounded_sm()
        .border_1()
        .border_color(rgb(0x242a31))
        .bg(rgb(0x0c0f12))
        .px_3()
        .py_2()
        .child(
            div()
                .flex()
                .flex_col()
                .gap_1()
                .child(check_run.name.clone())
                .child(
                    div()
                        .text_xs()
                        .text_color(rgb(0x9aa4b2))
                        .child(format!("{:?}", check_run.status)),
                ),
        )
        .child(render_check_conclusion(
            check_run.conclusion,
            check_run.status,
        ))
}

fn render_check_conclusion(
    conclusion: Option<CheckConclusion>,
    status: CheckStatus,
) -> impl IntoElement {
    let (label, color) = match (status, conclusion) {
        (CheckStatus::Completed, Some(CheckConclusion::Success)) => ("passed", rgb(0x34d399)),
        (CheckStatus::Completed, Some(CheckConclusion::Skipped)) => ("skipped", rgb(0x9aa4b2)),
        (CheckStatus::Completed, Some(CheckConclusion::Neutral)) => ("neutral", rgb(0x9aa4b2)),
        (CheckStatus::Completed, Some(CheckConclusion::Cancelled)) => ("cancelled", rgb(0xfbbf24)),
        (CheckStatus::Completed, Some(CheckConclusion::TimedOut)) => ("timed out", rgb(0xf87171)),
        (CheckStatus::Completed, Some(CheckConclusion::ActionRequired)) => {
            ("action required", rgb(0xfbbf24))
        }
        (CheckStatus::Completed, Some(CheckConclusion::Failure) | None) => {
            ("failed", rgb(0xf87171))
        }
        (CheckStatus::InProgress, _) => ("running", rgb(0x93c5fd)),
        (CheckStatus::Queued, _) => ("queued", rgb(0xfbbf24)),
    };

    div().text_sm().text_color(color).child(label)
}

fn render_actions_panel(
    pr: Option<&PullRequest>,
    workflow_runs: &[WorkflowRun],
    is_loading: bool,
    error: Option<&str>,
    action_error: Option<&str>,
    is_running_action: bool,
    cx: &mut Context<AppView>,
) -> impl IntoElement {
    let rerun_target = workflow_runs
        .iter()
        .find(|run| workflow_run_failed(run))
        .or_else(|| workflow_runs.first());
    let dispatch_target = workflow_runs.iter().find(|run| run.workflow_id.is_some());
    let can_rerun = !is_loading && error.is_none() && rerun_target.is_some() && !is_running_action;
    let can_dispatch =
        !is_loading && error.is_none() && dispatch_target.is_some() && !is_running_action;

    div()
        .id("actions-panel-scroll")
        .flex()
        .flex_col()
        .flex_1()
        .min_h_0()
        .overflow_y_scroll()
        .gap_2()
        .child("Workflow runs")
        .child(
            div()
                .flex()
                .items_center()
                .gap_2()
                .child(
                    Button::new("trigger-build")
                        .label("trigger build")
                        .small()
                        .outline()
                        .loading(is_running_action)
                        .disabled(!can_dispatch)
                        .on_click(cx.listener(|view, _, _, cx| {
                            view.run_workflow_action(WorkflowAction::DispatchBuild, cx);
                        })),
                )
                .child(
                    Button::new("rerun-failed-jobs")
                        .label("rerun failed")
                        .small()
                        .outline()
                        .loading(is_running_action)
                        .disabled(!can_rerun)
                        .on_click(cx.listener(|view, _, _, cx| {
                            view.run_workflow_action(WorkflowAction::RerunFailedJobs, cx);
                        })),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(rgb(0x9aa4b2))
                        .child("b / shift+r"),
                ),
        )
        .child(div().text_xs().text_color(rgb(0x9aa4b2)).child(format!(
                    "dispatch target: {} on {}",
                    dispatch_target
                        .map(workflow_run_label)
                        .unwrap_or_else(|| "none".to_string()),
                    pr.map(|pr| pr.head_ref.as_str()).unwrap_or("no selected branch")
                )))
        .child(div().text_xs().text_color(rgb(0x9aa4b2)).child(format!(
                    "rerun target: {}",
                    rerun_target
                        .map(workflow_run_label)
                        .unwrap_or_else(|| "none".to_string())
                )))
        .when_some(action_error.map(str::to_string), |element, error| {
            element.child(
                div()
                    .rounded_sm()
                    .border_1()
                    .border_color(rgb(0x7f1d1d))
                    .bg(rgb(0x2a1212))
                    .p_3()
                    .text_color(rgb(0xf87171))
                    .child(error),
            )
        })
        .when(is_loading, |element| {
            element.child(
                div()
                    .rounded_sm()
                    .border_1()
                    .border_color(rgb(0x242a31))
                    .bg(rgb(0x0c0f12))
                    .p_3()
                    .text_color(rgb(0x9aa4b2))
                    .child("Loading workflow runs..."),
            )
        })
        .when_some(error.map(str::to_string), |element, error| {
            element.child(
                div()
                    .rounded_sm()
                    .border_1()
                    .border_color(rgb(0x242a31))
                    .bg(rgb(0x0c0f12))
                    .p_3()
                    .text_color(rgb(0xf87171))
                    .child(error),
            )
        })
        .when(
            !is_loading && error.is_none() && workflow_runs.is_empty(),
            |element| {
                element.child(
                    div()
                        .rounded_sm()
                        .border_1()
                        .border_color(rgb(0x242a31))
                        .bg(rgb(0x0c0f12))
                        .p_3()
                        .text_color(rgb(0x9aa4b2))
                        .child("No workflow runs found for this PR head"),
                )
            },
        )
        .children(workflow_runs.iter().map(render_workflow_run))
}

fn workflow_run_label(run: &WorkflowRun) -> String {
    run.workflow_name
        .as_deref()
        .unwrap_or(run.name.as_str())
        .to_string()
}

fn workflow_run_failed(run: &WorkflowRun) -> bool {
    matches!(
        (run.status, run.conclusion),
        (
            WorkflowStatus::Completed,
            Some(
                WorkflowConclusion::Failure
                    | WorkflowConclusion::Cancelled
                    | WorkflowConclusion::TimedOut
                    | WorkflowConclusion::ActionRequired
            )
        )
    )
}

fn render_workflow_run(run: &WorkflowRun) -> impl IntoElement {
    div()
        .flex()
        .items_center()
        .justify_between()
        .gap_3()
        .rounded_sm()
        .border_1()
        .border_color(rgb(0x242a31))
        .bg(rgb(0x0c0f12))
        .px_3()
        .py_2()
        .child(
            div()
                .flex()
                .flex_col()
                .gap_1()
                .child(run.name.clone())
                .child(div().text_xs().text_color(rgb(0x9aa4b2)).child(format!(
                    "{}  {}  {}",
                    run.workflow_name.as_deref().unwrap_or("workflow"),
                    run.event,
                    run.head_branch
                ))),
        )
        .child(render_workflow_conclusion(run.conclusion, run.status))
}

fn render_workflow_conclusion(
    conclusion: Option<WorkflowConclusion>,
    status: WorkflowStatus,
) -> impl IntoElement {
    let (label, color) = workflow_conclusion_label(conclusion, status);

    div().text_sm().text_color(color).child(label)
}

fn workflow_conclusion_label(
    conclusion: Option<WorkflowConclusion>,
    status: WorkflowStatus,
) -> (&'static str, gpui::Hsla) {
    let (label, color) = match (status, conclusion) {
        (WorkflowStatus::Completed, Some(WorkflowConclusion::Success)) => ("passed", rgb(0x34d399)),
        (WorkflowStatus::Completed, Some(WorkflowConclusion::Skipped)) => {
            ("skipped", rgb(0x9aa4b2))
        }
        (WorkflowStatus::Completed, Some(WorkflowConclusion::Cancelled)) => {
            ("cancelled", rgb(0xfbbf24))
        }
        (WorkflowStatus::Completed, Some(WorkflowConclusion::TimedOut)) => {
            ("timed out", rgb(0xf87171))
        }
        (WorkflowStatus::Completed, Some(WorkflowConclusion::ActionRequired)) => {
            ("action required", rgb(0xfbbf24))
        }
        (WorkflowStatus::Completed, Some(WorkflowConclusion::Failure) | None) => {
            ("failed", rgb(0xf87171))
        }
        (WorkflowStatus::InProgress, _) => ("running", rgb(0x93c5fd)),
        (WorkflowStatus::Queued, _) => ("queued", rgb(0xfbbf24)),
    };

    (label, color.into())
}

fn render_logs_panel(
    run: Option<&WorkflowRun>,
    jobs: &[WorkflowJob],
    log_chunk: Option<&LogChunk>,
    is_loading: bool,
    error: Option<&str>,
    scroll_handle: UniformListScrollHandle,
    cx: &mut Context<AppView>,
) -> impl IntoElement {
    let line_count = log_chunk.map_or(0, |chunk| chunk.lines.len());

    div()
        .id("logs-panel")
        .flex()
        .flex_col()
        .flex_1()
        .min_h_0()
        .gap_2()
        .child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .gap_3()
                .child("Logs")
                .child(
                    Button::new("load-workflow-logs")
                        .label("load logs")
                        .small()
                        .outline()
                        .loading(is_loading)
                        .disabled(run.is_none() || is_loading)
                        .on_click(cx.listener(|view, _, _, cx| {
                            view.load_selected_workflow_logs(cx);
                        })),
                ),
        )
        .child(div().text_xs().text_color(rgb(0x9aa4b2)).child(format!(
                    "target: {}",
                    run.map(workflow_run_label)
                        .unwrap_or_else(|| "no workflow run".to_string())
                )))
        .when(is_loading, |element| {
            element.child(
                div()
                    .rounded_sm()
                    .border_1()
                    .border_color(rgb(0x242a31))
                    .bg(rgb(0x0c0f12))
                    .p_3()
                    .text_color(rgb(0x9aa4b2))
                    .child("Loading workflow jobs and logs..."),
            )
        })
        .when_some(error.map(str::to_string), |element, error| {
            element.child(
                div()
                    .rounded_sm()
                    .border_1()
                    .border_color(rgb(0x7f1d1d))
                    .bg(rgb(0x2a1212))
                    .p_3()
                    .text_color(rgb(0xf87171))
                    .child(error),
            )
        })
        .when(!is_loading && run.is_none(), |element| {
            element.child(
                div()
                    .rounded_sm()
                    .border_1()
                    .border_color(rgb(0x242a31))
                    .bg(rgb(0x0c0f12))
                    .p_3()
                    .text_color(rgb(0x9aa4b2))
                    .child("No workflow run found for this PR head"),
            )
        })
        .when(!jobs.is_empty(), |element| {
            element
                .child(
                    div()
                        .pt_1()
                        .text_xs()
                        .text_color(rgb(0x9aa4b2))
                        .child(format!("jobs {}", jobs.len())),
                )
                .children(jobs.iter().map(render_workflow_job))
        })
        .when(line_count > 0, |element| {
            element.child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_h_0()
                    .min_w_0()
                    .rounded_sm()
                    .border_1()
                    .border_color(rgb(0x242a31))
                    .bg(rgb(0x0c0f12))
                    .overflow_hidden()
                    .child(
                        uniform_list(
                            "workflow-log-lines",
                            line_count,
                            cx.processor(|view, range: std::ops::Range<usize>, _window, _cx| {
                                let Some(chunk) = view.log_chunk.as_ref() else {
                                    return Vec::new();
                                };
                                let mut rows = Vec::with_capacity(range.len());

                                for index in range {
                                    let Some(line) = chunk.lines.get(index) else {
                                        continue;
                                    };
                                    rows.push(render_log_line(line));
                                }

                                rows
                            }),
                        )
                        .track_scroll(&scroll_handle)
                        .with_horizontal_sizing_behavior(
                            ListHorizontalSizingBehavior::Unconstrained,
                        )
                        .flex_1()
                        .min_h_0()
                        .min_w_0()
                        .font_family("Menlo")
                        .text_xs(),
                    ),
            )
        })
        .when(
            !is_loading && run.is_some() && error.is_none() && log_chunk.is_none(),
            |element| {
                element.child(
                    div()
                        .rounded_sm()
                        .border_1()
                        .border_color(rgb(0x242a31))
                        .bg(rgb(0x0c0f12))
                        .p_3()
                        .text_color(rgb(0x9aa4b2))
                        .child("Press l or load logs to fetch the workflow log output"),
                )
            },
        )
}

fn render_workflow_job(job: &WorkflowJob) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .gap_1()
        .rounded_sm()
        .border_1()
        .border_color(rgb(0x242a31))
        .bg(rgb(0x0c0f12))
        .px_3()
        .py_2()
        .child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .gap_3()
                .child(job.name.clone())
                .child(render_workflow_conclusion(job.conclusion, job.status)),
        )
        .children(job.steps.iter().map(render_workflow_step))
}

fn render_workflow_step(step: &WorkflowStep) -> impl IntoElement {
    let (label, color) = workflow_conclusion_label(step.conclusion, step.status);

    div()
        .flex()
        .items_center()
        .justify_between()
        .gap_3()
        .pl_3()
        .text_xs()
        .text_color(rgb(0x9aa4b2))
        .child(format!("{}. {}", step.number, step.name))
        .child(div().text_color(color).child(label))
}

fn render_log_line(line: &LogLine) -> AnyElement {
    let color = match line.severity {
        LogSeverity::Trace => rgb(0x64748b),
        LogSeverity::Info => rgb(0xcbd5e1),
        LogSeverity::Warning => rgb(0xfbbf24),
        LogSeverity::Error => rgb(0xf87171),
    };

    div()
        .h(px(22.))
        .flex()
        .items_center()
        .whitespace_nowrap()
        .text_color(color)
        .child(
            div()
                .w(px(64.))
                .flex_none()
                .pr_3()
                .text_right()
                .text_color(rgb(0x64748b))
                .child(line.number.to_string()),
        )
        .child(div().flex_none().child(line.text.clone()))
        .into_any_element()
}

fn fake_pull_requests() -> Vec<PullRequest> {
    let repo = RepoId::new("sixt", "mobile-app");

    vec![
        PullRequest {
            repo: repo.clone(),
            number: 1842,
            title: "speed up pull request inbox refresh".to_string(),
            body: Some("Cache first, refresh in the background.".to_string()),
            author: "alex".to_string(),
            url: "https://github.com/sixt/mobile-app/pull/1842".to_string(),
            state: PullRequestState::Open,
            is_draft: false,
            head_ref: "feature/pr-cache".to_string(),
            base_ref: "main".to_string(),
            head_sha: "a1b2c3d".to_string(),
            review_decision: Some(ReviewDecision::ReviewRequired),
            merge_state: Some(MergeState::Clean),
            labels: vec![Label {
                name: "performance".to_string(),
                color: Some("34d399".to_string()),
            }],
            checks_summary: ChecksSummary {
                total: 18,
                passed: 16,
                failed: 0,
                pending: 2,
                skipped: 0,
            },
            unresolved_threads: 3,
        },
        PullRequest {
            repo: repo.clone(),
            number: 1837,
            title: "render failed action steps inline".to_string(),
            body: None,
            author: "maria".to_string(),
            url: "https://github.com/sixt/mobile-app/pull/1837".to_string(),
            state: PullRequestState::Open,
            is_draft: false,
            head_ref: "ci/failed-step-focus".to_string(),
            base_ref: "main".to_string(),
            head_sha: "d4e5f6a".to_string(),
            review_decision: Some(ReviewDecision::ChangesRequested),
            merge_state: Some(MergeState::Blocked),
            labels: vec![Label {
                name: "ci".to_string(),
                color: Some("fbbf24".to_string()),
            }],
            checks_summary: ChecksSummary {
                total: 21,
                passed: 18,
                failed: 2,
                pending: 1,
                skipped: 0,
            },
            unresolved_threads: 7,
        },
        PullRequest {
            repo,
            number: 1829,
            title: "add review thread domain model".to_string(),
            body: None,
            author: "kaan".to_string(),
            url: "https://github.com/sixt/mobile-app/pull/1829".to_string(),
            state: PullRequestState::Open,
            is_draft: true,
            head_ref: "review/thread-model".to_string(),
            base_ref: "main".to_string(),
            head_sha: "f7a8b9c".to_string(),
            review_decision: None,
            merge_state: Some(MergeState::Unknown),
            labels: vec![Label {
                name: "review".to_string(),
                color: Some("93c5fd".to_string()),
            }],
            checks_summary: ChecksSummary {
                total: 17,
                passed: 17,
                failed: 0,
                pending: 0,
                skipped: 0,
            },
            unresolved_threads: 0,
        },
    ]
}

fn configured_repo_from_env() -> Option<RepoId> {
    std::env::var("HARBOR_REPO")
        .ok()
        .or_else(|| std::env::var("GH_REPO").ok())
        .and_then(|value| parse_repo_id(&value))
}

fn parse_repo_id(value: &str) -> Option<RepoId> {
    let (owner, name) = value.split_once('/')?;

    if owner.is_empty() || name.is_empty() || name.contains('/') {
        None
    } else {
        Some(RepoId::new(owner, name))
    }
}

fn fake_files() -> Vec<DiffFile> {
    vec![
        DiffFile {
            path: "crates/ui/src/inbox.rs".to_string(),
            previous_path: None,
            status: FileStatus::Modified,
            additions: 42,
            deletions: 11,
            changes: 53,
            patch: Some(
                "@@ -14,6 +14,13 @@\n pub struct InboxState {\n+    selected: usize,\n+    visible_rows: Range<usize>,\n }\n+\n+impl InboxState {\n+    pub fn move_selection(&mut self, delta: i32) { /* fake diff */ }\n+}\n"
                    .to_string(),
            ),
        },
        DiffFile {
            path: "crates/github/src/transport.rs".to_string(),
            previous_path: None,
            status: FileStatus::Added,
            additions: 88,
            deletions: 0,
            changes: 88,
            patch: None,
        },
        DiffFile {
            path: "crates/logs/src/parser.rs".to_string(),
            previous_path: None,
            status: FileStatus::Modified,
            additions: 65,
            deletions: 22,
            changes: 87,
            patch: None,
        },
    ]
}

fn fake_pull_request_reviews() -> Vec<PullRequestReview> {
    vec![
        PullRequestReview {
            id: "review-1".to_string(),
            author: "maria".to_string(),
            state: PullRequestReviewState::ChangesRequested,
            body: Some("A couple of small issues before this is ready.".to_string()),
            submitted_at: Some(fake_time("2026-05-01T10:00:00Z")),
        },
        PullRequestReview {
            id: "review-2".to_string(),
            author: "alex".to_string(),
            state: PullRequestReviewState::Commented,
            body: Some("The scroll path feels much better now.".to_string()),
            submitted_at: Some(fake_time("2026-05-01T11:30:00Z")),
        },
    ]
}

fn fake_review_threads() -> Vec<ReviewThread> {
    vec![
        ReviewThread {
            id: "thread-1".to_string(),
            path: "crates/ui/src/inbox.rs".to_string(),
            state: ReviewThreadState::Unresolved,
            comments: vec![
                ReviewComment {
                    id: "comment-1".to_string(),
                    author: "maria".to_string(),
                    body: "This row update still looks broader than it needs to be.".to_string(),
                    created_at: fake_time("2026-05-01T10:00:00Z"),
                    updated_at: None,
                    position: Some(ReviewCommentPosition {
                        path: "crates/ui/src/inbox.rs".to_string(),
                        line: Some(42),
                        original_line: Some(39),
                        side: ReviewSide::Right,
                    }),
                },
                ReviewComment {
                    id: "comment-2".to_string(),
                    author: "kaan".to_string(),
                    body: "I will narrow the state update to the selected row.".to_string(),
                    created_at: fake_time("2026-05-01T10:20:00Z"),
                    updated_at: None,
                    position: Some(ReviewCommentPosition {
                        path: "crates/ui/src/inbox.rs".to_string(),
                        line: Some(42),
                        original_line: Some(39),
                        side: ReviewSide::Right,
                    }),
                },
            ],
        },
        ReviewThread {
            id: "thread-2".to_string(),
            path: "crates/logs/src/parser.rs".to_string(),
            state: ReviewThreadState::Resolved,
            comments: vec![ReviewComment {
                id: "comment-3".to_string(),
                author: "alex".to_string(),
                body: "Resolved after the parser stopped cloning full log lines.".to_string(),
                created_at: fake_time("2026-05-01T11:00:00Z"),
                updated_at: None,
                position: Some(ReviewCommentPosition {
                    path: "crates/logs/src/parser.rs".to_string(),
                    line: Some(17),
                    original_line: Some(17),
                    side: ReviewSide::Right,
                }),
            }],
        },
    ]
}

fn fake_time(value: &str) -> chrono::DateTime<chrono::Utc> {
    chrono::DateTime::parse_from_rfc3339(value)
        .map(|datetime| datetime.with_timezone(&chrono::Utc))
        .unwrap_or_else(|_| chrono::Utc::now())
}

#[cfg(test)]
mod tests {
    use harbor_domain::{
        CheckConclusion, CheckRun, CheckStatus, ChecksSummary, MergeState, PullRequest,
        PullRequestState, RepoId, ReviewThread, ReviewThreadState,
    };

    use super::{
        checks_summary_from_runs, merge_blocker, parse_repo_id, review_action_blocker,
        review_thread_counts,
    };

    #[test]
    fn parses_owner_and_repo() {
        let repo = parse_repo_id("acme/app").unwrap();

        assert_eq!(repo.owner, "acme");
        assert_eq!(repo.name, "app");
    }

    #[test]
    fn rejects_invalid_repo_values() {
        assert!(parse_repo_id("").is_none());
        assert!(parse_repo_id("acme").is_none());
        assert!(parse_repo_id("/app").is_none());
        assert!(parse_repo_id("acme/").is_none());
        assert!(parse_repo_id("acme/app/extra").is_none());
    }

    #[test]
    fn summarizes_check_runs() {
        let check_runs = vec![
            check_run(CheckStatus::Completed, Some(CheckConclusion::Success)),
            check_run(CheckStatus::Completed, Some(CheckConclusion::Failure)),
            check_run(CheckStatus::Completed, Some(CheckConclusion::Skipped)),
            check_run(CheckStatus::InProgress, None),
        ];

        let summary = checks_summary_from_runs(&check_runs);

        assert_eq!(summary.total, 4);
        assert_eq!(summary.passed, 1);
        assert_eq!(summary.failed, 1);
        assert_eq!(summary.skipped, 1);
        assert_eq!(summary.pending, 1);
    }

    #[test]
    fn allows_review_actions_for_open_pull_requests() {
        assert_eq!(review_action_blocker(&pull_request()), None);
    }

    #[test]
    fn blocks_merge_until_pull_request_is_ready() {
        let mut pr = pull_request();
        pr.checks_summary.pending = 1;

        assert_eq!(
            merge_blocker(&pr).as_deref(),
            Some("PR #7 still has pending checks")
        );

        pr.checks_summary.pending = 0;
        pr.unresolved_threads = 2;

        assert_eq!(
            merge_blocker(&pr).as_deref(),
            Some("PR #7 still has 2 unresolved review threads")
        );
    }

    #[test]
    fn allows_clean_pull_request_merge() {
        assert_eq!(merge_blocker(&pull_request()), None);
    }

    #[test]
    fn counts_review_threads_by_state() {
        let threads = vec![
            review_thread(ReviewThreadState::Unresolved),
            review_thread(ReviewThreadState::Resolved),
            review_thread(ReviewThreadState::Outdated),
            review_thread(ReviewThreadState::Unresolved),
        ];

        assert_eq!(review_thread_counts(&threads), (2, 1, 1));
    }

    fn check_run(status: CheckStatus, conclusion: Option<CheckConclusion>) -> CheckRun {
        CheckRun {
            id: None,
            name: "check".to_string(),
            status,
            conclusion,
            details_url: None,
            html_url: None,
            started_at: None,
            completed_at: None,
        }
    }

    fn pull_request() -> PullRequest {
        PullRequest {
            repo: RepoId::new("acme", "app"),
            number: 7,
            title: "Add feature".to_string(),
            body: None,
            author: "octocat".to_string(),
            url: "https://github.com/acme/app/pull/7".to_string(),
            state: PullRequestState::Open,
            is_draft: false,
            head_ref: "feature".to_string(),
            base_ref: "main".to_string(),
            head_sha: "abc123".to_string(),
            review_decision: None,
            merge_state: Some(MergeState::Clean),
            labels: Vec::new(),
            checks_summary: ChecksSummary {
                total: 1,
                passed: 1,
                failed: 0,
                pending: 0,
                skipped: 0,
            },
            unresolved_threads: 0,
        }
    }

    fn review_thread(state: ReviewThreadState) -> ReviewThread {
        ReviewThread {
            id: "thread".to_string(),
            path: "src/app.rs".to_string(),
            state,
            comments: Vec::new(),
        }
    }
}
