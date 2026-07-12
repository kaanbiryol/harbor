use sqlx::{
    SqlitePool,
    sqlite::{SqliteConnectOptions, SqlitePoolOptions},
};
use thiserror::Error;

mod http_cache;
mod pull_request_cache;
mod repositories;
mod rows;
mod schema;
mod settings;
mod sync_state;
mod types;

pub use pull_request_cache::{detail_target_key, inbox_target_key};
pub use types::{
    PullRequestDetailSection, RecentRepository, StorageConfig, StoredHttpCacheValidator,
    SyncTargetState,
};

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
        store.initialize_schema().await?;
        Ok(store)
    }
}

#[cfg(test)]
mod tests;
