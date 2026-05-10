use gpui::{AppContext, Context, ScrollStrategy};
use harbor_domain::{PullRequest, RepoId};
use harbor_github::PullRequestListFilter;
use harbor_storage::{RecentRepository, SqliteStore, StorageConfig, StorageError};
use harbor_sync::{SyncTarget, detect_pull_request_changes};

use crate::workspace::{
    AppView, PullRequestInboxCacheKey, PullRequestInboxMode, async_updates::AppViewAsyncUpdateExt,
};

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

            this.update_or_log(cx, "failed to update repository store state", move |view, cx| {
                match result {
                    Ok(load) => {
                        let repository_count = load.repositories.len();
                        let last_selected_repository = load.last_selected_repository.clone();
                        let store = load.store.clone();
                        view.repository_store = Some(load.store);
                        view.repository_error = None;

                        view.apply_recent_repositories(load.repositories);
                        if let Some(repository) = view.configured_repo.clone() {
                            view.record_recent_repository(repository, cx);
                        } else if let Some(repository) = last_selected_repository {
                            view.status =
                                format!("Opening last repository {}", repository.full_name());
                            view.load_repository_pull_requests_from_cache(
                                repository,
                                view.pull_request_inbox.mode,
                                cx,
                            );
                        } else if view.configured_repo.is_none()
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
            });
        }));
    }

    fn refresh_repositories_from_github(&mut self, store: SqliteStore, cx: &mut Context<Self>) {
        self.is_loading_repositories = true;
        let github_api = self.github_api.clone();
        let task =
            cx.background_spawn(async move { refresh_repository_store(store, github_api).await });

        self.repository_task = Some(cx.spawn(async move |this, cx| {
            let result = task.await;

            this.update_or_log(
                cx,
                "failed to update repository refresh state",
                move |view, cx| {
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
                },
            );
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

            this.update_or_log(
                cx,
                "failed to update repository write state",
                move |view, cx| {
                    match result {
                        Ok(()) => {
                            view.repository_error = None;
                        }
                        Err(error) => {
                            view.repository_error = Some(error.to_string());
                        }
                    }

                    cx.notify();
                },
            );
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
            self.mark_active_inbox_stale();
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
            self.record_recent_repository(repo.clone(), cx);
            self.spawn_pull_request_inbox_refresh(repo, mode, key, cx);
            return;
        }

        self.configured_repo = Some(repo.clone());
        self.pull_request_inbox.mode = mode;
        self.ensure_sync_loop(cx);
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
        self.clear_detail_loaded_state();
        self.set_detail_loading(false);
        self.set_log_loading(false);
        self.status = pull_request_inbox_loading_status(&repo, mode);

        if fetch_policy == PullRequestFetchPolicy::PreferCache
            && let Some(store) = self.repository_store.clone()
        {
            let load_repo = repo.clone();
            let load_key = key.clone();
            let task = cx.background_spawn(async move {
                store
                    .load_pull_request_inbox(&load_repo, mode.key())
                    .await
                    .map(|pull_requests| (pull_requests, store))
            });

            self.pr_list_task = Some(cx.spawn(async move |this, cx| {
                let result = task.await;

                this.update_or_log(
                    cx,
                    "failed to update cached pull request inbox state",
                    move |view, cx| {
                        if view.current_pull_request_inbox_key().as_ref() != Some(&load_key) {
                            return;
                        }

                        match result {
                            Ok((pull_requests, _store)) if !pull_requests.is_empty() => {
                                let count = pull_requests.len();
                                view.apply_loaded_pull_request_inbox(
                                    repo.clone(),
                                    mode,
                                    pull_requests,
                                    true,
                                    cx,
                                );
                                view.status = format!(
                                    "Showing {count} cached {} from {}",
                                    mode.status_label(),
                                    repo.full_name()
                                );
                                view.spawn_pull_request_inbox_refresh(repo, mode, load_key, cx);
                            }
                            Ok((_pull_requests, _store)) => {
                                view.spawn_pull_request_inbox_refresh(repo, mode, load_key, cx);
                            }
                            Err(error) => {
                                view.repository_error = Some(error.to_string());
                                view.spawn_pull_request_inbox_refresh(repo, mode, load_key, cx);
                            }
                        }

                        cx.notify();
                    },
                );
            }));
            return;
        }

        self.spawn_pull_request_inbox_refresh(repo, mode, key, cx);
    }

    pub(crate) fn spawn_pull_request_inbox_refresh(
        &mut self,
        repo: RepoId,
        mode: PullRequestInboxMode,
        key: PullRequestInboxCacheKey,
        cx: &mut Context<Self>,
    ) {
        self.is_loading_prs = true;
        self.load_error = None;
        self.mark_sync_attempt(SyncTarget::ActiveInbox);
        let github_api = self.github_api.clone();
        let store = self.repository_store.clone();
        let previous_pull_requests = self.pull_requests.clone();

        self.pr_list_task = Some(cx.spawn(async move |this, cx| {
            let result = github_api
                .list_repository_pull_requests(&repo, pull_request_list_filter(mode))
                .await;
            let cache_result = match (&store, result.as_ref()) {
                (Some(store), Ok(pull_requests)) => store
                    .save_pull_request_inbox(&repo, mode.key(), pull_requests)
                    .await
                    .map_err(|error| error.to_string()),
                (Some(store), Err(error)) => store
                    .record_sync_failure(
                        &harbor_storage::inbox_target_key(&repo, mode.key()),
                        &error.to_string(),
                    )
                    .await
                    .map_err(|error| error.to_string()),
                (None, _) => Ok(()),
            };

            this.update_or_log(
                cx,
                "failed to update pull request inbox state",
                move |view, cx| {
                    if view.current_pull_request_inbox_key().as_ref() != Some(&key) {
                        return;
                    }

                    view.is_loading_prs = false;
                    if let Err(error) = cache_result {
                        view.repository_error = Some(error);
                    }

                    match result {
                        Ok(pull_requests) => {
                            view.mark_sync_success(SyncTarget::ActiveInbox);
                            let count = pull_requests.len();
                            let status = pull_request_inbox_loaded_status(&repo, mode, count);
                            let change_events = detect_pull_request_changes(
                                &previous_pull_requests,
                                &pull_requests,
                            );
                            view.apply_loaded_pull_request_inbox(
                                repo.clone(),
                                mode,
                                pull_requests,
                                true,
                                cx,
                            );
                            view.load_error = None;
                            view.status = status;
                            view.handle_pull_request_change_events(change_events, cx);
                        }
                        Err(error) => {
                            view.mark_sync_failure(SyncTarget::ActiveInbox);
                            let mut status = pull_request_inbox_failed_status(&repo, mode);
                            view.set_detail_loading(false);
                            view.set_log_loading(false);
                            view.is_running_action = false;
                            view.is_running_pr_action = false;
                            view.load_error = Some(error.to_string());
                            if !view.pull_requests.is_empty() {
                                status = format!("{status}; showing cached data");
                            } else {
                                view.clear_changed_file_state();
                                view.clear_workflow_state();
                                view.clear_review_data_state();
                                view.clear_detail_loaded_state();
                                view.clear_review_submission_errors();
                                view.clear_log_content();
                                view.selected_pr = 0;
                                view.reset_diff_selection();
                                view.pr_list_scroll.scroll_to_item(0, ScrollStrategy::Top);
                                view.reset_detail_scrolls();
                            }
                            view.status = status;
                        }
                    }

                    cx.notify();
                },
            );
        }));
    }

    fn apply_loaded_pull_request_inbox(
        &mut self,
        repo: RepoId,
        mode: PullRequestInboxMode,
        pull_requests: Vec<PullRequest>,
        load_selected_detail: bool,
        cx: &mut Context<Self>,
    ) {
        let previous_selected = self
            .selected_pull_request()
            .map(|pull_request| (pull_request.number, pull_request.head_sha.clone()));
        let previous_key = self.current_pull_request_inbox_key();
        let same_inbox = previous_key
            .as_ref()
            .is_some_and(|key| key == &PullRequestInboxCacheKey::new(repo.clone(), mode));

        self.configured_repo = Some(repo);
        self.pull_request_inbox.mode = mode;
        self.pull_requests = pull_requests;

        self.selected_pr = previous_selected
            .as_ref()
            .and_then(|(number, _)| {
                self.pull_requests
                    .iter()
                    .position(|pull_request| pull_request.number == *number)
            })
            .unwrap_or(0)
            .min(self.pull_requests.len().saturating_sub(1));

        let selected_head_unchanged =
            previous_selected
                .as_ref()
                .is_some_and(|(number, previous_head_sha)| {
                    self.selected_pull_request().is_some_and(|pull_request| {
                        pull_request.number == *number
                            && pull_request.head_sha == *previous_head_sha
                    })
                });

        if !same_inbox || !selected_head_unchanged {
            self.clear_changed_file_state();
            self.clear_workflow_state();
            self.clear_review_data_state();
            self.clear_detail_loaded_state();
            self.clear_review_submission_errors();
            self.clear_log_content();
            self.reset_diff_selection();
            self.reset_detail_scrolls();
        }

        self.pr_list_scroll
            .scroll_to_item(self.selected_pr, ScrollStrategy::Center);

        if load_selected_detail && (!same_inbox || !selected_head_unchanged) {
            self.load_selected_pull_request(cx);
        } else {
            self.cache_current_pull_request_inbox_snapshot();
        }
    }
}

struct RepositoryLoad {
    store: SqliteStore,
    repositories: Vec<RecentRepository>,
    last_selected_repository: Option<RepoId>,
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

    let last_selected_repository = if configured_repo.is_none() {
        store.last_selected_repository().await?
    } else {
        configured_repo
    };
    let repositories = store.recent_repositories().await?;

    Ok(RepositoryLoad {
        store,
        repositories,
        last_selected_repository,
    })
}

async fn refresh_repository_store(
    store: SqliteStore,
    github_api: std::sync::Arc<dyn crate::workspace::github_service::GitHubApi>,
) -> std::result::Result<RepositoryRefresh, StorageError> {
    let repository_error = match github_api.list_repositories().await {
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
