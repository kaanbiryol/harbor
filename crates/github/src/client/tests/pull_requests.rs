use serde_json::{Value, json};

use super::super::{GitHubClient, PullRequestListFilter, test_support::RecordingTransport};
use crate::{ConditionalFetch, HttpCacheValidator};
use harbor_domain::RepoId;

#[test]
fn queries_repository_pull_request_filters() {
    for (filter, query) in [
        (
            PullRequestListFilter::Open,
            "repo:acme/app is:pr is:open archived:false sort:updated-desc",
        ),
        (
            PullRequestListFilter::Closed,
            "repo:acme/app is:pr is:closed archived:false sort:updated-desc",
        ),
        (
            PullRequestListFilter::NeedsReview,
            "repo:acme/app is:pr is:open archived:false review-requested:@me sort:updated-desc",
        ),
    ] {
        let transport = RecordingTransport::default();
        *transport
            .graphql_response
            .lock()
            .expect("graphql response mutex should not be poisoned") = Some(json!({
            "data": {
                "search": {
                    "pageInfo": {
                        "hasNextPage": false,
                        "endCursor": null
                    },
                    "nodes": []
                }
            }
        }));
        let client = GitHubClient::new(transport.clone());

        smol::block_on(client.list_repository_pull_requests(&RepoId::new("acme", "app"), filter))
            .unwrap();

        let calls = transport
            .graphql_calls
            .lock()
            .expect("graphql calls mutex should not be poisoned");
        assert_eq!(calls.len(), 1);
        assert!(calls[0].0.contains("HarborRepositoryPullRequests"));
        assert!(calls[0].0.contains("first: 100"));
        assert!(!calls[0].0.contains("statusCheckRollup"));
        assert!(!calls[0].0.contains("labels(first:"));
        assert_eq!(
            calls[0].1,
            json!({
                "searchQuery": query,
                "after": null,
            })
        );
    }
}

#[test]
fn paginates_repository_pull_requests() {
    let transport = RecordingTransport::default();
    *transport
        .graphql_responses
        .lock()
        .expect("graphql responses mutex should not be poisoned") = vec![
        json!({
            "data": {
                "search": {
                    "pageInfo": {
                        "hasNextPage": true,
                        "endCursor": "cursor-1"
                    },
                    "nodes": []
                }
            }
        }),
        json!({
            "data": {
                "search": {
                    "pageInfo": {
                        "hasNextPage": false,
                        "endCursor": null
                    },
                    "nodes": []
                }
            }
        }),
    ];
    let client = GitHubClient::new(transport.clone());

    smol::block_on(
        client.list_repository_pull_requests(
            &RepoId::new("acme", "app"),
            PullRequestListFilter::Open,
        ),
    )
    .unwrap();

    let calls = transport
        .graphql_calls
        .lock()
        .expect("graphql calls mutex should not be poisoned");
    assert_eq!(calls.len(), 2);
    assert_eq!(
        calls[0].1["searchQuery"],
        "repo:acme/app is:pr is:open archived:false sort:updated-desc"
    );
    assert_eq!(calls[0].1["after"], Value::Null);
    assert_eq!(calls[1].1["after"], "cursor-1");
}

#[test]
fn counts_repository_pull_requests() {
    let transport = RecordingTransport::default();
    *transport
        .graphql_response
        .lock()
        .expect("graphql response mutex should not be poisoned") = Some(json!({
        "data": {
            "search": {
                "issueCount": 12
            }
        }
    }));
    let client = GitHubClient::new(transport.clone());

    let count = smol::block_on(client.count_repository_pull_requests(
        &RepoId::new("acme", "app"),
        PullRequestListFilter::NeedsReview,
    ))
    .unwrap();

    assert_eq!(count, 12);
    let calls = transport
        .graphql_calls
        .lock()
        .expect("graphql calls mutex should not be poisoned");
    assert_eq!(calls.len(), 1);
    assert!(calls[0].0.contains("HarborRepositoryPullRequestCount"));
    assert!(calls[0].0.contains("issueCount"));
    assert!(!calls[0].0.contains("nodes"));
    assert_eq!(
        calls[0].1,
        json!({
            "searchQuery": "repo:acme/app is:pr is:open archived:false review-requested:@me sort:updated-desc",
        })
    );
}

#[test]
fn lists_light_pull_requests_with_conditional_validator() {
    let transport = RecordingTransport::default();
    let validator = HttpCacheValidator {
        etag: Some("\"etag\"".to_string()),
        last_modified: None,
    };
    *transport
        .conditional_get_response
        .lock()
        .expect("conditional get response mutex should not be poisoned") =
        Some(ConditionalFetch::Modified {
            value: json!([{
                "node_id": "pr-node",
                "number": 7,
                "title": "Add feature",
                "body": null,
                "html_url": "https://github.com/acme/app/pull/7",
                "state": "open",
                "draft": false,
                "user": { "login": "octocat" },
                "head": { "ref": "feature", "sha": "abc123" },
                "base": { "ref": "main", "sha": "def456" },
                "labels": [],
                "merged": false,
                "mergeable_state": "clean",
                "updated_at": "2026-05-01T10:00:00Z"
            }]),
            validator: Some(validator.clone()),
        });
    let client = GitHubClient::new(transport.clone());

    let result = smol::block_on(client.list_repository_pull_requests_light(
        &RepoId::new("acme", "app"),
        PullRequestListFilter::Open,
        Some(&validator),
    ))
    .unwrap();

    let ConditionalFetch::Modified {
        value,
        validator: returned_validator,
    } = result
    else {
        panic!("expected modified response");
    };
    assert_eq!(value.len(), 1);
    assert_eq!(value[0].number, 7);
    assert_eq!(returned_validator, Some(validator.clone()));

    let calls = transport
        .conditional_gets
        .lock()
        .expect("conditional gets mutex should not be poisoned");
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].0, "/repos/acme/app/pulls");
    assert_eq!(calls[0].2, Some(validator));
    assert!(
        calls[0]
            .1
            .contains(&("state".to_string(), "open".to_string()))
    );
    assert!(
        calls[0]
            .1
            .contains(&("per_page".to_string(), "100".to_string()))
    );
}

#[test]
fn enriches_pull_requests_by_node_ids() {
    let transport = RecordingTransport::default();
    *transport
        .graphql_response
        .lock()
        .expect("graphql response mutex should not be poisoned") = Some(json!({
        "data": {
            "nodes": [{
                "__typename": "PullRequest",
                "id": "pr-node",
                "reviewDecision": "APPROVED",
                "mergeStateStatus": "CLEAN"
            }]
        }
    }));
    let client = GitHubClient::new(transport.clone());

    let enrichments =
        smol::block_on(client.enrich_pull_requests_by_node_ids(&["pr-node".into()])).unwrap();

    assert_eq!(enrichments.len(), 1);
    assert_eq!(enrichments[0].node_id, "pr-node");
    assert_eq!(enrichments[0].checks_summary.total, 0);

    let calls = transport
        .graphql_calls
        .lock()
        .expect("graphql calls mutex should not be poisoned");
    assert_eq!(calls.len(), 1);
    assert!(calls[0].0.contains("HarborPullRequestEnrichment"));
    assert_eq!(calls[0].1, json!({ "ids": ["pr-node"] }));
}

#[test]
fn posts_pull_request_approval() {
    let transport = RecordingTransport::default();
    let client = GitHubClient::new(transport.clone());

    smol::block_on(client.approve_pull_request("acme", "app", 7)).unwrap();

    let posts = transport
        .posts
        .lock()
        .expect("posts mutex should not be poisoned");
    assert_eq!(posts.len(), 1);
    assert_eq!(posts[0].0, "/repos/acme/app/pulls/7/reviews");
    assert_eq!(posts[0].1, json!({ "event": "APPROVE" }));
}

#[test]
fn posts_pull_request_change_request() {
    let transport = RecordingTransport::default();
    let client = GitHubClient::new(transport.clone());

    smol::block_on(client.request_pull_request_changes(
        "acme",
        "app",
        7,
        "Please address the failing path.",
    ))
    .unwrap();

    let posts = transport
        .posts
        .lock()
        .expect("posts mutex should not be poisoned");
    assert_eq!(posts.len(), 1);
    assert_eq!(posts[0].0, "/repos/acme/app/pulls/7/reviews");
    assert_eq!(
        posts[0].1,
        json!({
            "event": "REQUEST_CHANGES",
            "body": "Please address the failing path.",
        })
    );
}

#[test]
fn puts_pull_request_squash_merge() {
    let transport = RecordingTransport::default();
    let client = GitHubClient::new(transport.clone());

    smol::block_on(client.merge_pull_request("acme", "app", 7, "abc123")).unwrap();

    let puts = transport
        .puts
        .lock()
        .expect("puts mutex should not be poisoned");
    assert_eq!(puts.len(), 1);
    assert_eq!(puts[0].0, "/repos/acme/app/pulls/7/merge");
    assert_eq!(
        puts[0].1,
        json!({
            "sha": "abc123",
            "merge_method": "squash",
        })
    );
}
