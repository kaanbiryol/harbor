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
    #[error("database schema version {found} is newer than the supported version {supported}")]
    UnsupportedSchemaVersion { found: i64, supported: i64 },
    #[error("storage operation failed: {0}")]
    Database(#[from] sqlx::Error),
    #[error("storage operation failed: {0}")]
    Serialization(#[from] serde_json::Error),
    #[error("storage operation failed: {0}")]
    Operation(String),
}

#[derive(Clone)]
pub struct SqliteStore {
    pool: SqlitePool,
    cache_account: String,
}

impl SqliteStore {
    /// Scopes GitHub caches to a verified account while sharing local preferences.
    pub fn for_account(&self, login: &str) -> Self {
        Self {
            pool: self.pool.clone(),
            cache_account: login.to_ascii_lowercase(),
        }
    }

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
        let store = Self {
            pool,
            cache_account: String::new(),
        };
        store.initialize_schema().await?;
        Ok(store)
    }
}

#[cfg(test)]
mod tests;
