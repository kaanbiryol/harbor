use std::path::PathBuf;

use thiserror::Error;

pub type Result<T> = std::result::Result<T, StorageError>;

#[derive(Debug, Error)]
pub enum StorageError {
    #[error("storage has not been initialized")]
    NotInitialized,
    #[error("storage operation failed: {0}")]
    Operation(String),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StorageConfig {
    pub database_path: PathBuf,
}
