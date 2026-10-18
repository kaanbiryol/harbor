use std::path::Path;

use harbor_domain::{RepoId, preferences::AppPreferences};
use harbor_storage::{RecentRepository, Result, SqliteStore};

const GITHUB_AUTH_SOURCE_SETTING_KEY: &str = "github.auth_source";

pub struct StoredRepositories {
    pub pinned_repositories: Vec<RecentRepository>,
    pub repositories_with_local_paths: Vec<RecentRepository>,
    pub last_selected_repository: Option<RepoId>,
}

pub async fn load_preferences(store: &SqliteStore) -> Result<AppPreferences> {
    store.load_preferences().await
}

pub async fn save_preferences(store: &SqliteStore, preferences: &AppPreferences) -> Result<()> {
    store.save_preferences(preferences).await
}

pub async fn load_github_auth_source(store: &SqliteStore) -> Result<Option<String>> {
    store.load_app_setting(GITHUB_AUTH_SOURCE_SETTING_KEY).await
}

pub async fn save_github_auth_source(store: &SqliteStore, source: &str) -> Result<()> {
    store
        .save_app_setting(GITHUB_AUTH_SOURCE_SETTING_KEY, source)
        .await
}

pub async fn delete_github_auth_source(store: &SqliteStore) -> Result<()> {
    store
        .delete_app_setting(GITHUB_AUTH_SOURCE_SETTING_KEY)
        .await
}

/// Records `configured_repository` when present; otherwise the last selected repository is restored.
pub async fn load_stored_repositories(
    store: &SqliteStore,
    configured_repository: Option<RepoId>,
) -> Result<StoredRepositories> {
    if let Some(repository) = configured_repository.as_ref() {
        store.record_repository(repository).await?;
    }

    let last_selected_repository = if configured_repository.is_none() {
        store.last_selected_repository().await?
    } else {
        configured_repository
    };
    let pinned_repositories = store.pinned_repositories().await?;
    let repositories_with_local_paths = store.repositories_with_local_paths().await?;

    Ok(StoredRepositories {
        pinned_repositories,
        repositories_with_local_paths,
        last_selected_repository,
    })
}

pub async fn record_repository(store: &SqliteStore, repository: &RepoId) -> Result<()> {
    store.record_repository(repository).await
}

pub async fn set_repository_pinned(
    store: &SqliteStore,
    repository: &RepoId,
    pinned: bool,
) -> Result<()> {
    store.set_repository_pinned(repository, pinned).await
}

pub async fn set_repository_local_path(
    store: &SqliteStore,
    repository: &RepoId,
    local_path: &Path,
) -> Result<()> {
    store
        .set_repository_local_path(repository, local_path)
        .await
}
