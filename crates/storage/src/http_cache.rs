use sqlx::Row;

use super::{Result, SqliteStore, StoredHttpCacheValidator};

impl SqliteStore {
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
}
