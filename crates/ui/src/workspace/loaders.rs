use gpui::{AppContext, Context, ScrollStrategy};
use harbor_domain::{RepoId, ReviewThreadState};
use harbor_github::{GhCliTransport, GitHubClient};
use harbor_logs::parse_workflow_log;
use harbor_storage::{SqliteStore, StorageConfig, StorageError};

use crate::actions::PanelTab;
use crate::diff::parse_files;
use crate::panels::{checks_summary_from_runs, workflow_run_label};
use crate::workspace::AppView;

impl AppView {
    pub(super) fn load_recent_repositories(&mut self, cx: &mut Context<Self>) {
        let configured_repo = self.configured_repo.clone();
        let task = cx.background_spawn(async move { load_repository_store(configured_repo).await });

        self.repository_task = Some(cx.spawn(async move |this, cx| {
            let result = task.await;

            if let Err(error) = this.update(cx, move |view, cx| {
                match result {
                    Ok(load) => {
                        let repository_count = load.repositories.len();
                        let repository_error = load.repository_error.clone();
                        view.repository_store = Some(load.store);
                        view.repository_error = load.repository_error;

                        for repository in load.repositories.into_iter().rev() {
                            view.remember_repository(repository);
                        }

                        if view.configured_repo.is_none()
                            && !view.is_loading_prs
                            && view.pull_requests.is_empty()
                        {
                            view.status = match (repository_count, repository_error) {
                                (0, Some(error)) => error,
                                (0, None) => "No repositories found from GitHub CLI".to_string(),
                                (count, Some(_)) => {
                                    format!(
                                        "Loaded {count} cached repositories; GitHub refresh failed"
                                    )
                                }
                                (count, None) => {
                                    format!("Loaded {count} repositories. Choose one with cmd+p")
                                }
                            };
                        }
                    }
                    Err(error) => {
                        view.repository_store = None;
                        view.repository_error = Some(error.to_string());
                        view.status = "Failed to initialize repository storage".to_string();
                    }
                }

                cx.notify();
            }) {
                eprintln!("failed to update repository store state: {error}");
            }
        }));
    }

    pub(crate) fn record_recent_repository(&mut self, repository: RepoId, cx: &mut Context<Self>) {
        self.remember_repository(repository.clone());

        let Some(store) = self.repository_store.clone() else {
            return;
        };

        let task = cx.background_spawn(async move { store.record_repository(&repository).await });

        cx.spawn(async move |this, cx| {
            let result = task.await;

            if let Err(error) = this.update(cx, move |view, cx| {
                match result {
                    Ok(()) => {
                        view.repository_error = None;
                    }
                    Err(error) => {
                        view.repository_error = Some(error.to_string());
                    }
                }

                cx.notify();
            }) {
                eprintln!("failed to update repository write state: {error}");
            }
        })
        .detach();
    }

    pub(super) fn load_pull_requests(&mut self, repo: RepoId, cx: &mut Context<Self>) {
        self.configured_repo = Some(repo.clone());
        self.record_recent_repository(repo.clone(), cx);
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
        self.pr_detail_tasks.clear();
        self.clear_review_composer_state();
        self.pending_review = None;
        self.review_comment_error = None;
        self.pending_review_error = None;
        self.is_loading_details = false;
        self.is_loading_files = false;
        self.is_loading_checks = false;
        self.is_loading_workflows = false;
        self.is_loading_reviews = false;
        self.is_loading_logs = false;
        self.status = format!("Loading open pull requests from {}", repo.full_name());

        let owner = repo.owner.clone();
        let name = repo.name.clone();

        self.pr_list_task = Some(cx.spawn(async move |this, cx| {
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
                        view.clear_review_composer_state();
                        view.pending_review = None;
                        view.review_comment_error = None;
                        view.pending_review_error = None;
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
                        view.clear_review_composer_state();
                        view.pending_review = None;
                        view.review_comment_error = None;
                        view.pending_review_error = None;
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
        }));
    }

    pub(super) fn load_selected_pull_request(&mut self, cx: &mut Context<Self>) {
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
        self.pr_detail_tasks.clear();
        self.files.clear();
        self.diffs.clear();
        self.check_runs.clear();
        self.workflow_runs.clear();
        self.workflow_jobs.clear();
        self.pull_request_reviews.clear();
        self.review_threads.clear();
        self.clear_review_composer_state();
        self.review_comment_error = None;
        self.pending_review_error = None;
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

        self.pr_detail_tasks.push(cx.spawn({
            let owner = owner.clone();
            let name = name.clone();
            let repo = repo.clone();

            async move |this, cx| {
                let result = GitHubClient::new(GhCliTransport)
                    .get_pull_request(&owner, &name, number)
                    .await;

                if let Err(error) = this.update(cx, move |view, cx| {
                    if !selected_pull_request_matches(view, &repo, number) {
                        return;
                    }

                    view.is_loading_details = false;
                    match result {
                        Ok(detail) => {
                            if let Some(selected) = view.pull_requests.get_mut(view.selected_pr) {
                                *selected = detail;
                            }
                            view.details_error = None;
                            view.status = format!("Loaded PR #{number} details");
                        }
                        Err(error) => {
                            view.details_error = Some(error.to_string());
                            view.status = format!("Failed to load PR #{number} details");
                        }
                    }

                    cx.notify();
                }) {
                    eprintln!("failed to update pull request detail state: {error}");
                }
            }
        }));

        self.pr_detail_tasks.push(cx.spawn({
            let owner = owner.clone();
            let name = name.clone();
            let repo = repo.clone();

            async move |this, cx| {
                let result = GitHubClient::new(GhCliTransport)
                    .list_pull_request_files(&owner, &name, number)
                    .await
                    .map(|files| {
                        let diffs = parse_files(&files);
                        (files, diffs)
                    });

                if let Err(error) = this.update(cx, move |view, cx| {
                    if !selected_pull_request_matches(view, &repo, number) {
                        return;
                    }

                    view.is_loading_files = false;
                    match result {
                        Ok((files, diffs)) => {
                            let count = files.len();
                            view.files = files;
                            view.diffs = diffs;
                            view.active_file = 0;
                            view.active_hunk = 0;
                            view.clear_review_composer_state();
                            view.file_list_scroll.scroll_to_item(0, ScrollStrategy::Top);
                            view.diff_list_scroll.scroll_to_item(0, ScrollStrategy::Top);
                            view.files_error = None;
                            view.status = format!("Loaded {count} changed files for PR #{number}");
                        }
                        Err(error) => {
                            view.files.clear();
                            view.diffs.clear();
                            view.active_file = 0;
                            view.active_hunk = 0;
                            view.clear_review_composer_state();
                            view.file_list_scroll.scroll_to_item(0, ScrollStrategy::Top);
                            view.diff_list_scroll.scroll_to_item(0, ScrollStrategy::Top);
                            view.files_error = Some(error.to_string());
                            view.status = format!("Failed to load changed files for PR #{number}");
                        }
                    }

                    cx.notify();
                }) {
                    eprintln!("failed to update pull request file state: {error}");
                }
            }
        }));

        self.pr_detail_tasks.push(cx.spawn({
            let owner = owner.clone();
            let name = name.clone();
            let repo = repo.clone();
            let head_sha = head_sha.clone();

            async move |this, cx| {
                let result = if head_sha.is_empty() {
                    Ok(Vec::new())
                } else {
                    GitHubClient::new(GhCliTransport)
                        .list_check_runs(&owner, &name, &head_sha)
                        .await
                };

                if let Err(error) = this.update(cx, move |view, cx| {
                    if !selected_pull_request_matches(view, &repo, number) {
                        return;
                    }

                    view.is_loading_checks = false;
                    match result {
                        Ok(check_runs) => {
                            let count = check_runs.len();
                            let summary = checks_summary_from_runs(&check_runs);
                            view.check_runs = check_runs;
                            view.checks_error = None;

                            if let Some(selected) = view.pull_requests.get_mut(view.selected_pr) {
                                selected.checks_summary = summary;
                            }

                            view.status = format!("Loaded {count} check runs for PR #{number}");
                        }
                        Err(error) => {
                            view.check_runs.clear();
                            view.checks_error = Some(error.to_string());
                            view.status = format!("Failed to load checks for PR #{number}");
                        }
                    }

                    cx.notify();
                }) {
                    eprintln!("failed to update pull request checks state: {error}");
                }
            }
        }));

        self.pr_detail_tasks.push(cx.spawn({
            let owner = owner.clone();
            let name = name.clone();
            let repo = repo.clone();
            let head_sha = head_sha.clone();

            async move |this, cx| {
                let result = if head_sha.is_empty() {
                    Ok(Vec::new())
                } else {
                    GitHubClient::new(GhCliTransport)
                        .list_workflow_runs_for_head(&owner, &name, &head_sha)
                        .await
                };

                if let Err(error) = this.update(cx, move |view, cx| {
                    if !selected_pull_request_matches(view, &repo, number) {
                        return;
                    }

                    view.is_loading_workflows = false;
                    match result {
                        Ok(workflow_runs) => {
                            let count = workflow_runs.len();
                            view.workflow_runs = workflow_runs;
                            view.workflows_error = None;
                            view.status = format!("Loaded {count} workflow runs for PR #{number}");

                            if view.active_tab == PanelTab::Logs
                                && view.logs_error.is_none()
                                && !view.workflow_runs.is_empty()
                            {
                                view.load_selected_workflow_logs(cx);
                            }
                        }
                        Err(error) => {
                            view.workflow_runs.clear();
                            view.workflows_error = Some(error.to_string());
                            view.status = format!("Failed to load workflow runs for PR #{number}");
                        }
                    }

                    cx.notify();
                }) {
                    eprintln!("failed to update pull request workflow state: {error}");
                }
            }
        }));

        self.pr_detail_tasks.push(cx.spawn({
            let owner = owner.clone();
            let name = name.clone();
            let repo = repo.clone();

            async move |this, cx| {
                let client = GitHubClient::new(GhCliTransport);
                let current_user_result = client.current_user().await;
                let pull_request_reviews_result = client
                    .list_pull_request_reviews(&owner, &name, number)
                    .await;
                let review_threads_result =
                    client.list_review_threads(&owner, &name, number).await;

                if let Err(error) = this.update(cx, move |view, cx| {
                    if !selected_pull_request_matches(view, &repo, number) {
                        return;
                    }

                    view.is_loading_reviews = false;
                    let mut loaded_review_thread_count = None;
                    let current_user_login = match current_user_result {
                        Ok(login) => {
                            view.reviews_error = None;
                            Some(login)
                        }
                        Err(error) => {
                            view.reviews_error =
                                Some(format!("Failed to detect current user: {error}"));
                            None
                        }
                    };

                    let reviews = match pull_request_reviews_result {
                        Ok(reviews) => Some(reviews),
                        Err(error) => {
                            view.pull_request_reviews.clear();
                            let message = format!("Failed to load review history: {error}");
                            view.reviews_error = Some(match view.reviews_error.take() {
                                Some(existing) => format!("{existing}; {message}"),
                                None => message,
                            });
                            None
                        }
                    };

                    match (reviews, review_threads_result) {
                        (Some(reviews), Ok(review_threads)) => {
                            let thread_count = review_threads.len();
                            view.apply_loaded_review_data(
                                reviews,
                                review_threads,
                                current_user_login,
                            );
                            loaded_review_thread_count = Some(thread_count);
                        }
                        (None, Ok(review_threads)) => {
                            let thread_count = review_threads.len();
                            view.review_threads = review_threads;
                            let unresolved_count = view
                                .review_threads
                                .iter()
                                .filter(|thread| thread.state == ReviewThreadState::Unresolved)
                                .count();
                            if let Some(selected) = view.pull_requests.get_mut(view.selected_pr) {
                                selected.unresolved_threads = unresolved_count;
                            }
                            loaded_review_thread_count = Some(thread_count);
                        }
                        (Some(reviews), Err(error)) => {
                            view.review_threads.clear();
                            view.apply_loaded_review_data(reviews, Vec::new(), current_user_login);
                            let message = format!("Failed to load review threads: {error}");
                            view.reviews_error = Some(match view.reviews_error.take() {
                                Some(existing) => format!("{existing}; {message}"),
                                None => message,
                            });
                        }
                        (None, Err(error)) => {
                            view.review_threads.clear();
                            let message = format!("Failed to load review threads: {error}");
                            view.reviews_error = Some(match view.reviews_error.take() {
                                Some(existing) => format!("{existing}; {message}"),
                                None => message,
                            });
                        }
                    }

                    view.status = match (view.reviews_error.as_ref(), loaded_review_thread_count) {
                        (None, Some(count)) => {
                            format!("Loaded review history and {count} threads for PR #{number}")
                        }
                        (None, None) => format!("Loaded review history for PR #{number}"),
                        (Some(_), Some(count)) => {
                            format!("Loaded {count} review threads for PR #{number}, but review history failed")
                        }
                        (Some(_), None) => format!("Failed to load review data for PR #{number}"),
                    };

                    cx.notify();
                }) {
                    eprintln!("failed to update pull request review state: {error}");
                }
            }
        }));
    }

    pub(crate) fn load_selected_review_data(&mut self, cx: &mut Context<Self>) {
        let Some(repo) = self.configured_repo.clone() else {
            return;
        };
        let Some(number) = self.selected_pull_request_number() else {
            return;
        };

        self.is_loading_reviews = true;
        self.reviews_error = None;
        self.status = format!("Refreshing review data for PR #{number}");
        cx.notify();

        let owner = repo.owner.clone();
        let name = repo.name.clone();

        self.pr_detail_tasks.push(cx.spawn(async move |this, cx| {
            let client = GitHubClient::new(GhCliTransport);
            let current_user_result = client.current_user().await;
            let reviews_result = client
                .list_pull_request_reviews(&owner, &name, number)
                .await;
            let threads_result = client.list_review_threads(&owner, &name, number).await;

            if let Err(error) = this.update(cx, move |view, cx| {
                if !selected_pull_request_matches(view, &repo, number) {
                    return;
                }

                view.is_loading_reviews = false;
                let current_user_login = current_user_result.ok();

                match (reviews_result, threads_result) {
                    (Ok(reviews), Ok(threads)) => {
                        let thread_count = threads.len();
                        view.apply_loaded_review_data(reviews, threads, current_user_login);
                        view.reviews_error = None;
                        view.status =
                            format!("Refreshed review data and {thread_count} threads for PR #{number}");
                    }
                    (Err(reviews_error), Ok(threads)) => {
                        let thread_count = threads.len();
                        view.pull_request_reviews.clear();
                        view.review_threads = threads;
                        view.reviews_error =
                            Some(format!("Failed to load review history: {reviews_error}"));
                        view.status = format!(
                            "Refreshed {thread_count} review threads for PR #{number}, but review history failed"
                        );
                    }
                    (Ok(reviews), Err(threads_error)) => {
                        view.review_threads.clear();
                        view.apply_loaded_review_data(reviews, Vec::new(), current_user_login);
                        view.reviews_error =
                            Some(format!("Failed to load review threads: {threads_error}"));
                        view.status =
                            format!("Refreshed review history for PR #{number}, but threads failed");
                    }
                    (Err(reviews_error), Err(threads_error)) => {
                        view.pull_request_reviews.clear();
                        view.review_threads.clear();
                        view.reviews_error = Some(format!(
                            "Failed to load review history: {reviews_error}; Failed to load review threads: {threads_error}"
                        ));
                        view.status = format!("Failed to refresh review data for PR #{number}");
                    }
                }

                cx.notify();
            }) {
                eprintln!("failed to update refreshed review state: {error}");
            }
        }));
    }

    pub(crate) fn load_selected_workflow_logs(&mut self, cx: &mut Context<Self>) {
        let Some(repo) = self.configured_repo.clone() else {
            self.logs_error =
                Some("Workflow logs require a selected repository and GitHub CLI auth".into());
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

        self.log_task = Some(cx.spawn(async move |this, cx| {
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
        }));
    }
}

struct RepositoryLoad {
    store: SqliteStore,
    repositories: Vec<RepoId>,
    repository_error: Option<String>,
}

fn selected_pull_request_matches(view: &AppView, repository: &RepoId, number: u64) -> bool {
    view.configured_repo.as_ref() == Some(repository)
        && view.selected_pull_request_number() == Some(number)
}

async fn load_repository_store(
    configured_repo: Option<RepoId>,
) -> std::result::Result<RepositoryLoad, StorageError> {
    let store = SqliteStore::connect(StorageConfig::from_env()?).await?;

    if let Some(repository) = configured_repo.as_ref() {
        store.record_repository(repository).await?;
    }

    let repository_error = match GitHubClient::new(GhCliTransport).list_repositories().await {
        Ok(repositories) => {
            store.sync_repositories(&repositories).await?;
            None
        }
        Err(error) => Some(format!(
            "failed to load repositories from GitHub CLI: {error}"
        )),
    };

    let repositories = store
        .recent_repositories()
        .await?
        .into_iter()
        .map(|repository| repository.id)
        .collect();

    Ok(RepositoryLoad {
        store,
        repositories,
        repository_error,
    })
}
