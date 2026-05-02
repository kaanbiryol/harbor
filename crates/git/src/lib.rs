use std::path::PathBuf;

use thiserror::Error;

pub type Result<T> = std::result::Result<T, GitError>;

#[derive(Debug, Error)]
pub enum GitError {
    #[error("local repository was not configured")]
    MissingRepository,
    #[error("git command failed: {0}")]
    Command(String),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LocalCheckout {
    pub repo_path: PathBuf,
    pub branch: String,
}
