use harbor_domain::{
    CheckRun, DiffFile, PullRequest, PullRequestReview, RepoId, ReviewThread, WorkflowRun,
};
use serde::{Serialize, de::DeserializeOwned};
use sqlx::{
    Row, Sqlite, SqlitePool, Transaction,
    sqlite::{SqliteConnectOptions, SqlitePoolOptions},
};
use thiserror::Error;

mod rows;
mod schema;
mod types;

pub use types::{
    PullRequestDetailSection, RecentRepository, StorageConfig, StoredHttpCacheValidator,
    SyncTargetState,
};

pub type Result<T> = std::result::Result<T, StorageError>;

use rows::{recent_repositories_from_rows, sync_target_state_from_row};

#[derive(Debug, Error)]
pub enum StorageError {
    #[error("storage has not been initialized")]
    NotInitialized,
    #[error("storage operation failed: {0}")]
    Operation(String),
}

impl From<sqlx::Error> for StorageError {
    fn from(error: sqlx::Error) -> Self {
        Self::Operation(error.to_string())
    }
}

impl From<serde_json::Error> for StorageError {
    fn from(error: serde_json::Error) -> Self {
        Self::Operation(error.to_string())
    }
}

#[derive(Clone)]
pub struct SqliteStore {
    pool: SqlitePool,
}

impl SqliteStore {
    pub async fn connect(config: StorageConfig) -> Result<Self> {
        if let Some(parent) = config.database_path.parent() {
            smol::fs::create_dir_all(parent).await.map_err(|error| {
                StorageError::Operation(format!("failed to create storage directory: {error}"))
            })?;
        }

        let options = SqliteConnectOptions::new()
            .filename(&config.database_path)
            .create_if_missing(true);
        let pool = SqlitePoolOptions::new()
            .max_connections(4)
            .connect_with(options)
            .await?;
        let store = Self { pool };
        store.migrate().await?;
        Ok(store)
    }

    pub async fn recent_repositories(&self) -> Result<Vec<RecentRepository>> {
        let rows = sqlx::query(
            "SELECT owner, name, pinned, local_path
             FROM recent_repositories
             ORDER BY
                pinned DESC,
                last_opened_at DESC,
                last_seen_at DESC,
                last_seen_position ASC,
                owner ASC,
                name ASC",
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(recent_repositories_from_rows(rows))
    }

    pub async fn recent_repositories_limited(&self, limit: usize) -> Result<Vec<RecentRepository>> {
        let rows = sqlx::query(
            "SELECT owner, name, pinned, local_path
             FROM recent_repositories
             ORDER BY
                pinned DESC,
                last_opened_at DESC,
                last_seen_at DESC,
                last_seen_position ASC,
                owner ASC,
                name ASC
             LIMIT ?1",
        )
        .bind(limit as i64)
        .fetch_all(&self.pool)
        .await?;

        Ok(recent_repositories_from_rows(rows))
    }

    pub async fn record_repository(&self, repository: &RepoId) -> Result<()> {
        sqlx::query(
            "INSERT INTO recent_repositories (owner, name, pinned, last_opened_at)
             VALUES (?1, ?2, 0, unixepoch())
             ON CONFLICT(owner, name) DO UPDATE SET last_opened_at = unixepoch()",
        )
        .bind(&repository.owner)
        .bind(&repository.name)
        .execute(&self.pool)
        .await?;

        self.record_last_selected_repository(repository).await?;

        Ok(())
    }

    pub async fn last_selected_repository(&self) -> Result<Option<RepoId>> {
        let row = sqlx::query(
            "SELECT owner, name
             FROM last_selected_repository
             WHERE id = 1",
        )
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(|row| RepoId::new(row.get::<String, _>("owner"), row.get::<String, _>("name"))))
    }

    pub async fn sync_repositories(&self, repositories: &[RepoId]) -> Result<()> {
        for (position, repository) in repositories.iter().enumerate() {
            sqlx::query(
                "INSERT INTO recent_repositories
                    (owner, name, pinned, last_opened_at, last_seen_at, last_seen_position)
                 VALUES (?1, ?2, 0, 0, unixepoch(), ?3)
                 ON CONFLICT(owner, name) DO UPDATE SET
                    last_seen_at = unixepoch(),
                    last_seen_position = excluded.last_seen_position",
            )
            .bind(&repository.owner)
            .bind(&repository.name)
            .bind(position as i64)
            .execute(&self.pool)
            .await?;
        }

        Ok(())
    }

    async fn record_last_selected_repository(&self, repository: &RepoId) -> Result<()> {
        sqlx::query(
            "INSERT INTO last_selected_repository (id, owner, name, updated_at)
             VALUES (1, ?1, ?2, unixepoch())
             ON CONFLICT(id) DO UPDATE SET
                owner = excluded.owner,
                name = excluded.name,
                updated_at = excluded.updated_at",
        )
        .bind(&repository.owner)
        .bind(&repository.name)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn set_repository_local_path(
        &self,
        repository: &RepoId,
        local_path: &std::path::Path,
    ) -> Result<()> {
        sqlx::query(
            "INSERT INTO recent_repositories (owner, name, pinned, last_opened_at, local_path)
             VALUES (?1, ?2, 0, unixepoch(), ?3)
             ON CONFLICT(owner, name) DO UPDATE SET
                local_path = excluded.local_path,
                last_opened_at = unixepoch()",
        )
        .bind(&repository.owner)
        .bind(&repository.name)
        .bind(local_path.to_string_lossy().as_ref())
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn load_pull_request_inbox(
        &self,
        repository: &RepoId,
        mode: &str,
    ) -> Result<Vec<PullRequest>> {
        let rows = sqlx::query(
            "SELECT pr_json
             FROM pull_request_inbox_cache
             WHERE owner = ?1 AND name = ?2 AND mode = ?3
             ORDER BY position ASC",
        )
        .bind(&repository.owner)
        .bind(&repository.name)
        .bind(mode)
        .fetch_all(&self.pool)
        .await?;

        rows.into_iter()
            .map(|row| {
                let json = row.get::<String, _>("pr_json");
                serde_json::from_str(&json).map_err(StorageError::from)
            })
            .collect()
    }

    pub async fn save_pull_request_inbox(
        &self,
        repository: &RepoId,
        mode: &str,
        pull_requests: &[PullRequest],
    ) -> Result<()> {
        let mut transaction = self.pool.begin().await?;

        sqlx::query(
            "DELETE FROM pull_request_inbox_cache
             WHERE owner = ?1 AND name = ?2 AND mode = ?3",
        )
        .bind(&repository.owner)
        .bind(&repository.name)
        .bind(mode)
        .execute(&mut *transaction)
        .await?;

        for (position, pull_request) in pull_requests.iter().enumerate() {
            sqlx::query(
                "INSERT INTO pull_request_inbox_cache
                    (owner, name, mode, number, head_sha, position, pr_json, fetched_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, unixepoch())",
            )
            .bind(&repository.owner)
            .bind(&repository.name)
            .bind(mode)
            .bind(pull_request.number as i64)
            .bind(&pull_request.head_sha)
            .bind(position as i64)
            .bind(serde_json::to_string(pull_request)?)
            .execute(&mut *transaction)
            .await?;
        }

        Self::record_sync_success_in_transaction(
            &mut transaction,
            &inbox_target_key(repository, mode),
        )
        .await?;
        transaction.commit().await?;

        Ok(())
    }

    pub async fn save_pull_request_metadata(&self, pull_request: &PullRequest) -> Result<()> {
        self.save_pull_request_detail_section(
            &pull_request.repo,
            pull_request.number,
            &pull_request.head_sha,
            PullRequestDetailSection::Metadata,
            pull_request,
        )
        .await
    }

    pub async fn load_pull_request_metadata(
        &self,
        repository: &RepoId,
        number: u64,
        head_sha: &str,
    ) -> Result<Option<PullRequest>> {
        self.load_pull_request_detail_section(
            repository,
            number,
            head_sha,
            PullRequestDetailSection::Metadata,
        )
        .await
    }

    pub async fn save_pull_request_files(
        &self,
        repository: &RepoId,
        number: u64,
        head_sha: &str,
        files: &[DiffFile],
    ) -> Result<()> {
        self.save_pull_request_detail_section(
            repository,
            number,
            head_sha,
            PullRequestDetailSection::Files,
            files,
        )
        .await
    }

    pub async fn load_pull_request_files(
        &self,
        repository: &RepoId,
        number: u64,
        head_sha: &str,
    ) -> Result<Option<Vec<DiffFile>>> {
        self.load_pull_request_detail_section(
            repository,
            number,
            head_sha,
            PullRequestDetailSection::Files,
        )
        .await
    }

    pub async fn save_pull_request_reviews(
        &self,
        repository: &RepoId,
        number: u64,
        head_sha: &str,
        reviews: &[PullRequestReview],
        threads: &[ReviewThread],
    ) -> Result<()> {
        let mut transaction = self.pool.begin().await?;

        Self::save_pull_request_detail_section_in_transaction(
            &mut transaction,
            repository,
            number,
            head_sha,
            PullRequestDetailSection::Reviews,
            reviews,
        )
        .await?;
        Self::record_sync_success_in_transaction(
            &mut transaction,
            &detail_target_key(repository, number, PullRequestDetailSection::Reviews),
        )
        .await?;
        Self::save_pull_request_detail_section_in_transaction(
            &mut transaction,
            repository,
            number,
            head_sha,
            PullRequestDetailSection::ReviewThreads,
            threads,
        )
        .await?;
        Self::record_sync_success_in_transaction(
            &mut transaction,
            &detail_target_key(repository, number, PullRequestDetailSection::ReviewThreads),
        )
        .await?;
        transaction.commit().await?;

        Ok(())
    }

    pub async fn load_pull_request_reviews(
        &self,
        repository: &RepoId,
        number: u64,
        head_sha: &str,
    ) -> Result<Option<(Vec<PullRequestReview>, Vec<ReviewThread>)>> {
        let reviews = self
            .load_pull_request_detail_section::<Vec<PullRequestReview>>(
                repository,
                number,
                head_sha,
                PullRequestDetailSection::Reviews,
            )
            .await?;
        let threads = self
            .load_pull_request_detail_section::<Vec<ReviewThread>>(
                repository,
                number,
                head_sha,
                PullRequestDetailSection::ReviewThreads,
            )
            .await?;

        Ok(match (reviews, threads) {
            (Some(reviews), Some(threads)) => Some((reviews, threads)),
            _ => None,
        })
    }

    pub async fn save_pull_request_check_runs(
        &self,
        repository: &RepoId,
        number: u64,
        head_sha: &str,
        check_runs: &[CheckRun],
    ) -> Result<()> {
        self.save_pull_request_detail_section(
            repository,
            number,
            head_sha,
            PullRequestDetailSection::CheckRuns,
            check_runs,
        )
        .await
    }

    pub async fn load_pull_request_check_runs(
        &self,
        repository: &RepoId,
        number: u64,
        head_sha: &str,
    ) -> Result<Option<Vec<CheckRun>>> {
        self.load_pull_request_detail_section(
            repository,
            number,
            head_sha,
            PullRequestDetailSection::CheckRuns,
        )
        .await
    }

    pub async fn save_pull_request_workflow_runs(
        &self,
        repository: &RepoId,
        number: u64,
        head_sha: &str,
        workflow_runs: &[WorkflowRun],
    ) -> Result<()> {
        self.save_pull_request_detail_section(
            repository,
            number,
            head_sha,
            PullRequestDetailSection::WorkflowRuns,
            workflow_runs,
        )
        .await
    }

    pub async fn load_pull_request_workflow_runs(
        &self,
        repository: &RepoId,
        number: u64,
        head_sha: &str,
    ) -> Result<Option<Vec<WorkflowRun>>> {
        self.load_pull_request_detail_section(
            repository,
            number,
            head_sha,
            PullRequestDetailSection::WorkflowRuns,
        )
        .await
    }

    pub async fn mark_sync_attempt(&self, target_key: &str) -> Result<()> {
        sqlx::query(
            "INSERT INTO sync_target_state (target_key, last_attempt_at, stale)
             VALUES (?1, unixepoch(), 0)
             ON CONFLICT(target_key) DO UPDATE SET
                last_attempt_at = unixepoch()",
        )
        .bind(target_key)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn record_sync_success(&self, target_key: &str) -> Result<()> {
        sqlx::query(
            "INSERT INTO sync_target_state
                (target_key, last_successful_fetch_at, last_attempt_at, last_error, stale)
             VALUES (?1, unixepoch(), unixepoch(), NULL, 0)
             ON CONFLICT(target_key) DO UPDATE SET
                last_successful_fetch_at = unixepoch(),
                last_attempt_at = unixepoch(),
                last_error = NULL,
                stale = 0",
        )
        .bind(target_key)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn record_sync_failure(&self, target_key: &str, error: &str) -> Result<()> {
        sqlx::query(
            "INSERT INTO sync_target_state (target_key, last_attempt_at, last_error, stale)
             VALUES (?1, unixepoch(), ?2, 1)
             ON CONFLICT(target_key) DO UPDATE SET
                last_attempt_at = unixepoch(),
                last_error = excluded.last_error,
                stale = 1",
        )
        .bind(target_key)
        .bind(error)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn mark_sync_target_stale(&self, target_key: &str) -> Result<()> {
        sqlx::query(
            "INSERT INTO sync_target_state (target_key, stale)
             VALUES (?1, 1)
             ON CONFLICT(target_key) DO UPDATE SET stale = 1",
        )
        .bind(target_key)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn sync_target_state(&self, target_key: &str) -> Result<Option<SyncTargetState>> {
        let row = sqlx::query(
            "SELECT target_key, last_successful_fetch_at, last_attempt_at, last_error, stale
             FROM sync_target_state
             WHERE target_key = ?1",
        )
        .bind(target_key)
        .fetch_optional(&self.pool)
        .await?;

        row.map(sync_target_state_from_row).transpose()
    }

    pub async fn load_http_cache_validator(
        &self,
        request_key: &str,
    ) -> Result<Option<StoredHttpCacheValidator>> {
        let row = sqlx::query(
            "SELECT etag, last_modified
             FROM http_cache_validators
             WHERE request_key = ?1",
        )
        .bind(request_key)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row
            .map(|row| StoredHttpCacheValidator {
                etag: row.get("etag"),
                last_modified: row.get("last_modified"),
            })
            .filter(|validator| !validator.is_empty()))
    }

    pub async fn save_http_cache_validator(
        &self,
        request_key: &str,
        validator: &StoredHttpCacheValidator,
    ) -> Result<()> {
        if validator.is_empty() {
            return Ok(());
        }

        sqlx::query(
            "INSERT INTO http_cache_validators (request_key, etag, last_modified, updated_at)
             VALUES (?1, ?2, ?3, unixepoch())
             ON CONFLICT(request_key) DO UPDATE SET
                etag = excluded.etag,
                last_modified = excluded.last_modified,
                updated_at = excluded.updated_at",
        )
        .bind(request_key)
        .bind(&validator.etag)
        .bind(&validator.last_modified)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn save_pull_request_detail_section<T>(
        &self,
        repository: &RepoId,
        number: u64,
        head_sha: &str,
        section: PullRequestDetailSection,
        value: &T,
    ) -> Result<()>
    where
        T: Serialize + ?Sized,
    {
        let mut transaction = self.pool.begin().await?;
        Self::save_pull_request_detail_section_in_transaction(
            &mut transaction,
            repository,
            number,
            head_sha,
            section,
            value,
        )
        .await?;
        Self::record_sync_success_in_transaction(
            &mut transaction,
            &detail_target_key(repository, number, section),
        )
        .await?;
        transaction.commit().await?;

        Ok(())
    }

    async fn save_pull_request_detail_section_in_transaction<T>(
        transaction: &mut Transaction<'_, Sqlite>,
        repository: &RepoId,
        number: u64,
        head_sha: &str,
        section: PullRequestDetailSection,
        value: &T,
    ) -> Result<()>
    where
        T: Serialize + ?Sized,
    {
        sqlx::query(
            "INSERT INTO pull_request_detail_cache
                (owner, name, number, head_sha, section, data_json, fetched_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, unixepoch())
             ON CONFLICT(owner, name, number, head_sha, section) DO UPDATE SET
                data_json = excluded.data_json,
                fetched_at = unixepoch()",
        )
        .bind(&repository.owner)
        .bind(&repository.name)
        .bind(number as i64)
        .bind(head_sha)
        .bind(section.key())
        .bind(serde_json::to_string(value)?)
        .execute(&mut **transaction)
        .await?;

        Ok(())
    }

    async fn record_sync_success_in_transaction(
        transaction: &mut Transaction<'_, Sqlite>,
        target_key: &str,
    ) -> Result<()> {
        sqlx::query(
            "INSERT INTO sync_target_state
                (target_key, last_successful_fetch_at, last_attempt_at, last_error, stale)
             VALUES (?1, unixepoch(), unixepoch(), NULL, 0)
             ON CONFLICT(target_key) DO UPDATE SET
                last_successful_fetch_at = unixepoch(),
                last_attempt_at = unixepoch(),
                last_error = NULL,
                stale = 0",
        )
        .bind(target_key)
        .execute(&mut **transaction)
        .await?;

        Ok(())
    }

    async fn load_pull_request_detail_section<T>(
        &self,
        repository: &RepoId,
        number: u64,
        head_sha: &str,
        section: PullRequestDetailSection,
    ) -> Result<Option<T>>
    where
        T: DeserializeOwned,
    {
        let row = sqlx::query(
            "SELECT data_json
             FROM pull_request_detail_cache
             WHERE owner = ?1 AND name = ?2 AND number = ?3 AND head_sha = ?4 AND section = ?5",
        )
        .bind(&repository.owner)
        .bind(&repository.name)
        .bind(number as i64)
        .bind(head_sha)
        .bind(section.key())
        .fetch_optional(&self.pool)
        .await?;

        row.map(|row| {
            let json = row.get::<String, _>("data_json");
            serde_json::from_str(&json).map_err(StorageError::from)
        })
        .transpose()
    }
}

pub fn inbox_target_key(repository: &RepoId, mode: &str) -> String {
    format!("inbox:{}:{}", repository.full_name(), mode)
}

pub fn detail_target_key(
    repository: &RepoId,
    number: u64,
    section: PullRequestDetailSection,
) -> String {
    format!("pr:{}#{}:{}", repository.full_name(), number, section.key())
}

#[cfg(test)]
mod tests;
