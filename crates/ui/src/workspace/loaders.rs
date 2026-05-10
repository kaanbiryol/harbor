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
                crate::workspace::log_entity_update_error("failed to update repository store state", error);
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
                crate::workspace::log_entity_update_error("failed to update repository refresh state", error);
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
                crate::workspace::log_entity_update_error(
                    "failed to update repository write state",
                    error,
                );
            }
        })
        .detach();
    }

    pub(super) fn load_pull_requests(&mut self, repo: RepoId, cx: &mut Context<Self>) {
        self.load_repository_pull_requests(
            repo,
            self.pull_request_inbox.mode,
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
            self.pull_request_inbox.mode,
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
        self.pull_request_inbox.mode = mode;
        self.record_recent_repository(repo.clone(), cx);
        self.is_loading_prs = true;
        self.load_error = None;
        self.clear_detail_errors();
        self.clear_log_error();
        self.clear_action_errors();
        self.pr_detail_tasks.clear();
        self.clear_review_data_state();
        self.clear_review_submission_errors();
        self.collapsed_file_tree_folders.clear();
        self.reviewed_file_paths.clear();
        self.reset_changed_file_filters();
        self.owned_file_paths.clear();
        self.set_detail_loading(false);
        self.set_log_loading(false);
        self.status = pull_request_inbox_loading_status(&repo, mode);

        self.pr_list_task = Some(cx.spawn(async move |this, cx| {
            let result = GitHubClient::new(GhCliTransport)
                .list_repository_pull_requests(&repo, pull_request_list_filter(mode))
                .await;

            if let Err(error) = this.update(cx, |view, cx| {
                if view.current_pull_request_inbox_key().as_ref() != Some(&key) {
                    return;
                }

                view.is_loading_prs = false;

                match result {
                    Ok(pull_requests) => {
                        let count = pull_requests.len();
                        let status = pull_request_inbox_loaded_status(&repo, mode, count);
                        view.pull_requests = pull_requests;
                        view.clear_changed_file_state();
                        view.clear_workflow_state();
                        view.clear_review_data_state();
                        view.clear_review_submission_errors();
                        view.clear_log_content();
                        view.selected_pr = 0;
                        view.reset_diff_selection();
                        view.pr_list_scroll.scroll_to_item(0, ScrollStrategy::Top);
                        view.reset_detail_scrolls();
                        view.load_error = None;
                        view.status = status;
                        view.refresh_selected_pull_request(cx);
                    }
                    Err(error) => {
                        let status = pull_request_inbox_failed_status(&repo, mode);
                        view.pull_requests.clear();
                        view.clear_changed_file_state();
                        view.clear_workflow_state();
                        view.clear_review_data_state();
                        view.clear_review_submission_errors();
                        view.clear_log_content();
                        view.selected_pr = 0;
                        view.reset_diff_selection();
                        view.pr_list_scroll.scroll_to_item(0, ScrollStrategy::Top);
                        view.reset_detail_scrolls();
                        view.set_detail_loading(false);
                        view.set_log_loading(false);
                        view.is_running_action = false;
                        view.is_running_pr_action = false;
                        view.load_error = Some(error.to_string());
                        view.status = status;
                    }
                }

                cx.notify();
            }) {
                crate::workspace::log_entity_update_error(
                    "failed to update pull request inbox state",
                    error,
                );
            }
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
