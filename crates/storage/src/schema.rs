use sqlx::{Sqlite, Transaction};

use crate::{Result, SqliteStore, StorageError};

const CURRENT_SCHEMA_VERSION: i64 = 2;

impl SqliteStore {
    pub(super) async fn initialize_schema(&self) -> Result<()> {
        let mut transaction = self.pool.begin().await?;
        Self::migrate_schema(&mut transaction).await?;
        transaction.commit().await?;
        Ok(())
    }

    async fn migrate_schema(transaction: &mut Transaction<'_, Sqlite>) -> Result<()> {
        let schema_version = sqlx::query_scalar::<_, i64>("PRAGMA user_version")
            .fetch_one(&mut **transaction)
            .await?;

        if schema_version > CURRENT_SCHEMA_VERSION {
            return Err(StorageError::UnsupportedSchemaVersion {
                found: schema_version,
                supported: CURRENT_SCHEMA_VERSION,
            });
        }

        if schema_version < 1 {
            Self::migrate_to_v1(transaction).await?;
            sqlx::query("PRAGMA user_version = 1")
                .execute(&mut **transaction)
                .await?;
        }

        if schema_version < 2 {
            Self::migrate_to_v2(transaction).await?;
            sqlx::query("PRAGMA user_version = 2")
                .execute(&mut **transaction)
                .await?;
        }

        Ok(())
    }

    async fn migrate_to_v2(transaction: &mut Transaction<'_, Sqlite>) -> Result<()> {
        // Legacy caches have no account identity, so assigning them to a user would be unsafe.
        for statement in [
            "DROP TABLE pull_request_detail_cache",
            "DROP TABLE sync_target_state",
            "DROP TABLE http_cache_validators",
            "CREATE TABLE pull_request_detail_cache (
                account TEXT NOT NULL DEFAULT '',
                owner TEXT NOT NULL,
                name TEXT NOT NULL,
                number INTEGER NOT NULL,
                head_sha TEXT NOT NULL,
                section TEXT NOT NULL,
                data_json TEXT NOT NULL,
                fetched_at INTEGER NOT NULL,
                PRIMARY KEY (account, owner, name, number, head_sha, section)
            )",
            "CREATE TABLE sync_target_state (
                account TEXT NOT NULL DEFAULT '',
                target_key TEXT NOT NULL,
                last_successful_fetch_at INTEGER,
                last_attempt_at INTEGER,
                last_error TEXT,
                stale INTEGER NOT NULL DEFAULT 0,
                PRIMARY KEY (account, target_key)
            )",
            "CREATE TABLE http_cache_validators (
                account TEXT NOT NULL DEFAULT '',
                request_key TEXT NOT NULL,
                etag TEXT,
                last_modified TEXT,
                updated_at INTEGER NOT NULL,
                PRIMARY KEY (account, request_key)
            )",
        ] {
            sqlx::query(statement).execute(&mut **transaction).await?;
        }
        Ok(())
    }

    async fn migrate_to_v1(transaction: &mut Transaction<'_, Sqlite>) -> Result<()> {
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
}
