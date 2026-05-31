use std::path::PathBuf;

use chrono::{DateTime, Utc};
use harbor_domain::RepoId;

use crate::{Result, StorageError};

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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PullRequestDetailSection {
    Metadata,
    Files,
    Reviews,
    ReviewThreads,
    CheckRuns,
    WorkflowRuns,
}

impl PullRequestDetailSection {
    pub fn key(self) -> &'static str {
        match self {
            Self::Metadata => "metadata",
            Self::Files => "files",
            Self::Reviews => "reviews",
            Self::ReviewThreads => "review_threads",
            Self::CheckRuns => "check_runs",
            Self::WorkflowRuns => "workflow_runs",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SyncTargetState {
    pub target_key: String,
    pub last_successful_fetch_at: Option<DateTime<Utc>>,
    pub last_attempt_at: Option<DateTime<Utc>>,
    pub last_error: Option<String>,
    pub stale: bool,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct StoredHttpCacheValidator {
    pub etag: Option<String>,
    pub last_modified: Option<String>,
}

impl StoredHttpCacheValidator {
    pub fn is_empty(&self) -> bool {
        self.etag.is_none() && self.last_modified.is_none()
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
