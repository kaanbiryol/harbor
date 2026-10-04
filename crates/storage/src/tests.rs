use std::{
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

use super::*;
use chrono::DateTime;
use harbor_domain::{
    ChecksSummary, MergeState, PullRequest, PullRequestState, RepoId, ReviewDecision,
};

#[test]
fn initializes_schema_at_current_version() {
    smol::block_on(async {
        let database_path = test_database_path("schema-version");
        let store = SqliteStore::connect(StorageConfig {
            database_path: database_path.clone(),
        })
        .await
        .expect("connect sqlite store");

        assert_eq!(schema_version(&store).await, 2);

        cleanup_database(database_path);
    });
}

#[test]
fn upgrades_unversioned_database_without_losing_data() {
    smol::block_on(async {
        let database_path = test_database_path("unversioned-schema");
        let config = StorageConfig {
            database_path: database_path.clone(),
        };
        let store = SqliteStore::connect(config.clone())
            .await
            .expect("connect sqlite store");
        let repository = RepoId::new("acme", "app");
        store
            .set_repository_pinned(&repository, true)
            .await
            .expect("pin repository");
        sqlx::query("PRAGMA user_version = 0")
            .execute(&store.pool)
            .await
            .expect("mark database as unversioned");
        store.pool.close().await;

        let migrated_store = SqliteStore::connect(config)
            .await
            .expect("migrate sqlite store");
        let repositories = migrated_store
            .pinned_repositories()
            .await
            .expect("load repositories after migration");

        assert_eq!(schema_version(&migrated_store).await, 2);
        assert_eq!(repositories.len(), 1);
        assert_eq!(repositories[0].id, repository);

        cleanup_database(database_path);
    });
}

#[test]
fn rejects_database_from_newer_harbor_version() {
    smol::block_on(async {
        let database_path = test_database_path("newer-schema");
        let config = StorageConfig {
            database_path: database_path.clone(),
        };
        let store = SqliteStore::connect(config.clone())
            .await
            .expect("connect sqlite store");
        sqlx::query("PRAGMA user_version = 3")
            .execute(&store.pool)
            .await
            .expect("set newer schema version");
        store.pool.close().await;

        let error = SqliteStore::connect(config)
            .await
            .err()
            .expect("newer schema should be rejected");

        assert!(matches!(
            error,
            StorageError::UnsupportedSchemaVersion {
                found: 3,
                supported: 2
            }
        ));

        cleanup_database(database_path);
    });
}

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
fn preferences_round_trip_and_accept_missing_fields() {
    use harbor_domain::preferences::{
        AppPreferences, PreferredEditor, PreferredTerminal, RefreshInterval,
    };
    smol::block_on(async {
        let database_path = test_database_path("preferences");
        let store = SqliteStore::connect(StorageConfig {
            database_path: database_path.clone(),
        })
        .await
        .expect("connect store");
        assert_eq!(
            store.load_preferences().await.expect("defaults"),
            AppPreferences::default()
        );
        let mut preferences = AppPreferences {
            code_font_size: 18.0,
            wrap_diff_lines: false,
            editor: PreferredEditor::Zed,
            terminal: PreferredTerminal::Ghostty,
            focused_refresh: RefreshInterval::Manual,
            background_refresh: RefreshInterval::OneHour,
            worktree_directory: Some(std::env::temp_dir().join("custom-worktrees")),
            ..AppPreferences::default()
        };
        preferences.notifications.approvals = false;
        store
            .save_preferences(&preferences)
            .await
            .expect("save preferences");
        assert_eq!(
            store.load_preferences().await.expect("reload preferences"),
            preferences
        );
        store
            .save_app_setting(
                "preferences",
                r#"{"code_font_size":999,"notifications":{"enabled":false}}"#,
            )
            .await
            .expect("older settings");
        let loaded = store
            .load_preferences()
            .await
            .expect("load partial settings");
        assert_eq!(loaded.code_font_size, 24.0);
        assert!(!loaded.notifications.enabled);
        assert!(loaded.notifications.checks_failed);
        assert_eq!(loaded.focused_refresh, RefreshInterval::FiveMinutes);
        store
            .save_app_setting("preferences", "broken json")
            .await
            .expect("invalid settings");
        assert!(store.load_preferences().await.is_err());
        store.pool.close().await;
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
fn records_sync_failure() {
    smol::block_on(async {
        let database_path = test_database_path("sync-failure");
        let store = SqliteStore::connect(StorageConfig {
            database_path: database_path.clone(),
        })
        .await
        .expect("connect sqlite store");
        let repository = RepoId::new("acme", "app");
        let target_key = inbox_target_key(&repository, "open");

        store
            .record_sync_failure(&target_key, "rate limited")
            .await
            .expect("record failure");

        let target_state = store
            .sync_target_state(&target_key)
            .await
            .expect("load target state")
            .expect("target state");

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

#[test]
fn account_caches_are_isolated_and_preferences_are_shared() {
    smol::block_on(async {
        let database_path = test_database_path("account-cache");
        let store = SqliteStore::connect(StorageConfig {
            database_path: database_path.clone(),
        })
        .await
        .unwrap();
        let alice = store.for_account("Alice");
        let bob = store.for_account("bob");
        let pull_request = pull_request(42);
        let validator = StoredHttpCacheValidator {
            etag: Some("alice-etag".into()),
            last_modified: None,
        };
        alice
            .save_pull_request_metadata(&pull_request)
            .await
            .unwrap();
        alice
            .save_http_cache_validator("request", &validator)
            .await
            .unwrap();
        alice.record_sync_success("target").await.unwrap();
        alice
            .set_repository_pinned(&pull_request.repo, true)
            .await
            .unwrap();
        assert_eq!(
            bob.load_pull_request_metadata(&pull_request.repo, 42, &pull_request.head_sha)
                .await
                .unwrap(),
            None
        );
        assert_eq!(
            bob.load_http_cache_validator("request").await.unwrap(),
            None
        );
        assert_eq!(bob.sync_target_state("target").await.unwrap(), None);
        assert_eq!(bob.pinned_repositories().await.unwrap().len(), 1);
        assert_eq!(
            store
                .for_account("alice")
                .load_pull_request_metadata(&pull_request.repo, 42, &pull_request.head_sha)
                .await
                .unwrap(),
            Some(pull_request)
        );
        // A response captured before an account switch still writes only to its original namespace.
        alice
            .record_sync_failure("target", "late failure")
            .await
            .unwrap();
        assert_eq!(bob.sync_target_state("target").await.unwrap(), None);
        store.pool.close().await;
        cleanup_database(database_path);
    });
}

#[test]
fn account_migration_discards_unowned_cache_and_preserves_preferences() {
    smol::block_on(async {
        let database_path = test_database_path("legacy-account-cache");
        let config = StorageConfig {
            database_path: database_path.clone(),
        };
        let store = SqliteStore::connect(config.clone()).await.unwrap();
        let pull_request = pull_request(42);
        store
            .save_pull_request_metadata(&pull_request)
            .await
            .unwrap();
        store
            .set_repository_pinned(&pull_request.repo, true)
            .await
            .unwrap();
        sqlx::query("PRAGMA user_version = 1")
            .execute(&store.pool)
            .await
            .unwrap();
        store.pool.close().await;
        let migrated = SqliteStore::connect(config).await.unwrap();
        assert_eq!(
            migrated
                .load_pull_request_metadata(&pull_request.repo, 42, &pull_request.head_sha)
                .await
                .unwrap(),
            None
        );
        assert_eq!(migrated.pinned_repositories().await.unwrap().len(), 1);
        migrated.pool.close().await;
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

async fn schema_version(store: &SqliteStore) -> i64 {
    sqlx::query_scalar("PRAGMA user_version")
        .fetch_one(&store.pool)
        .await
        .expect("load schema version")
}

fn pull_request(number: u64) -> PullRequest {
    PullRequest {
        repo: RepoId::new("acme", "app"),
        node_id: format!("pr-node-{number}"),
        number,
        title: "Add feature".to_string(),
        body: None,
        author: "octocat".to_string(),
        author_avatar_url: None,
        url: format!("https://github.com/acme/app/pull/{number}"),
        state: PullRequestState::Open,
        is_draft: false,
        head_ref: "feature".to_string(),
        base_ref: "main".to_string(),
        head_sha: "abc123".to_string(),
        review_decision: Some(ReviewDecision::ReviewRequired),
        merge_state: Some(MergeState::Clean),
        merge_capabilities: Default::default(),
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
