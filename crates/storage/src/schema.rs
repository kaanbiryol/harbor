use std::collections::HashSet;

use sqlx::{Row, Sqlite, Transaction};

use crate::{Result, SqliteStore};

const INITIAL_SCHEMA_VERSION: i64 = 1;
const REPOSITORY_STATE_VERSION: i64 = 2;

impl SqliteStore {
    pub(super) async fn migrate(&self) -> Result<()> {
        let mut transaction = self.pool.begin().await?;
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS schema_migrations (
                version INTEGER PRIMARY KEY,
                applied_at INTEGER NOT NULL
            )",
        )
        .execute(&mut *transaction)
        .await?;

        let applied_versions = sqlx::query("SELECT version FROM schema_migrations")
            .fetch_all(&mut *transaction)
            .await?
            .into_iter()
            .map(|row| row.get::<i64, _>("version"))
            .collect::<HashSet<_>>();

        if !applied_versions.contains(&INITIAL_SCHEMA_VERSION) {
            Self::apply_initial_schema(&mut transaction).await?;
            Self::record_schema_migration(&mut transaction, INITIAL_SCHEMA_VERSION).await?;
        }
        if !applied_versions.contains(&REPOSITORY_STATE_VERSION) {
            Self::apply_repository_state_migration(&mut transaction).await?;
            Self::record_schema_migration(&mut transaction, REPOSITORY_STATE_VERSION).await?;
        }

        transaction.commit().await?;
        Ok(())
    }

    async fn apply_initial_schema(transaction: &mut Transaction<'_, Sqlite>) -> Result<()> {
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
        .execute(&mut **transaction)
        .await?;
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS last_selected_repository (
                id INTEGER PRIMARY KEY CHECK (id = 1),
                owner TEXT NOT NULL,
                name TEXT NOT NULL,
                updated_at INTEGER NOT NULL
            )",
        )
        .execute(&mut **transaction)
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
                cache_version INTEGER NOT NULL DEFAULT 1,
                fetched_at INTEGER NOT NULL,
                PRIMARY KEY (owner, name, mode, number)
            )",
        )
        .execute(&mut **transaction)
        .await?;
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS pull_request_detail_cache (
                owner TEXT NOT NULL,
                name TEXT NOT NULL,
                number INTEGER NOT NULL,
                head_sha TEXT NOT NULL,
                section TEXT NOT NULL,
                data_json TEXT NOT NULL,
                cache_version INTEGER NOT NULL DEFAULT 1,
                fetched_at INTEGER NOT NULL,
                PRIMARY KEY (owner, name, number, head_sha, section)
            )",
        )
        .execute(&mut **transaction)
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
        .execute(&mut **transaction)
        .await?;
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS http_cache_validators (
                request_key TEXT PRIMARY KEY,
                etag TEXT,
                last_modified TEXT,
                updated_at INTEGER NOT NULL
            )",
        )
        .execute(&mut **transaction)
        .await?;
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS app_settings (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL,
                updated_at INTEGER NOT NULL
            )",
        )
        .execute(&mut **transaction)
        .await?;

        Ok(())
    }

    async fn apply_repository_state_migration(
        transaction: &mut Transaction<'_, Sqlite>,
    ) -> Result<()> {
        Self::add_column_if_missing(
            transaction,
            "recent_repositories",
            "local_path",
            "ALTER TABLE recent_repositories ADD COLUMN local_path TEXT",
        )
        .await?;
        Self::add_column_if_missing(
            transaction,
            "recent_repositories",
            "last_seen_at",
            "ALTER TABLE recent_repositories ADD COLUMN last_seen_at INTEGER NOT NULL DEFAULT 0",
        )
        .await?;
        Self::add_column_if_missing(
            transaction,
            "recent_repositories",
            "last_seen_position",
            "ALTER TABLE recent_repositories ADD COLUMN last_seen_position INTEGER NOT NULL DEFAULT 0",
        )
        .await?;
        Self::add_column_if_missing(
            transaction,
            "pull_request_inbox_cache",
            "cache_version",
            "ALTER TABLE pull_request_inbox_cache ADD COLUMN cache_version INTEGER NOT NULL DEFAULT 1",
        )
        .await?;
        Self::add_column_if_missing(
            transaction,
            "pull_request_detail_cache",
            "cache_version",
            "ALTER TABLE pull_request_detail_cache ADD COLUMN cache_version INTEGER NOT NULL DEFAULT 1",
        )
        .await?;

        sqlx::query(
            "DELETE FROM recent_repositories
             WHERE pinned = 0 AND local_path IS NULL",
        )
        .execute(&mut **transaction)
        .await?;

        Ok(())
    }

    async fn add_column_if_missing(
        transaction: &mut Transaction<'_, Sqlite>,
        table: &str,
        column: &str,
        migration: &str,
    ) -> Result<()> {
        let columns = sqlx::query(&format!("PRAGMA table_info({table})"))
            .fetch_all(&mut **transaction)
            .await?;
        if !columns
            .iter()
            .any(|row| row.get::<String, _>("name") == column)
        {
            sqlx::query(migration).execute(&mut **transaction).await?;
        }
        Ok(())
    }

    async fn record_schema_migration(
        transaction: &mut Transaction<'_, Sqlite>,
        version: i64,
    ) -> Result<()> {
        sqlx::query(
            "INSERT INTO schema_migrations (version, applied_at)
             VALUES (?1, unixepoch())",
        )
        .bind(version)
        .execute(&mut **transaction)
        .await?;
        Ok(())
    }
}
