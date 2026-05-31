use std::io::{Cursor, Write};

use serde_json::json;
use zip::{ZipWriter, write::SimpleFileOptions};

use super::*;

#[test]
fn parses_included_rate_limit_headers_and_json_body() {
    let output = concat!(
        "HTTP/2 200 OK\r\n",
        "x-ratelimit-limit: 5000\r\n",
        "x-ratelimit-remaining: 42\r\n",
        "x-ratelimit-reset: 1770000000\r\n",
        "x-ratelimit-resource: graphql\r\n",
        "x-ratelimit-used: 12\r\n",
        "retry-after: 5\r\n",
        "\r\n",
        "{\"data\":{\"viewer\":{\"login\":\"octocat\"}}}"
    );

    let parsed = parse_json_output(output.as_bytes()).unwrap();

    assert_eq!(
        parsed.value,
        json!({ "data": { "viewer": { "login": "octocat" } } })
    );
    assert_eq!(parsed.metadata.status_code, Some(200));
    assert_eq!(parsed.metadata.rate_limit.remaining, Some(42));
    assert_eq!(parsed.metadata.rate_limit.limit, Some(5000));
    assert_eq!(
        parsed.metadata.rate_limit.reset_epoch_seconds,
        Some(1_770_000_000)
    );
    assert_eq!(
        parsed.metadata.rate_limit.resource.as_deref(),
        Some("graphql")
    );
    assert_eq!(parsed.metadata.rate_limit.used, Some(12));
    assert_eq!(parsed.metadata.rate_limit.retry_after_seconds, Some(5));
}

#[test]
fn treats_header_only_success_as_null_json() {
    let output = concat!(
        "HTTP/2 204 No Content\r\n",
        "x-ratelimit-remaining: 41\r\n",
        "\r\n"
    );

    let parsed = parse_json_output(output.as_bytes()).unwrap();

    assert_eq!(parsed.value, Value::Null);
    assert_eq!(parsed.metadata.status_code, Some(204));
    assert_eq!(parsed.metadata.rate_limit.remaining, Some(41));
}

#[test]
fn parses_conditional_not_modified_metadata() {
    let output = concat!(
        "HTTP/2 304 Not Modified\r\n",
        "etag: \"abc\"\r\n",
        "last-modified: Wed, 01 May 2026 10:00:00 GMT\r\n",
        "x-ratelimit-resource: core\r\n",
        "x-ratelimit-remaining: 4999\r\n",
        "\r\n"
    );

    let parsed = parse_json_output(output.as_bytes()).unwrap();

    assert_eq!(parsed.value, Value::Null);
    assert_eq!(parsed.metadata.status_code, Some(304));
    assert_eq!(
        parsed.metadata.validator,
        Some(HttpCacheValidator {
            etag: Some("\"abc\"".to_string()),
            last_modified: Some("Wed, 01 May 2026 10:00:00 GMT".to_string()),
        })
    );
    assert_eq!(parsed.metadata.rate_limit.resource.as_deref(), Some("core"));
    assert_eq!(parsed.metadata.rate_limit.remaining, Some(4999));
}

#[test]
fn maps_graphql_primary_rate_limit_errors() {
    let metadata = GitHubRateLimitMetadata {
        retry_after_seconds: None,
        reset_epoch_seconds: Some(1_770_000_000),
        resource: Some("graphql".to_string()),
        remaining: Some(0),
        limit: Some(5000),
        used: None,
    };
    let value = json!({
        "errors": [
            {
                "type": "RATE_LIMITED",
                "message": "API rate limit exceeded"
            }
        ]
    });

    let error = graphql_rate_limit_error(&value, &metadata).unwrap();

    match error {
        GitHubError::RateLimited(limit) => {
            assert_eq!(limit.message, "API rate limit exceeded");
            assert_eq!(limit.reset_epoch_seconds, Some(1_770_000_000));
            assert_eq!(limit.remaining, Some(0));
            assert_eq!(limit.limit, Some(5000));
            assert_eq!(limit.resource.as_deref(), Some("graphql"));
        }
        other => panic!("expected primary rate limit error, got {other:?}"),
    }
}

#[test]
fn records_latest_successful_rate_limit() {
    let coordinator = GhCliRequestCoordinator::default();
    coordinator.record_rate_limit_and_attribution(
        GitHubApiFamily::GraphQl,
        "HarborRepositoryPullRequests".to_string(),
        &GitHubRateLimitMetadata {
            retry_after_seconds: None,
            reset_epoch_seconds: Some(1_770_000_000),
            resource: Some("graphql".to_string()),
            remaining: Some(42),
            limit: Some(5000),
            used: Some(12),
        },
        Duration::from_millis(25),
        Some(7),
    );

    let rate_limit = coordinator.latest_rate_limit().unwrap();

    assert_eq!(rate_limit.resource.as_deref(), Some("graphql"));
    assert_eq!(rate_limit.remaining, Some(42));
    assert_eq!(rate_limit.limit, Some(5000));
    assert_eq!(rate_limit.reset_epoch_seconds, Some(1_770_000_000));
    assert_eq!(rate_limit.used, Some(12));

    let attribution = coordinator.latest_request_attribution().unwrap();
    assert_eq!(attribution.operation_name, "HarborRepositoryPullRequests");
    assert_eq!(attribution.family, GitHubApiFamily::GraphQl);
    assert_eq!(attribution.graphql_cost, Some(7));
    assert_eq!(attribution.duration_ms, 25);

    let recent = coordinator.recent_request_attributions();
    assert_eq!(recent.len(), 1);
    assert_eq!(recent[0].operation_name, "HarborRepositoryPullRequests");
}

#[test]
fn mutation_interval_remaining_expires_without_overflow() {
    let state = GhCliRequestCoordinatorState::with_last_mutation_completed_at(
        Instant::now() - MUTATION_REQUEST_INTERVAL * 2,
    );

    assert_eq!(mutation_interval_remaining(&state), None);
}

#[test]
fn maps_secondary_rate_limit_failures() {
    let error = map_failed_status(
        b"HTTP/2 403\r\nretry-after: 60\r\n\r\n",
        b"You have exceeded a secondary rate limit",
    );

    match error {
        GitHubError::SecondaryRateLimited(limit) => {
            assert_eq!(limit.retry_after_seconds, Some(60));
        }
        other => panic!("expected secondary rate limit error, got {other:?}"),
    }
}

#[test]
fn extracts_workflow_log_archive_in_name_order() {
    let mut archive = ZipWriter::new(Cursor::new(Vec::new()));
    archive
        .start_file("2_test.txt", SimpleFileOptions::default())
        .unwrap();
    archive.write_all(b"test\n").unwrap();
    archive
        .start_file("1_build.txt", SimpleFileOptions::default())
        .unwrap();
    archive.write_all(b"build").unwrap();
    let bytes = archive.finish().unwrap().into_inner();

    let text = workflow_log_text_from_zip(&bytes).unwrap();

    assert_eq!(text, "build\ntest\n");
}
