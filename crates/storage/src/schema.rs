use sqlx::Row;

use crate::{Result, SqliteStore};

impl SqliteStore {
    pub(super) async fn migrate(&self) -> Result<()> {
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
