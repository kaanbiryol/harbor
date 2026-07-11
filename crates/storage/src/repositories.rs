use std::path::Path;

use harbor_domain::RepoId;
use sqlx::Row;

use super::{Result, SqliteStore, rows::recent_repositories_from_rows, types::RecentRepository};

impl SqliteStore {
    pub async fn pinned_repositories(&self) -> Result<Vec<RecentRepository>> {
        let rows = sqlx::query(
            "SELECT owner, name, pinned, local_path
             FROM recent_repositories
             WHERE pinned = 1
             ORDER BY
                last_opened_at DESC,
                owner ASC,
                name ASC",
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(recent_repositories_from_rows(rows))
    }

    pub async fn repositories_with_local_paths(&self) -> Result<Vec<RecentRepository>> {
        let rows = sqlx::query(
            "SELECT owner, name, pinned, local_path
             FROM recent_repositories
             WHERE local_path IS NOT NULL",
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(recent_repositories_from_rows(rows))
    }

    pub async fn record_repository(&self, repository: &RepoId) -> Result<()> {
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

    pub async fn set_repository_pinned(&self, repository: &RepoId, pinned: bool) -> Result<()> {
        sqlx::query(
            "INSERT INTO recent_repositories (owner, name, pinned, last_opened_at)
             VALUES (?1, ?2, ?3, unixepoch())
             ON CONFLICT(owner, name) DO UPDATE SET
                pinned = excluded.pinned,
                last_opened_at = unixepoch()",
        )
        .bind(&repository.owner)
        .bind(&repository.name)
        .bind(i64::from(pinned))
        .execute(&self.pool)
        .await?;

        if !pinned {
            sqlx::query(
                "DELETE FROM recent_repositories
                 WHERE owner = ?1 AND name = ?2 AND pinned = 0 AND local_path IS NULL",
            )
            .bind(&repository.owner)
            .bind(&repository.name)
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
        local_path: &Path,
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
}
