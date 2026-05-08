use gpui::{AppContext, Context, ScrollStrategy};
use harbor_domain::RepoId;
use harbor_github::{GhCliTransport, GitHubClient, PullRequestListFilter};
use harbor_storage::{RecentRepository, SqliteStore, StorageConfig, StorageError};

use crate::workspace::{AppView, PullRequestInboxCacheKey, PullRequestInboxMode};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PullRequestFetchPolicy {
    PreferCache,
    Refresh,
}

impl AppView {
    pub(super) fn load_recent_repositories(&mut self, cx: &mut Context<Self>) {
        let configured_repo = self.configured_repo.clone();
        self.is_loading_repositories = true;
        let task = cx.background_spawn(async move { load_repository_store(configured_repo).await });

        self.repository_task = Some(cx.spawn(async move |this, cx| {
            let result = task.await;

            if let Err(error) = this.update(cx, move |view, cx| {
                match result {
                    Ok(load) => {
                        let repository_count = load.repositories.len();
                        let store = load.store.clone();
                        view.repository_store = Some(load.store);
                        view.repository_error = None;

                        view.apply_recent_repositories(load.repositories);
                        if let Some(repository) = view.configured_repo.clone() {
                            view.record_recent_repository(repository, cx);
                        }

                        if view.configured_repo.is_none()
                            && !view.is_loading_prs
                            && view.pull_requests.is_empty()
                        {
                            view.status = if repository_count == 0 {
                                "Fetching repositories from GitHub...".to_string()
                            } else {
                                format!(
                                    "Loaded {repository_count} cached repositories. Choose one from the header or type owner/repo"
                                )
                            };
                        }

                        view.refresh_repositories_from_github(store, cx);
                    }
                    Err(error) => {
                        view.repository_store = None;
                        view.is_loading_repositories = false;
                        view.repository_error = Some(error.to_string());
                        view.status = "Failed to initialize repository storage".to_string();
                    }
                }

                cx.notify();
            }) {
                tracing::warn!(%error, "failed to update repository store state");
            }
        }));
    }

    fn refresh_repositories_from_github(&mut self, store: SqliteStore, cx: &mut Context<Self>) {
        self.is_loading_repositories = true;
        let task = cx.background_spawn(async move { refresh_repository_store(store).await });

        self.repository_task = Some(cx.spawn(async move |this, cx| {
            let result = task.await;

            if let Err(error) = this.update(cx, move |view, cx| {
                view.is_loading_repositories = false;

                match result {
                    Ok(load) => {
                        let repository_count = load.repositories.len();
                        let repository_error = load.repository_error.clone();
                        view.repository_error = load.repository_error;
                        view.apply_recent_repositories(load.repositories);

                        if view.configured_repo.is_none()
                            && !view.is_loading_prs
                            && view.pull_requests.is_empty()
                        {
                            view.status = match (repository_count, repository_error) {
                                (0, Some(error)) => error,
                                (0, None) => {
                                    "No repositories found. Type owner/repo to open a repository"
                                        .to_string()
                                }
                                (count, Some(_)) => {
                                    format!(
                                        "Loaded {count} cached repositories; GitHub refresh failed. Choose one from the header or type owner/repo"
                                    )
                                }
                                (count, None) => {
                                    format!(
                                        "Loaded {count} repositories. Choose one from the header or type owner/repo"
                                    )
                                }
                            };
                        }
                    }
                    Err(error) => {
                        view.repository_error = Some(error.to_string());
                        if view.configured_repo.is_none()
                            && !view.is_loading_prs
                            && view.pull_requests.is_empty()
                        {
                            view.status = error.to_string();
                        }
                    }
                }

                cx.notify();
            }) {
                tracing::warn!(%error, "failed to update repository refresh state");
            }
        }));
    }

    fn apply_recent_repositories(&mut self, repositories: Vec<RecentRepository>) {
        for repository in repositories.into_iter().rev() {
            self.remember_repository(repository.id.clone());
            if let Some(local_path) = repository.local_path {
                self.set_repository_local_path(repository.id, local_path);
            }
        }
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
                tracing::warn!(%error, "failed to update repository write state");
            }
        })
        .detach();
    }

    pub(super) fn load_pull_requests(&mut self, repo: RepoId, cx: &mut Context<Self>) {
        self.load_repository_pull_requests(
            repo,
            self.pull_request_inbox_mode,
            PullRequestFetchPolicy::PreferCache,
            cx,
        );
    }

    pub(super) fn load_repository_pull_requests_from_cache(
        &mut self,
        repo: RepoId,
        mode: PullRequestInboxMode,
        cx: &mut Context<Self>,
    ) {
        self.load_repository_pull_requests(repo, mode, PullRequestFetchPolicy::PreferCache, cx);
    }

    pub(super) fn refresh_pull_requests(&mut self, repo: RepoId, cx: &mut Context<Self>) {
        self.load_repository_pull_requests(
            repo,
            self.pull_request_inbox_mode,
            PullRequestFetchPolicy::Refresh,
            cx,
        );
    }

    pub(super) fn reload_pull_request_inbox(&mut self, cx: &mut Context<Self>) {
        if let Some(repo) = self.configured_repo.clone() {
            self.refresh_pull_requests(repo, cx);
        } else {
            self.status =
                "Select a repository from the header before loading pull requests".to_string();
            cx.notify();
        }
    }

    fn load_repository_pull_requests(
        &mut self,
        repo: RepoId,
        mode: PullRequestInboxMode,
        fetch_policy: PullRequestFetchPolicy,
        cx: &mut Context<Self>,
    ) {
        let key = PullRequestInboxCacheKey::new(repo.clone(), mode);

        self.cache_current_pull_request_inbox_snapshot();

        if fetch_policy == PullRequestFetchPolicy::PreferCache
            && self.restore_pull_request_inbox_snapshot(key.clone(), cx)
        {
            self.record_recent_repository(repo, cx);
            return;
        }

        self.configured_repo = Some(repo.clone());
        self.pull_request_inbox_mode = mode;
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
        self.collapsed_file_tree_folders.clear();
        self.reviewed_file_paths.clear();
        self.reset_changed_file_filters();
        self.owned_file_paths.clear();
        self.is_loading_details = false;
        self.is_loading_files = false;
        self.is_loading_checks = false;
        self.is_loading_workflows = false;
        self.is_loading_reviews = false;
        self.is_loading_logs = false;
        self.status = pull_request_inbox_loading_status(&repo, mode);

        self.pr_list_task = Some(cx.spawn(async move |this, cx| {
            let result = GitHubClient::new(GhCliTransport)
                .list_repository_pull_requests(&repo, pull_request_list_filter(mode))
                .await;

            _ = this.update(cx, |view, cx| {
                if view.current_pull_request_inbox_key().as_ref() != Some(&key) {
                    return;
                }

                view.is_loading_prs = false;

                match result {
                    Ok(pull_requests) => {
                        let count = pull_requests.len();
                        let status = pull_request_inbox_loaded_status(&repo, mode, count);
                        view.pull_requests = pull_requests;
                        view.files.clear();
                        view.diffs.clear();
                        view.collapsed_file_tree_folders.clear();
                        view.reviewed_file_paths.clear();
                        view.reset_changed_file_filters();
                        view.owned_file_paths.clear();
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
                        view.status = status;
                        view.refresh_selected_pull_request(cx);
                    }
                    Err(error) => {
                        let status = pull_request_inbox_failed_status(&repo, mode);
                        view.pull_requests.clear();
                        view.files.clear();
                        view.diffs.clear();
                        view.collapsed_file_tree_folders.clear();
                        view.reviewed_file_paths.clear();
                        view.reset_changed_file_filters();
                        view.owned_file_paths.clear();
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
                        view.status = status;
                    }
                }

                cx.notify();
            });
        }));
    }
}

struct RepositoryLoad {
    store: SqliteStore,
    repositories: Vec<RecentRepository>,
}

struct RepositoryRefresh {
    repositories: Vec<RecentRepository>,
    repository_error: Option<String>,
}

fn pull_request_list_filter(mode: PullRequestInboxMode) -> PullRequestListFilter {
    match mode {
        PullRequestInboxMode::Open => PullRequestListFilter::Open,
        PullRequestInboxMode::Closed => PullRequestListFilter::Closed,
        PullRequestInboxMode::NeedsReview => PullRequestListFilter::NeedsReview,
    }
}

fn pull_request_inbox_loading_status(repository: &RepoId, mode: PullRequestInboxMode) -> String {
    format!(
        "Loading {} from {}",
        mode.status_label(),
        repository.full_name()
    )
}

fn pull_request_inbox_loaded_status(
    repository: &RepoId,
    mode: PullRequestInboxMode,
    count: usize,
) -> String {
    format!(
        "Loaded {count} {} from {}",
        mode.status_label(),
        repository.full_name()
    )
}

fn pull_request_inbox_failed_status(repository: &RepoId, mode: PullRequestInboxMode) -> String {
    format!(
        "Failed to load {} from {}",
        mode.status_label(),
        repository.full_name()
    )
}

async fn load_repository_store(
    configured_repo: Option<RepoId>,
) -> std::result::Result<RepositoryLoad, StorageError> {
    let store = SqliteStore::connect(StorageConfig::from_env()?).await?;

    if let Some(repository) = configured_repo.as_ref() {
        store.record_repository(repository).await?;
    }

    let repositories = store.recent_repositories().await?;

    Ok(RepositoryLoad {
        store,
        repositories,
    })
}

async fn refresh_repository_store(
    store: SqliteStore,
) -> std::result::Result<RepositoryRefresh, StorageError> {
    let repository_error = match GitHubClient::new(GhCliTransport).list_repositories().await {
        Ok(repositories) => {
            store.sync_repositories(&repositories).await?;
            None
        }
        Err(error) => Some(format!(
            "failed to load repositories from GitHub CLI: {error}"
        )),
    };

    let repositories = store.recent_repositories().await?;

    Ok(RepositoryRefresh {
        repositories,
        repository_error,
    })
}
