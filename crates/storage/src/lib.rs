use std::path::PathBuf;

use harbor_domain::RepoId;
use sqlx::{
    Row, SqlitePool,
    sqlite::{SqliteConnectOptions, SqlitePoolOptions},
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

#[derive(Clone)]
pub struct SqliteStore {
    pool: SqlitePool,
}

impl SqliteStore {
    pub async fn connect(config: StorageConfig) -> Result<Self> {
        if let Some(parent) = config.database_path.parent() {
            std::fs::create_dir_all(parent).map_err(|error| {
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
             ORDER BY pinned DESC, last_opened_at DESC, owner ASC, name ASC",
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|row| RecentRepository {
                id: RepoId::new(row.get::<String, _>("owner"), row.get::<String, _>("name")),
                pinned: row.get::<i64, _>("pinned") != 0,
                local_path: row
                    .get::<Option<String>, _>("local_path")
                    .map(PathBuf::from),
            })
            .collect())
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

        Ok(())
    }

    pub async fn sync_repositories(&self, repositories: &[RepoId]) -> Result<()> {
        for repository in repositories {
            sqlx::query(
                "INSERT INTO recent_repositories (owner, name, pinned, last_opened_at)
                 VALUES (?1, ?2, 0, 0)
                 ON CONFLICT(owner, name) DO NOTHING",
            )
            .bind(&repository.owner)
            .bind(&repository.name)
            .execute(&self.pool)
            .await?;
        }

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

    async fn migrate(&self) -> Result<()> {
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS recent_repositories (
                owner TEXT NOT NULL,
                name TEXT NOT NULL,
                pinned INTEGER NOT NULL DEFAULT 0,
                last_opened_at INTEGER NOT NULL DEFAULT (unixepoch()),
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

        if !has_local_path {
            sqlx::query("ALTER TABLE recent_repositories ADD COLUMN local_path TEXT")
                .execute(&self.pool)
                .await?;
        }

        Ok(())
    }
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
}
