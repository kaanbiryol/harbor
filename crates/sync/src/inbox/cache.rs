use harbor_domain::RepoId;
use harbor_github::GitHubError;
use harbor_storage::{SqliteStore, StorageError};

use super::{PullRequestInboxMode, PullRequestInboxRefresh};

pub async fn cache_pull_request_inbox_refresh(
    store: Option<&SqliteStore>,
    repository: &RepoId,
    mode: PullRequestInboxMode,
    refresh: &std::result::Result<PullRequestInboxRefresh, GitHubError>,
) -> std::result::Result<(), StorageError> {
    let Some(store) = store else {
        return Ok(());
    };

    match refresh {
        Ok(PullRequestInboxRefresh::Modified { .. }) => {
            store
                .record_sync_success(&harbor_storage::inbox_target_key(repository, mode.key()))
                .await
        }
        Ok(PullRequestInboxRefresh::NotModified) => {
            store
                .record_sync_success(&harbor_storage::inbox_target_key(repository, mode.key()))
                .await
        }
        Err(error) => {
            store
                .record_sync_failure(
                    &harbor_storage::inbox_target_key(repository, mode.key()),
                    &error.to_string(),
                )
                .await
        }
    }
}
