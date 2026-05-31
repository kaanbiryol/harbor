use std::time::{SystemTime, UNIX_EPOCH};

use harbor_domain::{ChecksSummary, MergeState, PullRequest, PullRequestState, ReviewDecision};

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
fn limits_recent_repository_results() {
    smol::block_on(async {
        let database_path = test_database_path("limits-repositories");
        let store = SqliteStore::connect(StorageConfig {
            database_path: database_path.clone(),
        })
        .await
        .expect("connect sqlite store");

        store
            .sync_repositories(&[
                RepoId::new("acme", "one"),
                RepoId::new("acme", "two"),
                RepoId::new("acme", "three"),
            ])
            .await
            .expect("sync repositories");

        let repositories = store
            .recent_repositories_limited(2)
            .await
            .expect("load limited repositories");

        assert_eq!(repositories.len(), 2);

        cleanup_database(database_path);
    });
}

#[test]
fn latest_repository_sync_takes_priority_over_stale_synced_rows() {
    smol::block_on(async {
        let database_path = test_database_path("latest-sync-priority");
        let store = SqliteStore::connect(StorageConfig {
            database_path: database_path.clone(),
        })
        .await
        .expect("connect sqlite store");
        let stale_repository = RepoId::new("aaa", "stale");
        let latest_repository = RepoId::new("zzz", "latest");

        store
            .sync_repositories(std::slice::from_ref(&stale_repository))
            .await
            .expect("sync stale repository");
        sqlx::query("UPDATE recent_repositories SET last_seen_at = 1")
            .execute(&store.pool)
            .await
            .expect("age synced repositories");
        store
            .sync_repositories(std::slice::from_ref(&latest_repository))
            .await
            .expect("sync latest repository");

        let repositories = store
            .recent_repositories_limited(1)
            .await
            .expect("load limited repositories");

        assert_eq!(repositories[0].id, latest_repository);

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
fn syncing_repositories_does_not_replace_last_selected_repository() {
    smol::block_on(async {
        let database_path = test_database_path("sync-keeps-last-selected-repository");
        let store = SqliteStore::connect(StorageConfig {
            database_path: database_path.clone(),
        })
        .await
        .expect("connect sqlite store");
        let selected_repository = RepoId::new("acme", "app");
        let synced_repository = RepoId::new("zed", "editor");

        store
            .record_repository(&selected_repository)
            .await
            .expect("record selected repository");
        store
            .sync_repositories(std::slice::from_ref(&synced_repository))
            .await
            .expect("sync repositories");

        assert_eq!(
            store
                .last_selected_repository()
                .await
                .expect("load last selected repository"),
            Some(selected_repository)
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
        assert_eq!(
            migration_versions(&store).await.expect("load migrations"),
            vec![1]
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
            vec![1]
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

        assert_eq!(cached_metadata, Some(pull_request));
        assert_eq!(cached_checks, Some(Vec::new()));

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
        checks_summary: ChecksSummary::default(),
        unresolved_threads: 0,
        updated_at: DateTime::from_timestamp(1_777_777_777, 0),
    }
}
