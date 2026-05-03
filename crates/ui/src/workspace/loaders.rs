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

        self.pr_detail_task = Some(cx.spawn(async move |this, cx| {
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
