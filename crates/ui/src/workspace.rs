mod commands;
mod loaders;
mod render;

use gpui::{
    App, AppContext, Context, Entity, FocusHandle, ScrollStrategy, Subscription, Task,
    UniformListScrollHandle, Window,
};
use gpui_component::input::{InputEvent, InputState};
use harbor_domain::{
    CheckRun, DiffFile, PullRequest, PullRequestReview, PullRequestReviewState, RepoId,
    ReviewCommentRange, ReviewSide, ReviewThread, WorkflowJob, WorkflowRun,
};
use harbor_github::{GhCliTransport, GitHubClient, SubmitPullRequestReviewEvent};
use harbor_logs::LogChunk;
use harbor_storage::SqliteStore;

use crate::actions::{DEFAULT_REQUEST_CHANGES_BODY, PanelTab};
use crate::diff::{ParsedDiff, parse_files};
use crate::panels::workflow_run_failed;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ReviewLineTarget {
    pub(crate) hunk_index: usize,
    pub(crate) line_index: usize,
    pub(crate) range: ReviewCommentRange,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ReviewComposer {
    pub(crate) anchor: ReviewLineTarget,
    pub(crate) range: ReviewCommentRange,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ReviewLineSelection {
    pub(crate) anchor: ReviewLineTarget,
    pub(crate) current: ReviewLineTarget,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PendingReviewSession {
    pub(crate) node_id: String,
    pub(crate) comment_count: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ReviewCommentSubmission {
    SingleComment,
    StartReview,
    AddToReview,
}

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
    pub(crate) review_composer: Option<ReviewComposer>,
    pub(crate) review_line_selection: Option<ReviewLineSelection>,
    pub(crate) pending_review: Option<PendingReviewSession>,
    pub(crate) review_comment_input: Entity<InputState>,
    pub(crate) pending_review_body_input: Entity<InputState>,
    pub(crate) log_chunk: Option<LogChunk>,
    pr_list_task: Option<Task<()>>,
    pr_detail_tasks: Vec<Task<()>>,
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
    repository_switcher_selection: usize,
    pull_request_switcher_selection: usize,
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
    pub(crate) is_submitting_review_comment: bool,
    pub(crate) is_submitting_pending_review: bool,
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
    pub(crate) review_comment_error: Option<String>,
    pub(crate) pending_review_error: Option<String>,
    current_user_login: Option<String>,
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
        let review_comment_input = cx.new(|cx| {
            InputState::new(window, cx)
                .auto_grow(3, 8)
                .placeholder("Leave a comment")
                .clean_on_escape()
        });
        let pending_review_body_input = cx.new(|cx| {
            InputState::new(window, cx)
                .auto_grow(2, 6)
                .placeholder("Review summary")
                .clean_on_escape()
        });
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
            cx.subscribe_in(&review_comment_input, window, Self::on_review_input_event),
            cx.subscribe_in(
                &pending_review_body_input,
                window,
                Self::on_review_input_event,
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
            review_composer: None,
            review_line_selection: None,
            pending_review: None,
            review_comment_input,
            pending_review_body_input,
            log_chunk: None,
            pr_list_task: None,
            pr_detail_tasks: Vec::new(),
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
            repository_switcher_selection: 0,
            pull_request_switcher_selection: 0,
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
            is_submitting_review_comment: false,
            is_submitting_pending_review: false,
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
            review_comment_error: None,
            pending_review_error: None,
            current_user_login: None,
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
        self.clear_review_composer_state();
        self.pending_review = None;
        self.reviews_error = None;
        self.logs_error = None;
        self.pr_action_error = None;
        self.review_comment_error = None;
        self.pending_review_error = None;
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
            let path = file.path.clone();
            self.active_file = index;
            self.active_hunk = 0;
            self.active_tab = PanelTab::Diff;
            self.clear_review_composer_state();
            self.file_list_scroll
                .scroll_to_item(index, ScrollStrategy::Center);
            self.diff_list_scroll.scroll_to_item(0, ScrollStrategy::Top);
            self.status = format!("Selected {path}");
        }

        cx.notify();
    }

    pub(crate) fn start_review_line_selection(
        &mut self,
        target: ReviewLineTarget,
        cx: &mut Context<Self>,
    ) {
        self.review_line_selection = Some(ReviewLineSelection {
            anchor: target.clone(),
            current: target,
        });
        self.review_composer = None;
        self.review_comment_error = None;
        self.active_tab = PanelTab::Diff;
        self.status = "Started review line selection".to_string();
        cx.notify();
    }

    pub(crate) fn extend_review_line_selection(
        &mut self,
        target: ReviewLineTarget,
        cx: &mut Context<Self>,
    ) {
        if let Some(selection) = self.review_line_selection.as_mut() {
            selection.current = target;
        }
        cx.notify();
    }

    pub(crate) fn finish_review_line_selection(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(selection) = self.review_line_selection.take() else {
            return;
        };

        match review_composer_from_selection(&selection.anchor, &selection.current) {
            Ok(composer) => {
                let range = composer.range.clone();
                let label = review_comment_range_label(&range);
                self.review_comment_input.update(cx, |input, cx| {
                    input.set_value("", window, cx);
                    input.focus(window, cx);
                });
                self.review_composer = Some(composer);
                self.review_comment_error = None;
                self.status = format!("Opened review composer for {label}");
            }
            Err(message) => {
                self.review_composer = None;
                self.review_comment_error = Some(message.clone());
                self.status = message;
            }
        }

        cx.notify();
    }

    pub(crate) fn cancel_review_composer(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.clear_review_composer_state();
        self.review_comment_input.update(cx, |input, cx| {
            input.set_value("", window, cx);
        });
        self.status = "Cancelled review comment".to_string();
        cx.notify();
    }

    pub(crate) fn remember_repository(&mut self, repository: RepoId) {
        self.repositories.retain(|existing| existing != &repository);
        self.repositories.insert(0, repository);
    }

    pub(crate) fn current_repository(&self) -> Option<&RepoId> {
        self.selected_pull_request()
            .map(|pull_request| &pull_request.repo)
            .or(self.configured_repo.as_ref())
    }

    pub(crate) fn switcher_repositories(&self) -> Vec<RepoId> {
        let mut repositories = self.repositories.clone();

        if let Some(repository) = self.configured_repo.clone() {
            if !repositories.iter().any(|existing| existing == &repository) {
                repositories.push(repository);
            }
        }

        for pull_request in &self.pull_requests {
            if !repositories
                .iter()
                .any(|repository| repository == &pull_request.repo)
            {
                repositories.push(pull_request.repo.clone());
            }
        }

        repositories
    }

    pub(crate) fn filtered_switcher_repositories(&self, cx: &App) -> Vec<RepoId> {
        let query = normalized_search_query(&self.repository_search_input.read(cx).value());

        self.switcher_repositories()
            .into_iter()
            .filter(|repository| repository_matches_query(repository, &query))
            .collect()
    }

    pub(crate) fn filtered_switcher_pull_requests(&self, cx: &App) -> Vec<(usize, PullRequest)> {
        let query = normalized_search_query(&self.pull_request_search_input.read(cx).value());

        self.current_repository()
            .map(|repository| {
                self.pull_requests
                    .iter()
                    .enumerate()
                    .filter(|(_, pull_request)| &pull_request.repo == repository)
                    .filter(|(_, pull_request)| pull_request_matches_query(pull_request, &query))
                    .map(|(index, pull_request)| (index, pull_request.clone()))
                    .collect()
            })
            .unwrap_or_default()
    }

    pub(crate) fn reset_repository_switcher_selection(&mut self, cx: &App) {
        let current_repository = self.current_repository().cloned();
        let repositories = self.filtered_switcher_repositories(cx);
        self.repository_switcher_selection = current_repository
            .and_then(|current| {
                repositories
                    .iter()
                    .position(|repository| *repository == current)
            })
            .unwrap_or(0);
    }

    pub(crate) fn reset_pull_request_switcher_selection(&mut self, cx: &App) {
        let pull_requests = self.filtered_switcher_pull_requests(cx);
        self.pull_request_switcher_selection = pull_requests
            .iter()
            .position(|(index, _)| *index == self.selected_pr)
            .unwrap_or(0);
    }

    pub(crate) fn move_repository_switcher_selection(
        &mut self,
        delta: isize,
        cx: &mut Context<Self>,
    ) {
        let len = self.filtered_switcher_repositories(cx).len();
        self.repository_switcher_selection =
            next_switcher_index(self.repository_switcher_selection, len, delta);
        cx.notify();
    }

    pub(crate) fn move_pull_request_switcher_selection(
        &mut self,
        delta: isize,
        cx: &mut Context<Self>,
    ) {
        let len = self.filtered_switcher_pull_requests(cx).len();
        self.pull_request_switcher_selection =
            next_switcher_index(self.pull_request_switcher_selection, len, delta);
        cx.notify();
    }

    pub(crate) fn accept_repository_switcher_selection(&mut self, cx: &mut Context<Self>) {
        let repositories = self.filtered_switcher_repositories(cx);
        let Some(repository) = repositories
            .get(
                self.repository_switcher_selection
                    .min(repositories.len().saturating_sub(1)),
            )
            .cloned()
        else {
            self.status = "No repositories match search".to_string();
            cx.notify();
            return;
        };

        self.select_repository_from_switcher(repository, cx);
        self.repository_switcher_open = false;
        cx.notify();
    }

    pub(crate) fn accept_pull_request_switcher_selection(&mut self, cx: &mut Context<Self>) {
        let pull_requests = self.filtered_switcher_pull_requests(cx);
        let Some((index, _)) = pull_requests
            .get(
                self.pull_request_switcher_selection
                    .min(pull_requests.len().saturating_sub(1)),
            )
            .cloned()
        else {
            self.status = "No pull requests match search".to_string();
            cx.notify();
            return;
        };

        self.select_pull_request(index, cx);
        self.pull_request_switcher_open = false;
        cx.notify();
    }

    pub(crate) fn clear_review_composer_state(&mut self) {
        self.review_composer = None;
        self.review_line_selection = None;
        self.review_comment_error = None;
    }

    pub(crate) fn apply_loaded_review_data(
        &mut self,
        reviews: Vec<PullRequestReview>,
        review_threads: Vec<ReviewThread>,
        current_user_login: Option<String>,
    ) -> usize {
        let unresolved_count = review_threads
            .iter()
            .filter(|thread| thread.state == harbor_domain::ReviewThreadState::Unresolved)
            .count();
        let existing_pending_review = self.pending_review.clone();
        self.current_user_login = current_user_login;
        self.pending_review = pending_review_from_reviews(
            &reviews,
            self.current_user_login.as_deref(),
            existing_pending_review.as_ref(),
        );
        self.pull_request_reviews = reviews;
        self.review_threads = review_threads;

        if let Some(selected) = self.pull_requests.get_mut(self.selected_pr) {
            selected.unresolved_threads = unresolved_count;
        }

        unresolved_count
    }

    pub(crate) fn submit_review_comment(
        &mut self,
        submission: ReviewCommentSubmission,
        cx: &mut Context<Self>,
    ) {
        if self.is_submitting_review_comment {
            self.status = "A review comment is already being submitted".to_string();
            cx.notify();
            return;
        }

        let Some(composer) = self.review_composer.clone() else {
            self.review_comment_error = Some("Select diff lines before commenting".to_string());
            self.status = "Select diff lines before commenting".to_string();
            cx.notify();
            return;
        };
        let Some(pr) = self.selected_pull_request().cloned() else {
            self.review_comment_error = Some("Select a pull request before commenting".to_string());
            self.status = "Select a pull request before commenting".to_string();
            cx.notify();
            return;
        };

        let body = self.review_comment_input.read(cx).value().to_string();
        let body = body.trim().to_string();
        if body.is_empty() {
            self.review_comment_error = Some("Enter a comment before sending".to_string());
            self.status = "Enter a comment before sending".to_string();
            cx.notify();
            return;
        }

        let pending_review_node_id = match submission {
            ReviewCommentSubmission::AddToReview => {
                let Some(pending_review) = self.pending_review.clone() else {
                    self.review_comment_error =
                        Some("Start a review before adding a review comment".to_string());
                    self.status = "Start a review before adding a review comment".to_string();
                    cx.notify();
                    return;
                };
                Some(pending_review.node_id)
            }
            ReviewCommentSubmission::SingleComment | ReviewCommentSubmission::StartReview => None,
        };

        if submission == ReviewCommentSubmission::StartReview && pr.node_id.is_empty() {
            self.review_comment_error =
                Some("GitHub did not return a pull request node id".to_string());
            self.status = "Cannot start review without a pull request node id".to_string();
            cx.notify();
            return;
        }

        self.is_submitting_review_comment = true;
        self.review_comment_error = None;
        self.status = match submission {
            ReviewCommentSubmission::SingleComment => {
                format!("Posting comment on PR #{}", pr.number)
            }
            ReviewCommentSubmission::StartReview => {
                format!("Starting pending review on PR #{}", pr.number)
            }
            ReviewCommentSubmission::AddToReview => {
                format!("Adding comment to pending review on PR #{}", pr.number)
            }
        };
        cx.notify();

        cx.spawn(async move |this, cx| {
            let client = GitHubClient::new(GhCliTransport);
            let result = match submission {
                ReviewCommentSubmission::SingleComment => client
                    .create_pull_request_review_comment(
                        &pr.repo.owner,
                        &pr.repo.name,
                        pr.number,
                        &pr.head_sha,
                        &composer.range,
                        &body,
                    )
                    .await
                    .map(|()| None),
                ReviewCommentSubmission::StartReview => client
                    .start_pull_request_review(&pr.node_id, &pr.head_sha, &composer.range, &body)
                    .await
                    .map(Some),
                ReviewCommentSubmission::AddToReview => {
                    if let Some(pending_review_node_id) = pending_review_node_id {
                        client
                            .add_pending_review_thread(
                                &pending_review_node_id,
                                &composer.range,
                                &body,
                            )
                            .await
                            .map(|()| None)
                    } else {
                        Err(harbor_github::GitHubError::Transport(
                            "missing pending review id".to_string(),
                        ))
                    }
                }
            };

            if let Err(error) = this.update(cx, move |view, cx| {
                view.is_submitting_review_comment = false;

                match result {
                    Ok(new_pending_review_node_id) => {
                        match submission {
                            ReviewCommentSubmission::SingleComment => {}
                            ReviewCommentSubmission::StartReview => {
                                if let Some(node_id) = new_pending_review_node_id {
                                    view.pending_review = Some(PendingReviewSession {
                                        node_id,
                                        comment_count: 1,
                                    });
                                }
                            }
                            ReviewCommentSubmission::AddToReview => {
                                if let Some(pending_review) = view.pending_review.as_mut() {
                                    pending_review.comment_count += 1;
                                }
                            }
                        }

                        view.review_composer = None;
                        view.review_line_selection = None;
                        view.review_comment_error = None;
                        view.status = match submission {
                            ReviewCommentSubmission::SingleComment => {
                                format!("Posted comment on PR #{}", pr.number)
                            }
                            ReviewCommentSubmission::StartReview => {
                                format!("Started pending review on PR #{}", pr.number)
                            }
                            ReviewCommentSubmission::AddToReview => {
                                format!("Added review comment on PR #{}", pr.number)
                            }
                        };
                        view.load_selected_review_data(cx);
                    }
                    Err(error) => {
                        let message = format!("Failed to submit review comment: {error}");
                        view.review_comment_error = Some(message.clone());
                        view.status = message;
                    }
                }

                cx.notify();
            }) {
                eprintln!("failed to update review comment submission state: {error}");
            }
        })
        .detach();
    }

    pub(crate) fn submit_pending_pull_request_review(
        &mut self,
        event: SubmitPullRequestReviewEvent,
        cx: &mut Context<Self>,
    ) {
        if self.is_submitting_pending_review || self.is_running_pr_action {
            self.status = "A pull request action is already running".to_string();
            cx.notify();
            return;
        }

        let Some(pending_review) = self.pending_review.clone() else {
            self.pending_review_error = Some("No pending review to submit".to_string());
            self.status = "No pending review to submit".to_string();
            cx.notify();
            return;
        };
        let Some(pr) = self.selected_pull_request().cloned() else {
            self.pending_review_error =
                Some("Select a pull request before submitting a review".to_string());
            self.status = "Select a pull request before submitting a review".to_string();
            cx.notify();
            return;
        };

        let body = self.pending_review_body_input.read(cx).value().to_string();
        let body = match event {
            SubmitPullRequestReviewEvent::RequestChanges if body.trim().is_empty() => {
                Some(DEFAULT_REQUEST_CHANGES_BODY.to_string())
            }
            _ => {
                let trimmed = body.trim();
                (!trimmed.is_empty()).then(|| trimmed.to_string())
            }
        };

        self.is_submitting_pending_review = true;
        self.is_running_pr_action = true;
        self.pending_review_error = None;
        self.status = format!("Submitting pending review on PR #{}", pr.number);
        cx.notify();

        cx.spawn(async move |this, cx| {
            let result = GitHubClient::new(GhCliTransport)
                .submit_pull_request_review(&pending_review.node_id, event, body.as_deref())
                .await;

            if let Err(error) = this.update(cx, move |view, cx| {
                view.is_submitting_pending_review = false;
                view.is_running_pr_action = false;

                match result {
                    Ok(()) => {
                        view.pending_review = None;
                        view.pending_review_error = None;
                        view.status = format!("Submitted pending review on PR #{}", pr.number);
                        view.load_selected_review_data(cx);
                    }
                    Err(error) => {
                        let message = format!("Failed to submit pending review: {error}");
                        view.pending_review_error = Some(message.clone());
                        view.status = message;
                    }
                }

                cx.notify();
            }) {
                eprintln!("failed to update pending review submission state: {error}");
            }
        })
        .detach();
    }

    fn on_switcher_search_event(
        &mut self,
        input: &Entity<InputState>,
        event: &InputEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let is_repository_input = input.entity_id() == self.repository_search_input.entity_id();
        let is_pull_request_input = input.entity_id() == self.pull_request_search_input.entity_id();

        match event {
            InputEvent::Change => {
                if is_repository_input {
                    self.repository_switcher_selection = 0;
                } else if is_pull_request_input {
                    self.pull_request_switcher_selection = 0;
                }

                cx.notify();
            }
            InputEvent::PressEnter { .. }
                if is_repository_input && self.repository_switcher_open =>
            {
                self.accept_repository_switcher_selection(cx);
            }
            InputEvent::PressEnter { .. }
                if is_pull_request_input && self.pull_request_switcher_open =>
            {
                self.accept_pull_request_switcher_selection(cx);
            }
            _ => {}
        }
    }

    fn on_review_input_event(
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

pub(crate) fn normalized_search_query(query: &str) -> String {
    query.trim().to_lowercase()
}

pub(crate) fn repository_matches_query(repository: &RepoId, query: &str) -> bool {
    if query.is_empty() {
        return true;
    }

    repository.full_name().to_lowercase().contains(query)
}

pub(crate) fn pull_request_matches_query(pull_request: &PullRequest, query: &str) -> bool {
    if query.is_empty() {
        return true;
    }

    pull_request.title.to_lowercase().contains(query)
        || pull_request.number.to_string().contains(query)
        || pull_request.author.to_lowercase().contains(query)
}

pub(crate) fn next_switcher_index(current: usize, len: usize, delta: isize) -> usize {
    if len == 0 {
        return 0;
    }

    let current = current.min(len - 1) as isize;
    (current + delta).rem_euclid(len as isize) as usize
}

fn review_composer_from_selection(
    anchor: &ReviewLineTarget,
    current: &ReviewLineTarget,
) -> std::result::Result<ReviewComposer, String> {
    let range = review_range_from_targets(anchor, current)?;
    let anchor = if anchor.line_index >= current.line_index {
        anchor.clone()
    } else {
        current.clone()
    };

    Ok(ReviewComposer { anchor, range })
}

pub(crate) fn review_range_from_targets(
    anchor: &ReviewLineTarget,
    current: &ReviewLineTarget,
) -> std::result::Result<ReviewCommentRange, String> {
    if anchor.hunk_index != current.hunk_index {
        return Err("Review comments can only span lines in one diff hunk".to_string());
    }

    if anchor.range.path != current.range.path {
        return Err("Review comments can only span one file".to_string());
    }

    if anchor.range.side != current.range.side {
        return Err("Review comments can only span one diff side".to_string());
    }

    let (start, end) = if anchor.line_index <= current.line_index {
        (anchor, current)
    } else {
        (current, anchor)
    };
    let mut range = end.range.clone();

    if start.line_index != end.line_index {
        range.start_line = Some(start.range.line);
        range.start_side = Some(start.range.side);
    } else {
        range.start_line = None;
        range.start_side = None;
    }

    Ok(range)
}

fn pending_review_from_reviews(
    reviews: &[PullRequestReview],
    current_user_login: Option<&str>,
    existing_pending_review: Option<&PendingReviewSession>,
) -> Option<PendingReviewSession> {
    reviews
        .iter()
        .find(|review| {
            review.state == PullRequestReviewState::Pending
                && current_user_login.is_none_or(|login| review.author == login)
                && review
                    .node_id
                    .as_ref()
                    .is_some_and(|node_id| !node_id.is_empty())
        })
        .and_then(|review| {
            let node_id = review.node_id.clone()?;
            let comment_count = existing_pending_review
                .filter(|pending_review| pending_review.node_id == node_id)
                .map_or(0, |pending_review| pending_review.comment_count);

            Some(PendingReviewSession {
                node_id,
                comment_count,
            })
        })
        .or_else(|| existing_pending_review.cloned())
}

fn review_comment_range_label(range: &ReviewCommentRange) -> String {
    let side = match range.side {
        ReviewSide::Left => "left",
        ReviewSide::Right => "right",
    };

    if let Some(start_line) = range.start_line {
        format!("{side} lines {start_line}-{}", range.line)
    } else {
        format!("{side} line {}", range.line)
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
