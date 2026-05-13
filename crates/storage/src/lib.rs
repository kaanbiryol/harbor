use std::path::PathBuf;

use chrono::{DateTime, Utc};
use harbor_domain::{
    CheckRun, DiffFile, PullRequest, PullRequestReview, RepoId, ReviewThread, WorkflowRun,
};
use serde::{Serialize, de::DeserializeOwned};
use sqlx::{
    Row, Sqlite, SqlitePool, Transaction,
    sqlite::{SqliteConnectOptions, SqlitePoolOptions, SqliteRow},
};
use thiserror::Error;

pub type Result<T> = std::result::Result<T, StorageError>;

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

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StorageConfig {
    pub database_path: PathBuf,
}

impl StorageConfig {
    pub fn from_env() -> Result<Self> {
        if let Ok(path) = std::env::var("HARBOR_DATABASE_PATH") {
            return Ok(Self {
                database_path: PathBuf::from(path),
            });
        }

        Ok(Self {
            database_path: default_database_path()?,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RecentRepository {
    pub id: RepoId,
    pub pinned: bool,
    pub local_path: Option<PathBuf>,
}

fn recent_repositories_from_rows(rows: Vec<SqliteRow>) -> Vec<RecentRepository> {
    rows.into_iter()
        .map(|row| RecentRepository {
            id: RepoId::new(row.get::<String, _>("owner"), row.get::<String, _>("name")),
            pinned: row.get::<i64, _>("pinned") != 0,
            local_path: row
                .get::<Option<String>, _>("local_path")
                .map(PathBuf::from),
        })
        .collect()
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PullRequestDetailSection {
    Metadata,
    Files,
    Reviews,
    ReviewThreads,
    CheckRuns,
    WorkflowRuns,
}

impl PullRequestDetailSection {
    pub fn key(self) -> &'static str {
        match self {
            Self::Metadata => "metadata",
            Self::Files => "files",
            Self::Reviews => "reviews",
            Self::ReviewThreads => "review_threads",
            Self::CheckRuns => "check_runs",
            Self::WorkflowRuns => "workflow_runs",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SyncTargetState {
    pub target_key: String,
    pub last_successful_fetch_at: Option<DateTime<Utc>>,
    pub last_attempt_at: Option<DateTime<Utc>>,
    pub last_error: Option<String>,
    pub stale: bool,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct StoredHttpCacheValidator {
    pub etag: Option<String>,
    pub last_modified: Option<String>,
}

impl StoredHttpCacheValidator {
    pub fn is_empty(&self) -> bool {
        self.etag.is_none() && self.last_modified.is_none()
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

    async fn migrate(&self) -> Result<()> {
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS schema_migrations (
                version INTEGER PRIMARY KEY,
                applied_at INTEGER NOT NULL
            )",
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS recent_repositories (
                owner TEXT NOT NULL,
                name TEXT NOT NULL,
                pinned INTEGER NOT NULL DEFAULT 0,
                last_opened_at INTEGER NOT NULL DEFAULT (unixepoch()),
                last_seen_at INTEGER NOT NULL DEFAULT 0,
                last_seen_position INTEGER NOT NULL DEFAULT 0,
                local_path TEXT,
                PRIMARY KEY (owner, name)
            )",
        )
        .execute(&self.pool)
        .await?;

        let columns = sqlx::query("PRAGMA table_info(recent_repositories)")
            .fetch_all(&self.pool)
            .await?;
        let has_local_path = columns
            .iter()
            .any(|row| row.get::<String, _>("name") == "local_path");
        let has_last_seen_at = columns
            .iter()
            .any(|row| row.get::<String, _>("name") == "last_seen_at");
        let has_last_seen_position = columns
            .iter()
            .any(|row| row.get::<String, _>("name") == "last_seen_position");

        if !has_local_path {
            sqlx::query("ALTER TABLE recent_repositories ADD COLUMN local_path TEXT")
                .execute(&self.pool)
                .await?;
        }
        if !has_last_seen_at {
            sqlx::query(
                "ALTER TABLE recent_repositories ADD COLUMN last_seen_at INTEGER NOT NULL DEFAULT 0",
            )
            .execute(&self.pool)
            .await?;
        }
        if !has_last_seen_position {
            sqlx::query(
                "ALTER TABLE recent_repositories
                 ADD COLUMN last_seen_position INTEGER NOT NULL DEFAULT 0",
            )
            .execute(&self.pool)
            .await?;
        }

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS last_selected_repository (
                id INTEGER PRIMARY KEY CHECK (id = 1),
                owner TEXT NOT NULL,
                name TEXT NOT NULL,
                updated_at INTEGER NOT NULL
            )",
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS pull_request_inbox_cache (
                owner TEXT NOT NULL,
                name TEXT NOT NULL,
                mode TEXT NOT NULL,
                number INTEGER NOT NULL,
                head_sha TEXT NOT NULL,
                position INTEGER NOT NULL,
                pr_json TEXT NOT NULL,
                fetched_at INTEGER NOT NULL,
                PRIMARY KEY (owner, name, mode, number)
            )",
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS pull_request_detail_cache (
                owner TEXT NOT NULL,
                name TEXT NOT NULL,
                number INTEGER NOT NULL,
                head_sha TEXT NOT NULL,
                section TEXT NOT NULL,
                data_json TEXT NOT NULL,
                fetched_at INTEGER NOT NULL,
                PRIMARY KEY (owner, name, number, head_sha, section)
            )",
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS sync_target_state (
                target_key TEXT PRIMARY KEY,
                last_successful_fetch_at INTEGER,
                last_attempt_at INTEGER,
                last_error TEXT,
                stale INTEGER NOT NULL DEFAULT 0
            )",
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS http_cache_validators (
                request_key TEXT PRIMARY KEY,
                etag TEXT,
                last_modified TEXT,
                updated_at INTEGER NOT NULL
            )",
        )
        .execute(&self.pool)
        .await?;

        self.record_schema_migration(1).await?;

        Ok(())
    }

    async fn record_schema_migration(&self, version: i64) -> Result<()> {
        sqlx::query(
            "INSERT INTO schema_migrations (version, applied_at)
             VALUES (?1, unixepoch())
             ON CONFLICT(version) DO NOTHING",
        )
        .bind(version)
        .execute(&self.pool)
        .await?;

        Ok(())
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

fn sync_target_state_from_row(row: sqlx::sqlite::SqliteRow) -> Result<SyncTargetState> {
    Ok(SyncTargetState {
        target_key: row.get("target_key"),
        last_successful_fetch_at: unix_timestamp_to_datetime(
            row.get::<Option<i64>, _>("last_successful_fetch_at"),
        )?,
        last_attempt_at: unix_timestamp_to_datetime(row.get::<Option<i64>, _>("last_attempt_at"))?,
        last_error: row.get("last_error"),
        stale: row.get::<i64, _>("stale") != 0,
    })
}

fn unix_timestamp_to_datetime(timestamp: Option<i64>) -> Result<Option<DateTime<Utc>>> {
    timestamp
        .map(|timestamp| {
            DateTime::<Utc>::from_timestamp(timestamp, 0).ok_or_else(|| {
                StorageError::Operation(format!("invalid unix timestamp {timestamp}"))
            })
        })
        .transpose()
}

fn default_database_path() -> Result<PathBuf> {
    if let Ok(data_home) = std::env::var("XDG_DATA_HOME") {
        return Ok(PathBuf::from(data_home)
            .join("harbor")
            .join("harbor.sqlite"));
    }

    let home = std::env::var("HOME")
        .map_err(|_| StorageError::Operation("HOME is not set".to_string()))?;

    #[cfg(target_os = "macos")]
    {
        Ok(PathBuf::from(home)
            .join("Library")
            .join("Application Support")
            .join("Harbor")
            .join("harbor.sqlite"))
    }

    #[cfg(not(target_os = "macos"))]
    {
        Ok(PathBuf::from(home)
            .join(".local")
            .join("share")
            .join("harbor")
            .join("harbor.sqlite"))
    }
}

#[cfg(test)]
mod tests {
    use std::time::{SystemTime, UNIX_EPOCH};

    use harbor_domain::{ChecksSummary, MergeState, PullRequest, PullRequestState, ReviewDecision};

    use super::*;

    #[test]
    fn syncs_repositories_without_marking_them_recent() {
        smol::block_on(async {
            let database_path = test_database_path("syncs-repositories");
            let store = SqliteStore::connect(StorageConfig {
                database_path: database_path.clone(),
            })
            .await
            .expect("connect sqlite store");
            let repository = RepoId::new("acme", "app");
            let old_repository = RepoId::new("acme", "old");

            store
                .sync_repositories(&[repository.clone(), old_repository.clone()])
                .await
                .expect("sync repositories");
            store
                .record_repository(&repository)
                .await
                .expect("record repository");

            let repositories = store
                .recent_repositories()
                .await
                .expect("load recent repositories");

            assert_eq!(repositories.len(), 2);
            assert_eq!(repositories[0].id, repository);
            assert!(!repositories[0].pinned);
            assert_eq!(repositories[0].local_path, None);
            assert_eq!(repositories[1].id, old_repository);

            cleanup_database(database_path);
        });
    }

    #[test]
    fn limits_recent_repository_results() {
        smol::block_on(async {
            let database_path = test_database_path("limits-repositories");
            let store = SqliteStore::connect(StorageConfig {
                database_path: database_path.clone(),
            })
            .await
            .expect("connect sqlite store");

            store
                .sync_repositories(&[
                    RepoId::new("acme", "one"),
                    RepoId::new("acme", "two"),
                    RepoId::new("acme", "three"),
                ])
                .await
                .expect("sync repositories");

            let repositories = store
                .recent_repositories_limited(2)
                .await
                .expect("load limited repositories");

            assert_eq!(repositories.len(), 2);

            cleanup_database(database_path);
        });
    }

    #[test]
    fn latest_repository_sync_takes_priority_over_stale_synced_rows() {
        smol::block_on(async {
            let database_path = test_database_path("latest-sync-priority");
            let store = SqliteStore::connect(StorageConfig {
                database_path: database_path.clone(),
            })
            .await
            .expect("connect sqlite store");
            let stale_repository = RepoId::new("aaa", "stale");
            let latest_repository = RepoId::new("zzz", "latest");

            store
                .sync_repositories(std::slice::from_ref(&stale_repository))
                .await
                .expect("sync stale repository");
            sqlx::query("UPDATE recent_repositories SET last_seen_at = 1")
                .execute(&store.pool)
                .await
                .expect("age synced repositories");
            store
                .sync_repositories(std::slice::from_ref(&latest_repository))
                .await
                .expect("sync latest repository");

            let repositories = store
                .recent_repositories_limited(1)
                .await
                .expect("load limited repositories");

            assert_eq!(repositories[0].id, latest_repository);

            cleanup_database(database_path);
        });
    }

    #[test]
    fn records_last_selected_repository_when_repository_is_opened() {
        smol::block_on(async {
            let database_path = test_database_path("last-selected-repository");
            let store = SqliteStore::connect(StorageConfig {
                database_path: database_path.clone(),
            })
            .await
            .expect("connect sqlite store");
            let first_repository = RepoId::new("acme", "app");
            let second_repository = RepoId::new("zed", "editor");

            assert_eq!(
                store
                    .last_selected_repository()
                    .await
                    .expect("load empty last selected repository"),
                None
            );

            store
                .record_repository(&first_repository)
                .await
                .expect("record first repository");
            assert_eq!(
                store
                    .last_selected_repository()
                    .await
                    .expect("load first last selected repository"),
                Some(first_repository)
            );

            store
                .record_repository(&second_repository)
                .await
                .expect("record second repository");
            assert_eq!(
                store
                    .last_selected_repository()
                    .await
                    .expect("load second last selected repository"),
                Some(second_repository)
            );

            cleanup_database(database_path);
        });
    }

    #[test]
    fn syncing_repositories_does_not_replace_last_selected_repository() {
        smol::block_on(async {
            let database_path = test_database_path("sync-keeps-last-selected-repository");
            let store = SqliteStore::connect(StorageConfig {
                database_path: database_path.clone(),
            })
            .await
            .expect("connect sqlite store");
            let selected_repository = RepoId::new("acme", "app");
            let synced_repository = RepoId::new("zed", "editor");

            store
                .record_repository(&selected_repository)
                .await
                .expect("record selected repository");
            store
                .sync_repositories(std::slice::from_ref(&synced_repository))
                .await
                .expect("sync repositories");

            assert_eq!(
                store
                    .last_selected_repository()
                    .await
                    .expect("load last selected repository"),
                Some(selected_repository)
            );

            cleanup_database(database_path);
        });
    }

    #[test]
    fn saves_repository_local_path() {
        smol::block_on(async {
            let database_path = test_database_path("saves-local-path");
            let store = SqliteStore::connect(StorageConfig {
                database_path: database_path.clone(),
            })
            .await
            .expect("connect sqlite store");
            let repository = RepoId::new("acme", "app");
            let local_path = PathBuf::from("/tmp/acme-app");

            store
                .set_repository_local_path(&repository, &local_path)
                .await
                .expect("save local path");

            let repositories = store
                .recent_repositories()
                .await
                .expect("load recent repositories");

            assert_eq!(repositories.len(), 1);
            assert_eq!(repositories[0].id, repository);
            assert_eq!(
                repositories[0].local_path.as_deref(),
                Some(local_path.as_path())
            );

            cleanup_database(database_path);
        });
    }

    #[test]
    fn migrates_existing_repository_table_for_local_path() {
        smol::block_on(async {
            let database_path = test_database_path("migrates-local-path");
            std::fs::create_dir_all(database_path.parent().expect("database parent"))
                .expect("create database parent");
            let options = SqliteConnectOptions::new()
                .filename(&database_path)
                .create_if_missing(true);
            let pool = SqlitePoolOptions::new()
                .max_connections(1)
                .connect_with(options)
                .await
                .expect("connect old schema");

            sqlx::query(
                "CREATE TABLE recent_repositories (
                    owner TEXT NOT NULL,
                    name TEXT NOT NULL,
                    pinned INTEGER NOT NULL DEFAULT 0,
                    last_opened_at INTEGER NOT NULL DEFAULT 0,
                    PRIMARY KEY (owner, name)
                )",
            )
            .execute(&pool)
            .await
            .expect("create old table");
            sqlx::query(
                "INSERT INTO recent_repositories (owner, name, pinned, last_opened_at)
                 VALUES ('acme', 'app', 0, 0)",
            )
            .execute(&pool)
            .await
            .expect("insert old row");
            pool.close().await;

            let store = SqliteStore::connect(StorageConfig {
                database_path: database_path.clone(),
            })
            .await
            .expect("migrate sqlite store");
            let repositories = store
                .recent_repositories()
                .await
                .expect("load recent repositories");

            assert_eq!(repositories.len(), 1);
            assert_eq!(repositories[0].id, RepoId::new("acme", "app"));
            assert_eq!(repositories[0].local_path, None);
            assert_eq!(
                migration_versions(&store).await.expect("load migrations"),
                vec![1]
            );

            cleanup_database(database_path);
        });
    }

    #[test]
    fn records_initial_schema_migration_for_new_database() {
        smol::block_on(async {
            let database_path = test_database_path("records-schema-migration");
            let store = SqliteStore::connect(StorageConfig {
                database_path: database_path.clone(),
            })
            .await
            .expect("connect sqlite store");

            assert_eq!(
                migration_versions(&store).await.expect("load migrations"),
                vec![1]
            );

            cleanup_database(database_path);
        });
    }

    #[test]
    fn saves_and_loads_pull_request_inbox_cache() {
        smol::block_on(async {
            let database_path = test_database_path("pull-request-inbox-cache");
            let store = SqliteStore::connect(StorageConfig {
                database_path: database_path.clone(),
            })
            .await
            .expect("connect sqlite store");
            let repository = RepoId::new("acme", "app");
            let pull_request = pull_request(7);

            store
                .save_pull_request_inbox(&repository, "open", std::slice::from_ref(&pull_request))
                .await
                .expect("save inbox");

            let cached = store
                .load_pull_request_inbox(&repository, "open")
                .await
                .expect("load inbox");

            assert_eq!(cached, vec![pull_request]);

            cleanup_database(database_path);
        });
    }

    #[test]
    fn replaces_absent_pull_requests_for_cached_mode() {
        smol::block_on(async {
            let database_path = test_database_path("replace-pull-request-inbox-cache");
            let store = SqliteStore::connect(StorageConfig {
                database_path: database_path.clone(),
            })
            .await
            .expect("connect sqlite store");
            let repository = RepoId::new("acme", "app");

            store
                .save_pull_request_inbox(&repository, "open", &[pull_request(7), pull_request(8)])
                .await
                .expect("save initial inbox");
            store
                .save_pull_request_inbox(&repository, "open", &[pull_request(8)])
                .await
                .expect("replace inbox");

            let cached = store
                .load_pull_request_inbox(&repository, "open")
                .await
                .expect("load inbox");

            assert_eq!(cached.len(), 1);
            assert_eq!(cached[0].number, 8);

            cleanup_database(database_path);
        });
    }

    #[test]
    fn failed_inbox_replacement_keeps_existing_cache() {
        smol::block_on(async {
            let database_path = test_database_path("inbox-rollback");
            let store = SqliteStore::connect(StorageConfig {
                database_path: database_path.clone(),
            })
            .await
            .expect("connect sqlite store");
            let repository = RepoId::new("acme", "app");
            let original_pull_request = pull_request(7);

            store
                .save_pull_request_inbox(
                    &repository,
                    "open",
                    std::slice::from_ref(&original_pull_request),
                )
                .await
                .expect("save original inbox");
            sqlx::query(
                "CREATE TRIGGER fail_inbox_insert
                 BEFORE INSERT ON pull_request_inbox_cache
                 BEGIN
                    SELECT RAISE(FAIL, 'blocked insert');
                 END",
            )
            .execute(&store.pool)
            .await
            .expect("create failing trigger");

            let result = store
                .save_pull_request_inbox(&repository, "open", &[pull_request(8)])
                .await;
            assert!(result.is_err());

            let cached = store
                .load_pull_request_inbox(&repository, "open")
                .await
                .expect("load inbox after failed replacement");
            assert_eq!(cached, vec![original_pull_request]);

            cleanup_database(database_path);
        });
    }

    #[test]
    fn saves_and_loads_detail_sections_independently() {
        smol::block_on(async {
            let database_path = test_database_path("pull-request-detail-cache");
            let store = SqliteStore::connect(StorageConfig {
                database_path: database_path.clone(),
            })
            .await
            .expect("connect sqlite store");
            let pull_request = pull_request(7);

            store
                .save_pull_request_metadata(&pull_request)
                .await
                .expect("save metadata");
            store
                .save_pull_request_check_runs(
                    &pull_request.repo,
                    pull_request.number,
                    &pull_request.head_sha,
                    &[],
                )
                .await
                .expect("save checks");

            let cached_metadata = store
                .load_pull_request_metadata(
                    &pull_request.repo,
                    pull_request.number,
                    &pull_request.head_sha,
                )
                .await
                .expect("load metadata");
            let cached_checks = store
                .load_pull_request_check_runs(
                    &pull_request.repo,
                    pull_request.number,
                    &pull_request.head_sha,
                )
                .await
                .expect("load checks");

            assert_eq!(cached_metadata, Some(pull_request));
            assert_eq!(cached_checks, Some(Vec::new()));

            cleanup_database(database_path);
        });
    }

    #[test]
    fn records_sync_failure_without_dropping_cache() {
        smol::block_on(async {
            let database_path = test_database_path("sync-failure-preserves-cache");
            let store = SqliteStore::connect(StorageConfig {
                database_path: database_path.clone(),
            })
            .await
            .expect("connect sqlite store");
            let repository = RepoId::new("acme", "app");
            let pull_request = pull_request(7);
            let target_key = inbox_target_key(&repository, "open");

            store
                .save_pull_request_inbox(&repository, "open", std::slice::from_ref(&pull_request))
                .await
                .expect("save inbox");
            store
                .record_sync_failure(&target_key, "rate limited")
                .await
                .expect("record failure");

            let cached = store
                .load_pull_request_inbox(&repository, "open")
                .await
                .expect("load inbox");
            let target_state = store
                .sync_target_state(&target_key)
                .await
                .expect("load target state")
                .expect("target state");

            assert_eq!(cached, vec![pull_request]);
            assert_eq!(target_state.last_error.as_deref(), Some("rate limited"));
            assert!(target_state.stale);

            cleanup_database(database_path);
        });
    }

    #[test]
    fn saves_and_loads_http_cache_validator() {
        smol::block_on(async {
            let database_path = test_database_path("http-cache-validator");
            let store = SqliteStore::connect(StorageConfig {
                database_path: database_path.clone(),
            })
            .await
            .expect("connect sqlite store");
            let validator = StoredHttpCacheValidator {
                etag: Some("\"abc\"".to_string()),
                last_modified: Some("Wed, 01 May 2026 10:00:00 GMT".to_string()),
            };

            store
                .save_http_cache_validator("rest:acme/app:open", &validator)
                .await
                .expect("save validator");

            let cached = store
                .load_http_cache_validator("rest:acme/app:open")
                .await
                .expect("load validator");

            assert_eq!(cached, Some(validator));

            cleanup_database(database_path);
        });
    }

    fn test_database_path(name: &str) -> PathBuf {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock before unix epoch")
            .as_nanos();
        std::env::temp_dir()
            .join(format!("harbor-storage-{name}-{suffix}"))
            .join("harbor.sqlite")
    }

    fn cleanup_database(database_path: PathBuf) {
        let Some(directory) = database_path.parent() else {
            return;
        };
        if let Err(error) = std::fs::remove_dir_all(directory) {
            eprintln!("failed to clean up test database: {error}");
        }
    }

    async fn migration_versions(store: &SqliteStore) -> Result<Vec<i64>> {
        let rows = sqlx::query("SELECT version FROM schema_migrations ORDER BY version ASC")
            .fetch_all(&store.pool)
            .await?;

        Ok(rows
            .into_iter()
            .map(|row| row.get::<i64, _>("version"))
            .collect())
    }

    fn pull_request(number: u64) -> PullRequest {
        PullRequest {
            repo: RepoId::new("acme", "app"),
            node_id: format!("pr-node-{number}"),
            number,
            title: "Add feature".to_string(),
            body: None,
            author: "octocat".to_string(),
            url: format!("https://github.com/acme/app/pull/{number}"),
            state: PullRequestState::Open,
            is_draft: false,
            head_ref: "feature".to_string(),
            base_ref: "main".to_string(),
            head_sha: "abc123".to_string(),
            review_decision: Some(ReviewDecision::ReviewRequired),
            merge_state: Some(MergeState::Clean),
            labels: Vec::new(),
            checks_summary: ChecksSummary::default(),
            unresolved_threads: 0,
            updated_at: DateTime::from_timestamp(1_777_777_777, 0),
        }
    }
}
