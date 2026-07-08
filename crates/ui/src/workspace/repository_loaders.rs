use std::sync::Arc;

use gpui::{AppContext, Context};
use harbor_domain::RepoId;
use harbor_storage::{RecentRepository, SqliteStore, StorageConfig, StorageError};

use crate::workspace::{AppView, async_updates::AppViewAsyncUpdateExt, github_service::GitHubApi};

const RECENT_REPOSITORY_SWITCHER_LIMIT: usize = 200;

impl AppView {
    pub(super) fn load_recent_repositories(&mut self, cx: &mut Context<Self>) {
        let configured_repo = self.repository_state.configured_repo_cloned();
        self.repository_state.start_loading();
        let task = cx.background_spawn(async move { load_repository_store(configured_repo).await });

        self.tasks.set_repository_task(cx.spawn(async move |this, cx| {
            let result = task.await;

            this.update_or_log(cx, "failed to update repository store state", move |view, cx| {
                match result {
                    Ok(load) => {
                        let repository_count = load.repositories.len();
                        let last_selected_repository = load.last_selected_repository.clone();
                        let store = load.store.clone();
                        view.repository_state.set_store(load.store);

                        if !view.github_api.has_auth() {
                            view.show_github_sign_in_required();
                            cx.notify();
                            return;
                        }

                        view.apply_recent_repositories(load.repositories);
                        if let Some(repository) = view.repository_state.configured_repo_cloned() {
                            view.record_recent_repository(repository, cx);
                        } else if let Some(repository) = last_selected_repository {
                            view.status =
                                format!("Opening last repository {}", repository.full_name());
                            view.load_repository_pull_requests_from_cache(
                                repository,
                                view.pull_request_inbox.mode(),
                                cx,
                            );
                        } else if !view.repository_state.has_configured_repo()
                            && !view.pull_request_inbox.is_loading()
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
                        view.repository_state.clear_store_with_error(error.to_string());
                        view.status = "Failed to initialize repository storage".to_string();
                    }
                }

                cx.notify();
            });
        }));
    }

    fn refresh_repositories_from_github(&mut self, store: SqliteStore, cx: &mut Context<Self>) {
        self.repository_state.start_loading();
        let github_api = self.github_api.clone();
        let task =
            cx.background_spawn(async move { refresh_repository_store(store, github_api).await });

        self.tasks.set_repository_task(cx.spawn(async move |this, cx| {
            let result = task.await;

            this.update_or_log(
                cx,
                "failed to update repository refresh state",
                move |view, cx| {
                    view.repository_state.finish_loading();

                    match result {
                        Ok(load) => {
                            let repository_count = load.repositories.len();
                            let repository_error = load.repository_error.clone();
                            let repository_notice = load.repository_notice.clone();
                            if let Some(error) = load.repository_error {
                                view.repository_state.set_error(error);
                            } else if let Some(notice) = load.repository_notice {
                                view.repository_state.set_notice(notice);
                            } else {
                                view.repository_state.clear_error();
                                view.repository_state.clear_notice();
                            }
                            view.apply_recent_repositories(load.repositories);

                            if !view.repository_state.has_configured_repo()
                                && !view.pull_request_inbox.is_loading()
                                && view.pull_requests.is_empty()
                            {
                                view.status =
                                    match (repository_count, repository_error, repository_notice) {
                                        (0, Some(error), _) => error,
                                        (0, None, _) => {
                                            "No repositories found. Type owner/repo to open a repository"
                                                .to_string()
                                        }
                                        (count, Some(_), _) => {
                                            format!(
                                                "Loaded {count} cached repositories; GitHub refresh failed. Choose one from the header or type owner/repo"
                                            )
                                        }
                                        (count, None, Some(_)) => {
                                            format!(
                                                "Loaded {count} cached repositories. Type owner/repo to open another repository"
                                            )
                                        }
                                        (count, None, None) => {
                                            format!(
                                                "Loaded {count} repositories. Choose one from the header or type owner/repo"
                                            )
                                        }
                                    };
                            }
                        }
                        Err(error) => {
                            view.repository_state.set_error(error.to_string());
                            if !view.repository_state.has_configured_repo()
                                && !view.pull_request_inbox.is_loading()
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

        let Some(store) = self.repository_state.store() else {
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
                            view.repository_state.clear_error();
                        }
                        Err(error) => {
                            view.repository_state.set_error(error.to_string());
                        }
                    }

                    cx.notify();
                },
            );
        })
        .detach();
    }

    pub(crate) fn open_typed_repository_from_switcher(
        &mut self,
        repository: RepoId,
        cx: &mut Context<Self>,
    ) {
        if !self.github_api.has_auth() {
            self.show_github_sign_in_required();
            cx.notify();
            return;
        }

        let github_api = self.github_api.clone();
        let requested_repository = repository.clone();
        self.repository_state.start_loading();
        self.status = format!("Opening repository {}", repository.full_name());
        let task = cx.background_spawn(async move { github_api.get_repository(&repository).await });

        self.tasks
            .set_repository_task(cx.spawn(async move |this, cx| {
                let result = task.await;

                this.update_or_log(
                    cx,
                    "failed to update repository lookup state",
                    move |view, cx| {
                        view.repository_state.finish_loading();
                        match result {
                            Ok(repository) => {
                                view.repository_state.clear_error();
                                view.repository_state.clear_notice();
                                view.load_pull_requests(repository, cx);
                            }
                            Err(error) => {
                                let repository = requested_repository.full_name();
                                let error =
                                    format!("failed to open repository {repository}: {error}");
                                view.repository_state.set_error(error.clone());
                                view.status = error;
                            }
                        }

                        cx.notify();
                    },
                );
            }));
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
    repository_notice: Option<String>,
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
    let repositories = store
        .recent_repositories_limited(RECENT_REPOSITORY_SWITCHER_LIMIT)
        .await?;

    Ok(RepositoryLoad {
        store,
        repositories,
        last_selected_repository,
    })
}

async fn refresh_repository_store(
    store: SqliteStore,
    github_api: Arc<dyn GitHubApi>,
) -> std::result::Result<RepositoryRefresh, StorageError> {
    let mut repository_notice = None;
    let repository_error = match github_api.list_repositories().await {
        Ok(repository_list) => {
            let refreshed_repository_count = repository_list.repositories.len();
            if repository_list.possibly_limited {
                repository_notice = Some(format!(
                    "Refreshed latest {refreshed_repository_count} repositories from GitHub. Cached repositories may also appear."
                ));
            }
            store
                .sync_repositories(&repository_list.repositories)
                .await?;
            None
        }
        Err(error) => Some(format!("failed to load repositories from GitHub: {error}")),
    };

    let repositories = store
        .recent_repositories_limited(RECENT_REPOSITORY_SWITCHER_LIMIT)
        .await?;

    Ok(RepositoryRefresh {
        repositories,
        repository_error,
        repository_notice,
    })
}
