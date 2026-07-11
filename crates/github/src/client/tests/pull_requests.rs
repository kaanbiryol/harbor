use serde_json::{Value, json};

use super::super::{GitHubClient, PullRequestListFilter, test_support::RecordingTransport};
use crate::{ConditionalFetch, HttpCacheValidator, PullRequestPageCursor};
use harbor_domain::{FileViewedState, MergeMethod, RepoId};

#[test]
fn updates_pull_request_body() {
    let transport = RecordingTransport::default();
    *transport
        .graphql_response
        .lock()
        .expect("graphql response mutex should not be poisoned") = Some(json!({
        "data": {
            "updatePullRequest": {
                "pullRequest": {
                    "id": "pr-node",
                    "body": "Updated description"
                }
            }
        }
    }));
    let client = GitHubClient::new(transport.clone());

    smol::block_on(client.update_pull_request_body("pr-node", "Updated description")).unwrap();

    let calls = transport
        .graphql_calls
        .lock()
        .expect("graphql calls mutex should not be poisoned");
    assert_eq!(calls.len(), 1);
    assert!(calls[0].0.contains("HarborUpdatePullRequest"));
    assert_eq!(
        calls[0].1,
        json!({
            "input": {
                "pullRequestId": "pr-node",
                "body": "Updated description",
            }
        })
    );
}

#[test]
fn adds_pull_request_people_and_labels() {
    let transport = RecordingTransport::default();
    let client = GitHubClient::new(transport.clone());

    smol::block_on(client.request_pull_request_reviewer("acme", "app", 7, "reviewer")).unwrap();
    smol::block_on(client.add_pull_request_assignee("acme", "app", 7, "assignee")).unwrap();
    smol::block_on(client.add_pull_request_label("acme", "app", 7, "needs-review")).unwrap();

    let posts = transport
        .posts
        .lock()
        .expect("posts mutex should not be poisoned");
    assert_eq!(
        posts.as_slice(),
        [
            (
                "/repos/acme/app/pulls/7/requested_reviewers".to_string(),
                json!({ "reviewers": ["reviewer"] }),
            ),
            (
                "/repos/acme/app/issues/7/assignees".to_string(),
                json!({ "assignees": ["assignee"] }),
            ),
            (
                "/repos/acme/app/issues/7/labels".to_string(),
                json!({ "labels": ["needs-review"] }),
            ),
        ]
    );
}

#[test]
fn queries_repository_pull_request_filters() {
    for (filter, query) in [
        (
            PullRequestListFilter::Open,
            "repo:acme/app is:pr is:open archived:false sort:created-desc",
        ),
        (
            PullRequestListFilter::Closed,
            "repo:acme/app is:pr is:closed archived:false sort:created-desc",
        ),
        (
            PullRequestListFilter::NeedsReview,
            "repo:acme/app is:pr is:open archived:false review-requested:@me sort:created-desc",
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
        assert!(calls[0].0.contains("first: $first"));
        assert!(!calls[0].0.contains("statusCheckRollup"));
        assert!(!calls[0].0.contains("labels(first:"));
        assert_eq!(
            calls[0].1,
            json!({
                "searchQuery": query,
                "first": 100,
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
        "repo:acme/app is:pr is:open archived:false sort:created-desc"
    );
    assert_eq!(calls[0].1["after"], Value::Null);
    assert_eq!(calls[0].1["first"], 100);
    assert_eq!(calls[1].1["after"], "cursor-1");
    assert_eq!(calls[1].1["first"], 100);
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
            "searchQuery": "repo:acme/app is:pr is:open archived:false review-requested:@me sort:created-desc",
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
                "created_at": "2026-05-01T09:00:00Z",
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
    assert_eq!(
        value[0].created_at.map(|time| time.to_rfc3339()),
        Some("2026-05-01T09:00:00+00:00".to_string())
    );
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
    assert!(
        calls[0]
            .1
            .contains(&("sort".to_string(), "created".to_string()))
    );
    assert!(
        calls[0]
            .1
            .contains(&("direction".to_string(), "desc".to_string()))
    );
}

#[test]
fn lists_light_pull_request_pages_with_twenty_rows() {
    let transport = RecordingTransport::default();
    *transport
        .get_response
        .lock()
        .expect("get response mutex should not be poisoned") = Some(json!([]));
    let client = GitHubClient::new(transport.clone());

    let result = smol::block_on(client.list_repository_pull_requests_light_page(
        &RepoId::new("acme", "app"),
        PullRequestListFilter::Open,
        Some(PullRequestPageCursor::RestPage(2)),
        20,
        None,
    ))
    .unwrap();

    let ConditionalFetch::Modified { value, .. } = result else {
        panic!("expected modified response");
    };
    assert!(value.pull_requests.is_empty());
    assert_eq!(value.next_cursor, None);

    let conditional_calls = transport
        .conditional_gets
        .lock()
        .expect("conditional gets mutex should not be poisoned");
    assert!(conditional_calls.is_empty());
    let calls = transport
        .gets
        .lock()
        .expect("gets mutex should not be poisoned");
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].0, "/repos/acme/app/pulls");
    assert!(
        calls[0]
            .1
            .contains(&("per_page".to_string(), "20".to_string()))
    );
    assert!(
        calls[0]
            .1
            .contains(&("sort".to_string(), "created".to_string()))
    );
    assert!(
        calls[0]
            .1
            .contains(&("direction".to_string(), "desc".to_string()))
    );
    assert!(calls[0].1.contains(&("page".to_string(), "2".to_string())));
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
fn lists_pull_request_files_with_viewer_viewed_state() {
    let transport = RecordingTransport::default();
    *transport
        .get_response
        .lock()
        .expect("get response mutex should not be poisoned") = Some(json!([
        {
            "filename": "src/lib.rs",
            "status": "modified",
            "additions": 2,
            "deletions": 1,
            "changes": 3,
            "patch": "@@ -1 +1 @@\n-old\n+new\n"
        },
        {
            "filename": "src/new.rs",
            "status": "added",
            "additions": 1,
            "deletions": 0,
            "changes": 1,
            "patch": "@@ -0,0 +1 @@\n+new\n"
        }
    ]));
    *transport
        .graphql_response
        .lock()
        .expect("graphql response mutex should not be poisoned") = Some(json!({
        "data": {
            "repository": {
                "pullRequest": {
                    "id": "pr-node",
                    "files": {
                        "pageInfo": {
                            "hasNextPage": false,
                            "endCursor": null
                        },
                        "nodes": [
                            {
                                "path": "src/lib.rs",
                                "viewerViewedState": "VIEWED"
                            },
                            {
                                "path": "src/new.rs",
                                "viewerViewedState": "DISMISSED"
                            }
                        ]
                    }
                }
            }
        }
    }));
    let client = GitHubClient::new(transport.clone());

    let files = smol::block_on(client.list_pull_request_files("acme", "app", 7)).unwrap();

    assert_eq!(files.len(), 2);
    assert_eq!(files[0].path, "src/lib.rs");
    assert_eq!(files[0].viewed_state, FileViewedState::Viewed);
    assert!(files[0].patch.is_some());
    assert_eq!(files[1].path, "src/new.rs");
    assert_eq!(files[1].viewed_state, FileViewedState::ChangedSinceViewed);

    let gets = transport
        .gets
        .lock()
        .expect("gets mutex should not be poisoned");
    assert_eq!(gets.len(), 1);
    assert_eq!(gets[0].0, "/repos/acme/app/pulls/7/files");
    assert!(gets[0].1.contains(&("per_page".into(), "100".into())));
    assert!(gets[0].1.contains(&("page".into(), "1".into())));

    let graphql_calls = transport
        .graphql_calls
        .lock()
        .expect("graphql calls mutex should not be poisoned");
    assert_eq!(graphql_calls.len(), 1);
    assert!(
        graphql_calls[0]
            .0
            .contains("HarborPullRequestFileViewedStates")
    );
    assert_eq!(
        graphql_calls[0].1,
        json!({
            "owner": "acme",
            "repo": "app",
            "number": 7,
            "first": 100,
            "after": null,
        })
    );
}

#[test]
fn marks_pull_request_file_viewed() {
    let transport = RecordingTransport::default();
    *transport
        .graphql_response
        .lock()
        .expect("graphql response mutex should not be poisoned") = Some(json!({
        "data": {
            "markFileAsViewed": {
                "pullRequest": {
                    "id": "pr-node"
                }
            }
        }
    }));
    let client = GitHubClient::new(transport.clone());

    smol::block_on(client.mark_pull_request_file_viewed("pr-node", "src/lib.rs")).unwrap();

    let graphql_calls = transport
        .graphql_calls
        .lock()
        .expect("graphql calls mutex should not be poisoned");
    assert_eq!(graphql_calls.len(), 1);
    assert!(graphql_calls[0].0.contains("HarborMarkFileAsViewed"));
    assert_eq!(
        graphql_calls[0].1,
        json!({
            "input": {
                "pullRequestId": "pr-node",
                "path": "src/lib.rs",
            }
        })
    );
}

#[test]
fn unmarks_pull_request_file_viewed() {
    let transport = RecordingTransport::default();
    *transport
        .graphql_response
        .lock()
        .expect("graphql response mutex should not be poisoned") = Some(json!({
        "data": {
            "unmarkFileAsViewed": {
                "pullRequest": {
                    "id": "pr-node"
                }
            }
        }
    }));
    let client = GitHubClient::new(transport.clone());

    smol::block_on(client.unmark_pull_request_file_viewed("pr-node", "src/lib.rs")).unwrap();

    let graphql_calls = transport
        .graphql_calls
        .lock()
        .expect("graphql calls mutex should not be poisoned");
    assert_eq!(graphql_calls.len(), 1);
    assert!(graphql_calls[0].0.contains("HarborUnmarkFileAsViewed"));
    assert_eq!(
        graphql_calls[0].1,
        json!({
            "input": {
                "pullRequestId": "pr-node",
                "path": "src/lib.rs",
            }
        })
    );
}

#[test]
fn posts_pull_request_approval() {
    let transport = RecordingTransport::default();
    let client = GitHubClient::new(transport.clone());

    smol::block_on(client.approve_pull_request("acme", "app", 7, None)).unwrap();

    let posts = transport
        .posts
        .lock()
        .expect("posts mutex should not be poisoned");
    assert_eq!(posts.len(), 1);
    assert_eq!(posts[0].0, "/repos/acme/app/pulls/7/reviews");
    assert_eq!(posts[0].1, json!({ "event": "APPROVE" }));
}

#[test]
fn posts_pull_request_approval_with_body() {
    let transport = RecordingTransport::default();
    let client = GitHubClient::new(transport.clone());

    smol::block_on(client.approve_pull_request("acme", "app", 7, Some("Looks good."))).unwrap();

    let posts = transport
        .posts
        .lock()
        .expect("posts mutex should not be poisoned");
    assert_eq!(posts.len(), 1);
    assert_eq!(posts[0].0, "/repos/acme/app/pulls/7/reviews");
    assert_eq!(
        posts[0].1,
        json!({
            "event": "APPROVE",
            "body": "Looks good.",
        })
    );
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

    smol::block_on(client.merge_pull_request("acme", "app", 7, "abc123", MergeMethod::Squash))
        .unwrap();

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

#[test]
fn puts_pull_request_merge_methods() {
    for (method, expected) in [
        (MergeMethod::Merge, "merge"),
        (MergeMethod::Rebase, "rebase"),
    ] {
        let transport = RecordingTransport::default();
        let client = GitHubClient::new(transport.clone());

        smol::block_on(client.merge_pull_request("acme", "app", 7, "abc123", method)).unwrap();

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
                "merge_method": expected,
            })
        );
    }
}
