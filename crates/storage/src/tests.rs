use std::{
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

use chrono::DateTime;
use harbor_domain::{
    ChecksSummary, MergeState, PullRequest, PullRequestState, RepoId, ReviewDecision,
};
use sqlx::Row;

use super::*;

#[test]
fn persists_only_pinned_repositories_for_the_switcher() {
    smol::block_on(async {
        let database_path = test_database_path("syncs-repositories");
        let store = SqliteStore::connect(StorageConfig {
            database_path: database_path.clone(),
        })
        .await
        .expect("connect sqlite store");
        let repository = RepoId::new("acme", "app");
        store
            .set_repository_pinned(&repository, true)
            .await
            .expect("pin repository");

        let repositories = store
            .pinned_repositories()
            .await
            .expect("load pinned repositories");

        assert_eq!(repositories.len(), 1);
        assert_eq!(repositories[0].id, repository);
        assert!(repositories[0].pinned);
        assert_eq!(repositories[0].local_path, None);

        cleanup_database(database_path);
    });
}

#[test]
fn unpins_repository_without_retaining_it_in_the_switcher() {
    smol::block_on(async {
        let database_path = test_database_path("limits-repositories");
        let store = SqliteStore::connect(StorageConfig {
            database_path: database_path.clone(),
        })
        .await
        .expect("connect sqlite store");

        let repository = RepoId::new("acme", "one");
        store
            .set_repository_pinned(&repository, true)
            .await
            .expect("pin repository");
        store
            .set_repository_pinned(&repository, false)
            .await
            .expect("unpin repository");

        let repositories = store
            .pinned_repositories()
            .await
            .expect("load pinned repositories");

        assert!(repositories.is_empty());

        cleanup_database(database_path);
    });
}

#[test]
fn records_last_selected_repository_when_repository_is_opened() {
    smol::block_on(async {
        let database_path = test_database_path("last-selected-repository");
        let store = SqliteStore::connect(StorageConfig {
            database_path: database_path.clone(),
        })
        .await
        .expect("connect sqlite store");
        let first_repository = RepoId::new("acme", "app");
        let second_repository = RepoId::new("zed", "editor");

        assert_eq!(
            store
                .last_selected_repository()
                .await
                .expect("load empty last selected repository"),
            None
        );

        store
            .record_repository(&first_repository)
            .await
            .expect("record first repository");
        assert_eq!(
            store
                .last_selected_repository()
                .await
                .expect("load first last selected repository"),
            Some(first_repository)
        );

        store
            .record_repository(&second_repository)
            .await
            .expect("record second repository");
        assert_eq!(
            store
                .last_selected_repository()
                .await
                .expect("load second last selected repository"),
            Some(second_repository)
        );

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
            .repositories_with_local_paths()
            .await
            .expect("load repositories with local paths");

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
             VALUES ('acme', 'app', 1, 0)",
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
            .pinned_repositories()
            .await
            .expect("load pinned repositories");

        assert_eq!(repositories.len(), 1);
        assert_eq!(repositories[0].id, RepoId::new("acme", "app"));
        assert_eq!(repositories[0].local_path, None);
        assert_eq!(
            migration_versions(&store).await.expect("load migrations"),
            vec![1, 2]
        );

        cleanup_database(database_path);
    });
}

#[test]
fn records_initial_schema_migration_for_new_database() {
    smol::block_on(async {
        let database_path = test_database_path("records-schema-migration");
        let store = SqliteStore::connect(StorageConfig {
            database_path: database_path.clone(),
        })
        .await
        .expect("connect sqlite store");

        assert_eq!(
            migration_versions(&store).await.expect("load migrations"),
            vec![1, 2]
        );

        cleanup_database(database_path);
    });
}

#[test]
fn saves_and_deletes_app_settings() {
    smol::block_on(async {
        let database_path = test_database_path("app-settings");
        let store = SqliteStore::connect(StorageConfig {
            database_path: database_path.clone(),
        })
        .await
        .expect("connect sqlite store");

        assert_eq!(
            store
                .load_app_setting("github.auth_source")
                .await
                .expect("load missing setting"),
            None
        );

        store
            .save_app_setting("github.auth_source", "gh_cli")
            .await
            .expect("save setting");
        assert_eq!(
            store
                .load_app_setting("github.auth_source")
                .await
                .expect("load saved setting"),
            Some("gh_cli".to_string())
        );

        store
            .save_app_setting("github.auth_source", "oauth")
            .await
            .expect("update setting");
        assert_eq!(
            store
                .load_app_setting("github.auth_source")
                .await
                .expect("load updated setting"),
            Some("oauth".to_string())
        );

        store
            .delete_app_setting("github.auth_source")
            .await
            .expect("delete setting");
        assert_eq!(
            store
                .load_app_setting("github.auth_source")
                .await
                .expect("load deleted setting"),
            None
        );

        cleanup_database(database_path);
    });
}

#[test]
fn saves_and_loads_pull_request_inbox_cache() {
    smol::block_on(async {
        let database_path = test_database_path("pull-request-inbox-cache");
        let store = SqliteStore::connect(StorageConfig {
            database_path: database_path.clone(),
        })
        .await
        .expect("connect sqlite store");
        let repository = RepoId::new("acme", "app");
        let pull_request = pull_request(7);

        store
            .save_pull_request_inbox(&repository, "open", std::slice::from_ref(&pull_request))
            .await
            .expect("save inbox");

        let cached = store
            .load_pull_request_inbox(&repository, "open")
            .await
            .expect("load inbox");

        assert_eq!(cached, vec![pull_request]);

        cleanup_database(database_path);
    });
}

#[test]
fn invalid_inbox_cache_rows_are_discarded() {
    smol::block_on(async {
        let database_path = test_database_path("invalid-inbox-cache");
        let store = SqliteStore::connect(StorageConfig {
            database_path: database_path.clone(),
        })
        .await
        .expect("connect sqlite store");
        let repository = RepoId::new("acme", "app");

        store
            .save_pull_request_inbox(&repository, "open", &[pull_request(7)])
            .await
            .expect("save inbox");
        sqlx::query(
            "UPDATE pull_request_inbox_cache
             SET pr_json = 'not-json'
             WHERE owner = 'acme' AND name = 'app' AND mode = 'open'",
        )
        .execute(&store.pool)
        .await
        .expect("corrupt inbox cache");

        let cached = store
            .load_pull_request_inbox(&repository, "open")
            .await
            .expect("load invalid inbox as cache miss");
        assert!(cached.is_empty());

        let remaining = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM pull_request_inbox_cache
             WHERE owner = 'acme' AND name = 'app' AND mode = 'open'",
        )
        .fetch_one(&store.pool)
        .await
        .expect("count inbox cache rows");
        assert_eq!(remaining, 0);

        cleanup_database(database_path);
    });
}

#[test]
fn replaces_absent_pull_requests_for_cached_mode() {
    smol::block_on(async {
        let database_path = test_database_path("replace-pull-request-inbox-cache");
        let store = SqliteStore::connect(StorageConfig {
            database_path: database_path.clone(),
        })
        .await
        .expect("connect sqlite store");
        let repository = RepoId::new("acme", "app");

        store
            .save_pull_request_inbox(&repository, "open", &[pull_request(7), pull_request(8)])
            .await
            .expect("save initial inbox");
        store
            .save_pull_request_inbox(&repository, "open", &[pull_request(8)])
            .await
            .expect("replace inbox");

        let cached = store
            .load_pull_request_inbox(&repository, "open")
            .await
            .expect("load inbox");

        assert_eq!(cached.len(), 1);
        assert_eq!(cached[0].number, 8);

        cleanup_database(database_path);
    });
}

#[test]
fn failed_inbox_replacement_keeps_existing_cache() {
    smol::block_on(async {
        let database_path = test_database_path("inbox-rollback");
        let store = SqliteStore::connect(StorageConfig {
            database_path: database_path.clone(),
        })
        .await
        .expect("connect sqlite store");
        let repository = RepoId::new("acme", "app");
        let original_pull_request = pull_request(7);

        store
            .save_pull_request_inbox(
                &repository,
                "open",
                std::slice::from_ref(&original_pull_request),
            )
            .await
            .expect("save original inbox");
        sqlx::query(
            "CREATE TRIGGER fail_inbox_insert
             BEFORE INSERT ON pull_request_inbox_cache
             BEGIN
                SELECT RAISE(FAIL, 'blocked insert');
             END",
        )
        .execute(&store.pool)
        .await
        .expect("create failing trigger");

        let result = store
            .save_pull_request_inbox(&repository, "open", &[pull_request(8)])
            .await;
        assert!(result.is_err());

        let cached = store
            .load_pull_request_inbox(&repository, "open")
            .await
            .expect("load inbox after failed replacement");
        assert_eq!(cached, vec![original_pull_request]);

        cleanup_database(database_path);
    });
}

#[test]
fn saves_and_loads_detail_sections_independently() {
    smol::block_on(async {
        let database_path = test_database_path("pull-request-detail-cache");
        let store = SqliteStore::connect(StorageConfig {
            database_path: database_path.clone(),
        })
        .await
        .expect("connect sqlite store");
        let pull_request = pull_request(7);

        store
            .save_pull_request_metadata(&pull_request)
            .await
            .expect("save metadata");
        store
            .save_pull_request_check_runs(
                &pull_request.repo,
                pull_request.number,
                &pull_request.head_sha,
                &[],
            )
            .await
            .expect("save checks");
        store
            .save_pull_request_reviews(
                &pull_request.repo,
                pull_request.number,
                &pull_request.head_sha,
                &[],
                &[],
                &[],
            )
            .await
            .expect("save reviews");

        let cached_metadata = store
            .load_pull_request_metadata(
                &pull_request.repo,
                pull_request.number,
                &pull_request.head_sha,
            )
            .await
            .expect("load metadata");
        let cached_checks = store
            .load_pull_request_check_runs(
                &pull_request.repo,
                pull_request.number,
                &pull_request.head_sha,
            )
            .await
            .expect("load checks");
        let cached_reviews = store
            .load_pull_request_reviews(
                &pull_request.repo,
                pull_request.number,
                &pull_request.head_sha,
            )
            .await
            .expect("load reviews")
            .expect("cached review data");

        assert_eq!(cached_metadata, Some(pull_request));
        assert_eq!(cached_checks, Some(Vec::new()));
        assert!(cached_reviews.0.is_empty());
        assert!(cached_reviews.1.is_empty());
        assert!(cached_reviews.2.is_empty());

        cleanup_database(database_path);
    });
}

#[test]
fn invalid_detail_cache_rows_are_discarded() {
    smol::block_on(async {
        let database_path = test_database_path("invalid-detail-cache");
        let store = SqliteStore::connect(StorageConfig {
            database_path: database_path.clone(),
        })
        .await
        .expect("connect sqlite store");
        let pull_request = pull_request(7);

        store
            .save_pull_request_metadata(&pull_request)
            .await
            .expect("save metadata");
        sqlx::query(
            "UPDATE pull_request_detail_cache
             SET data_json = 'not-json'
             WHERE owner = 'acme' AND name = 'app' AND number = 7 AND section = 'metadata'",
        )
        .execute(&store.pool)
        .await
        .expect("corrupt detail cache");

        let cached = store
            .load_pull_request_metadata(
                &pull_request.repo,
                pull_request.number,
                &pull_request.head_sha,
            )
            .await
            .expect("load invalid detail as cache miss");
        assert_eq!(cached, None);

        let remaining = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM pull_request_detail_cache
             WHERE owner = 'acme' AND name = 'app' AND number = 7 AND section = 'metadata'",
        )
        .fetch_one(&store.pool)
        .await
        .expect("count detail cache rows");
        assert_eq!(remaining, 0);

        cleanup_database(database_path);
    });
}

#[test]
fn records_sync_failure_without_dropping_cache() {
    smol::block_on(async {
        let database_path = test_database_path("sync-failure-preserves-cache");
        let store = SqliteStore::connect(StorageConfig {
            database_path: database_path.clone(),
        })
        .await
        .expect("connect sqlite store");
        let repository = RepoId::new("acme", "app");
        let pull_request = pull_request(7);
        let target_key = inbox_target_key(&repository, "open");

        store
            .save_pull_request_inbox(&repository, "open", std::slice::from_ref(&pull_request))
            .await
            .expect("save inbox");
        store
            .record_sync_failure(&target_key, "rate limited")
            .await
            .expect("record failure");

        let cached = store
            .load_pull_request_inbox(&repository, "open")
            .await
            .expect("load inbox");
        let target_state = store
            .sync_target_state(&target_key)
            .await
            .expect("load target state")
            .expect("target state");

        assert_eq!(cached, vec![pull_request]);
        assert_eq!(target_state.last_error.as_deref(), Some("rate limited"));
        assert!(target_state.stale);

        cleanup_database(database_path);
    });
}

#[test]
fn saves_and_loads_http_cache_validator() {
    smol::block_on(async {
        let database_path = test_database_path("http-cache-validator");
        let store = SqliteStore::connect(StorageConfig {
            database_path: database_path.clone(),
        })
        .await
        .expect("connect sqlite store");
        let validator = StoredHttpCacheValidator {
            etag: Some("\"abc\"".to_string()),
            last_modified: Some("Wed, 01 May 2026 10:00:00 GMT".to_string()),
        };

        store
            .save_http_cache_validator("rest:acme/app:open", &validator)
            .await
            .expect("save validator");

        let cached = store
            .load_http_cache_validator("rest:acme/app:open")
            .await
            .expect("load validator");

        assert_eq!(cached, Some(validator));

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

async fn migration_versions(store: &SqliteStore) -> Result<Vec<i64>> {
    let rows = sqlx::query("SELECT version FROM schema_migrations ORDER BY version ASC")
        .fetch_all(&store.pool)
        .await?;

    Ok(rows
        .into_iter()
        .map(|row| row.get::<i64, _>("version"))
        .collect())
}

fn pull_request(number: u64) -> PullRequest {
    PullRequest {
        repo: RepoId::new("acme", "app"),
        node_id: format!("pr-node-{number}"),
        number,
        title: "Add feature".to_string(),
        body: None,
        author: "octocat".to_string(),
        url: format!("https://github.com/acme/app/pull/{number}"),
        state: PullRequestState::Open,
        is_draft: false,
        head_ref: "feature".to_string(),
        base_ref: "main".to_string(),
        head_sha: "abc123".to_string(),
        review_decision: Some(ReviewDecision::ReviewRequired),
        merge_state: Some(MergeState::Clean),
        labels: Vec::new(),
        assignees: Vec::new(),
        requested_reviewers: Vec::new(),
        requested_teams: Vec::new(),
        checks_summary: ChecksSummary::default(),
        unresolved_threads: 0,
        created_at: DateTime::from_timestamp(1_777_777_777, 0),
        updated_at: DateTime::from_timestamp(1_777_777_777, 0),
    }
}
