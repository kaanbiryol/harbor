use harbor_domain::{
    FileStatus, FileViewedState, MergeState, PullRequestState, RepoId, ReviewDecision,
};
use serde_json::json;

use super::{
    diff_files_from_value, pull_request_commits_from_value,
    pull_request_file_viewed_states_page_from_graphql_value, pull_request_from_value,
    pull_request_search_count_from_graphql_value, pull_request_search_page_from_graphql_value,
    pull_requests_from_value,
};

#[test]
fn maps_pull_request_list() {
    let value = json!([
        {
            "node_id": "pr-node-42",
            "number": 42,
            "title": "make list rendering fast",
            "body": "Use cached data first",
            "html_url": "https://github.com/acme/app/pull/42",
            "state": "open",
            "draft": false,
            "user": { "login": "octocat" },
            "head": { "ref": "feature/list", "sha": "abc123" },
            "base": { "ref": "main", "sha": "def456" },
            "labels": [{ "name": "performance", "color": "34d399" }],
            "assignees": [
                {
                    "login": "mona",
                    "avatar_url": "https://avatars.githubusercontent.com/u/2?v=4"
                }
            ],
            "requested_reviewers": [
                {
                    "login": "hubot",
                    "avatar_url": null
                }
            ],
            "requested_teams": [
                {
                    "name": "Platform Reviewers",
                    "slug": "platform-reviewers"
                }
            ],
            "mergeable_state": "clean",
            "created_at": "2026-05-10T10:00:00Z"
        }
    ]);

    let pulls = pull_requests_from_value(RepoId::new("acme", "app"), value).unwrap();

    assert_eq!(pulls.len(), 1);
    assert_eq!(pulls[0].repo.full_name(), "acme/app");
    assert_eq!(pulls[0].node_id, "pr-node-42");
    assert_eq!(pulls[0].number, 42);
    assert_eq!(pulls[0].author, "octocat");
    assert_eq!(pulls[0].head_ref, "feature/list");
    assert_eq!(pulls[0].base_ref, "main");
    assert_eq!(pulls[0].state, PullRequestState::Open);
    assert_eq!(pulls[0].merge_state, Some(MergeState::Clean));
    assert_eq!(pulls[0].labels[0].name, "performance");
    assert_eq!(pulls[0].assignees[0].login, "mona");
    assert_eq!(
        pulls[0].assignees[0].avatar_url.as_deref(),
        Some("https://avatars.githubusercontent.com/u/2?v=4")
    );
    assert_eq!(pulls[0].requested_reviewers[0].login, "hubot");
    assert_eq!(pulls[0].requested_teams[0].name, "Platform Reviewers");
    assert_eq!(pulls[0].requested_teams[0].slug, "platform-reviewers");
    assert_eq!(
        pulls[0].created_at.map(|time| time.to_rfc3339()),
        Some("2026-05-10T10:00:00+00:00".to_string())
    );
}

#[test]
fn maps_pull_request_search_states() {
    let value = json!({
        "data": {
            "search": {
                "pageInfo": {
                    "hasNextPage": false,
                    "endCursor": null
                },
                "nodes": [
                    {
                        "__typename": "PullRequest",
                        "id": "pr-node-42",
                        "number": 42,
                        "title": "make list rendering fast",
                        "body": "",
                        "url": "https://github.com/acme/app/pull/42",
                        "state": "OPEN",
                        "isDraft": false,
                        "author": { "login": "octocat" },
                        "repository": {
                            "name": "app",
                            "owner": { "login": "acme" }
                        },
                        "headRefName": "feature/list",
                        "baseRefName": "main",
                        "headRefOid": "abc123",
                        "createdAt": "2026-05-10T10:00:00Z",
                        "reviewDecision": "REVIEW_REQUIRED",
                        "mergeStateStatus": "CLEAN",
                        "statusCheckRollup": {
                            "contexts": {
                                "nodes": [
                                    {
                                        "__typename": "CheckRun",
                                        "status": "COMPLETED",
                                        "conclusion": "SUCCESS"
                                    },
                                    {
                                        "__typename": "CheckRun",
                                        "status": "COMPLETED",
                                        "conclusion": "FAILURE"
                                    },
                                    {
                                        "__typename": "CheckRun",
                                        "status": "IN_PROGRESS",
                                        "conclusion": null
                                    },
                                    {
                                        "__typename": "StatusContext",
                                        "state": "SUCCESS"
                                    }
                                ]
                            }
                        },
                        "labels": {
                            "nodes": [{ "name": "performance", "color": "34d399" }]
                        },
                        "assignees": {
                            "nodes": [{ "login": "mona" }]
                        }
                    },
                    {
                        "__typename": "PullRequest",
                        "id": "pr-node-43",
                        "number": 43,
                        "title": "close stale work",
                        "body": null,
                        "url": "https://github.com/acme/app/pull/43",
                        "state": "CLOSED",
                        "isDraft": false,
                        "author": { "login": "octocat" },
                        "repository": {
                            "name": "app",
                            "owner": { "login": "acme" }
                        },
                        "headRefName": "feature/stale",
                        "baseRefName": "main",
                        "headRefOid": "def456",
                        "reviewDecision": null,
                        "mergeStateStatus": "UNKNOWN",
                        "labels": {
                            "nodes": []
                        }
                    },
                    {
                        "__typename": "PullRequest",
                        "id": "pr-node-44",
                        "number": 44,
                        "title": "merge completed work",
                        "body": null,
                        "url": "https://github.com/acme/app/pull/44",
                        "state": "MERGED",
                        "isDraft": false,
                        "author": { "login": "octocat" },
                        "repository": {
                            "name": "app",
                            "owner": { "login": "acme" }
                        },
                        "headRefName": "feature/done",
                        "baseRefName": "main",
                        "headRefOid": "ghi789",
                        "reviewDecision": "APPROVED",
                        "mergeStateStatus": "CLEAN",
                        "labels": {
                            "nodes": []
                        }
                    }
                ]
            }
        }
    });

    let page = pull_request_search_page_from_graphql_value(value).unwrap();

    assert_eq!(page.pull_requests.len(), 3);
    assert!(!page.has_next_page);
    assert_eq!(page.pull_requests[0].repo.full_name(), "acme/app");
    assert_eq!(page.pull_requests[0].node_id, "pr-node-42");
    assert_eq!(page.pull_requests[0].number, 42);
    assert_eq!(
        page.pull_requests[0].review_decision,
        Some(ReviewDecision::ReviewRequired)
    );
    assert_eq!(page.pull_requests[0].merge_state, Some(MergeState::Clean));
    assert_eq!(page.pull_requests[0].checks_summary.total, 4);
    assert_eq!(page.pull_requests[0].checks_summary.passed, 2);
    assert_eq!(page.pull_requests[0].checks_summary.failed, 1);
    assert_eq!(page.pull_requests[0].checks_summary.pending, 1);
    assert_eq!(page.pull_requests[0].labels[0].name, "performance");
    assert_eq!(page.pull_requests[0].assignees[0].login, "mona");
    assert_eq!(
        page.pull_requests[0]
            .created_at
            .map(|time| time.to_rfc3339()),
        Some("2026-05-10T10:00:00+00:00".to_string())
    );
    assert_eq!(page.pull_requests[1].state, PullRequestState::Closed);
    assert_eq!(page.pull_requests[2].state, PullRequestState::Merged);
    assert_eq!(
        page.pull_requests[2].review_decision,
        Some(ReviewDecision::Approved)
    );
}

#[test]
fn maps_pull_request_search_count() {
    let value = json!({
        "data": {
            "search": {
                "issueCount": 17
            }
        }
    });

    let count = pull_request_search_count_from_graphql_value(value).unwrap();

    assert_eq!(count, 17);
}

#[test]
fn maps_merged_pull_request() {
    let value = json!({
        "number": 9,
        "title": "merged pr",
        "body": null,
        "html_url": "https://github.com/acme/app/pull/9",
        "state": "closed",
        "draft": false,
        "user": null,
        "head": { "ref": "feature/done", "sha": "abc123" },
        "base": { "ref": "main", "sha": "def456" },
        "labels": [],
        "merged": true,
        "mergeable_state": "unknown"
    });

    let pull = pull_request_from_value(RepoId::new("acme", "app"), value).unwrap();

    assert_eq!(pull.state, PullRequestState::Merged);
    assert_eq!(pull.author, "ghost");
}

#[test]
fn maps_pull_request_files_with_missing_patch() {
    let value = json!([
        {
            "filename": "src/app.rs",
            "status": "modified",
            "additions": 12,
            "deletions": 4,
            "changes": 16,
            "patch": "@@ -1 +1 @@"
        },
        {
            "filename": "assets/logo.png",
            "status": "renamed",
            "previous_filename": "assets/old-logo.png",
            "additions": 0,
            "deletions": 0,
            "changes": 0
        }
    ]);

    let files = diff_files_from_value(value).unwrap();

    assert_eq!(files.len(), 2);
    assert_eq!(files[0].status, FileStatus::Modified);
    assert!(files[0].patch.is_some());
    assert_eq!(files[1].status, FileStatus::Renamed);
    assert_eq!(
        files[1].previous_path.as_deref(),
        Some("assets/old-logo.png")
    );
    assert!(files[1].patch.is_none());
    assert_eq!(files[1].viewed_state, FileViewedState::Unviewed);
}

#[test]
fn maps_pull_request_commits() {
    let commits = pull_request_commits_from_value(json!([{
        "sha": "abcdef123456",
        "author": { "login": "octocat", "avatar_url": "https://example.com/avatar.png" },
        "commit": {
            "message": "Add commits panel\n\nWith details",
            "author": { "name": "Octo Cat", "date": "2026-07-12T09:30:00Z" }
        }
    }]))
    .unwrap();

    assert_eq!(commits.len(), 1);
    assert_eq!(commits[0].sha, "abcdef123456");
    assert_eq!(commits[0].author, "octocat");
    assert_eq!(commits[0].message, "Add commits panel\n\nWith details");
    assert!(commits[0].authored_at.is_some());
}

#[test]
fn maps_pull_request_file_viewed_states_from_graphql() {
    let value = json!({
        "data": {
            "repository": {
                "pullRequest": {
                    "id": "pr-node",
                    "files": {
                        "pageInfo": {
                            "hasNextPage": true,
                            "endCursor": "cursor-1"
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
    });

    let page = pull_request_file_viewed_states_page_from_graphql_value(value).unwrap();

    assert!(page.has_next_page);
    assert_eq!(page.end_cursor.as_deref(), Some("cursor-1"));
    assert_eq!(page.file_states.len(), 2);
    assert_eq!(page.file_states[0].path, "src/lib.rs");
    assert_eq!(page.file_states[0].viewed_state, FileViewedState::Viewed);
    assert_eq!(page.file_states[1].path, "src/new.rs");
    assert_eq!(
        page.file_states[1].viewed_state,
        FileViewedState::ChangedSinceViewed
    );
}
