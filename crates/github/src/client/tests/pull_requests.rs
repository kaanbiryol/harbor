use serde_json::{Value, json};

use super::super::{GitHubClient, PullRequestListFilter, test_support::RecordingTransport};
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
