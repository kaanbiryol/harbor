use sqlx::Row;

use crate::{Result, SqliteStore};

impl SqliteStore {
    pub async fn load_preferences(&self) -> Result<harbor_domain::preferences::AppPreferences> {
        let mut preferences: harbor_domain::preferences::AppPreferences = self
            .load_app_setting("preferences")
            .await?
            .map(|value| serde_json::from_str(&value))
            .transpose()?
            .unwrap_or_default();
        preferences.normalize();
        Ok(preferences)
    }

    pub async fn save_preferences(
        &self,
        preferences: &harbor_domain::preferences::AppPreferences,
    ) -> Result<()> {
        self.save_app_setting("preferences", &serde_json::to_string(preferences)?)
            .await
    }

    pub async fn load_app_setting(&self, key: &str) -> Result<Option<String>> {
        let row = sqlx::query(
            "SELECT value
             FROM app_settings
             WHERE key = ?1",
        )
        .bind(key)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(|row| row.get::<String, _>("value")))
    }

    pub async fn save_app_setting(&self, key: &str, value: &str) -> Result<()> {
        sqlx::query(
            "INSERT INTO app_settings (key, value, updated_at)
             VALUES (?1, ?2, unixepoch())
             ON CONFLICT(key) DO UPDATE SET
                value = excluded.value,
                updated_at = excluded.updated_at",
        )
        .bind(key)
        .bind(value)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn delete_app_setting(&self, key: &str) -> Result<()> {
        sqlx::query(
            "DELETE FROM app_settings
             WHERE key = ?1",
        )
        .bind(key)
        .execute(&self.pool)
        .await?;

        Ok(())
    }
}
