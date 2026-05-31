use std::path::PathBuf;

use chrono::{DateTime, Utc};
use harbor_domain::RepoId;
use sqlx::{Row, sqlite::SqliteRow};

use super::{RecentRepository, Result, StorageError, SyncTargetState};

pub(super) fn recent_repositories_from_rows(rows: Vec<SqliteRow>) -> Vec<RecentRepository> {
    rows.into_iter()
        .map(|row| RecentRepository {
            id: RepoId::new(row.get::<String, _>("owner"), row.get::<String, _>("name")),
            pinned: row.get::<i64, _>("pinned") != 0,
            local_path: row
                .get::<Option<String>, _>("local_path")
                .map(PathBuf::from),
        })
        .collect()
}

pub(super) fn sync_target_state_from_row(row: SqliteRow) -> Result<SyncTargetState> {
    Ok(SyncTargetState {
        target_key: row.get("target_key"),
        last_successful_fetch_at: unix_timestamp_to_datetime(
            row.get::<Option<i64>, _>("last_successful_fetch_at"),
        )?,
        last_attempt_at: unix_timestamp_to_datetime(row.get::<Option<i64>, _>("last_attempt_at"))?,
        last_error: row.get("last_error"),
        stale: row.get::<i64, _>("stale") != 0,
    })
}

fn unix_timestamp_to_datetime(timestamp: Option<i64>) -> Result<Option<DateTime<Utc>>> {
    timestamp
        .map(|timestamp| {
            DateTime::<Utc>::from_timestamp(timestamp, 0).ok_or_else(|| {
                StorageError::Operation(format!("invalid unix timestamp {timestamp}"))
            })
        })
        .transpose()
}
