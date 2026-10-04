use sqlx::{Sqlite, Transaction};

use super::{Result, SqliteStore, rows::sync_target_state_from_row, types::SyncTargetState};

impl SqliteStore {
    pub async fn mark_sync_attempt(&self, target_key: &str) -> Result<()> {
        sqlx::query(
            "INSERT INTO sync_target_state (target_key, last_attempt_at, stale, account)
             VALUES (?1, unixepoch(), 0, ?2)
             ON CONFLICT(account, target_key) DO UPDATE SET
                last_attempt_at = unixepoch()",
        )
        .bind(target_key)
        .bind(&self.cache_account)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn record_sync_success(&self, target_key: &str) -> Result<()> {
        sqlx::query(
            "INSERT INTO sync_target_state
                (target_key, last_successful_fetch_at, last_attempt_at, last_error, stale, account)
             VALUES (?1, unixepoch(), unixepoch(), NULL, 0, ?2)
             ON CONFLICT(account, target_key) DO UPDATE SET
                last_successful_fetch_at = unixepoch(),
                last_attempt_at = unixepoch(),
                last_error = NULL,
                stale = 0",
        )
        .bind(target_key)
        .bind(&self.cache_account)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn record_sync_failure(&self, target_key: &str, error: &str) -> Result<()> {
        sqlx::query(
            "INSERT INTO sync_target_state (target_key, last_attempt_at, last_error, stale, account)
             VALUES (?1, unixepoch(), ?2, 1, ?3)
             ON CONFLICT(account, target_key) DO UPDATE SET
                last_attempt_at = unixepoch(),
                last_error = excluded.last_error,
                stale = 1",
        )
        .bind(target_key)
        .bind(error)
        .bind(&self.cache_account)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn mark_sync_target_stale(&self, target_key: &str) -> Result<()> {
        sqlx::query(
            "INSERT INTO sync_target_state (target_key, stale, account)
             VALUES (?1, 1, ?2)
             ON CONFLICT(account, target_key) DO UPDATE SET stale = 1",
        )
        .bind(target_key)
        .bind(&self.cache_account)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn sync_target_state(&self, target_key: &str) -> Result<Option<SyncTargetState>> {
        let row = sqlx::query(
            "SELECT target_key, last_successful_fetch_at, last_attempt_at, last_error, stale
             FROM sync_target_state
             WHERE target_key = ?1 AND account = ?2",
        )
        .bind(target_key)
        .bind(&self.cache_account)
        .fetch_optional(&self.pool)
        .await?;

        row.map(sync_target_state_from_row).transpose()
    }

    pub(crate) async fn record_sync_success_in_transaction(
        transaction: &mut Transaction<'_, Sqlite>,
        account: &str,
        target_key: &str,
    ) -> Result<()> {
        sqlx::query(
            "INSERT INTO sync_target_state
                (target_key, last_successful_fetch_at, last_attempt_at, last_error, stale, account)
             VALUES (?1, unixepoch(), unixepoch(), NULL, 0, ?2)
             ON CONFLICT(account, target_key) DO UPDATE SET
                last_successful_fetch_at = unixepoch(),
                last_attempt_at = unixepoch(),
                last_error = NULL,
                stale = 0",
        )
        .bind(target_key)
        .bind(account)
        .execute(&mut **transaction)
        .await?;

        Ok(())
    }
}
