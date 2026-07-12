use harbor_domain::{PullRequestReviewState, ReactionContent, ReviewSide, ReviewThreadState};
use serde_json::json;

use super::{
    pull_request_comments_from_value, pull_request_reviews_from_value,
    review_threads_from_graphql_value,
};

#[test]
fn maps_pull_request_reviews() {
    let value = json!([
        {
            "id": 401,
            "node_id": "review-node-401",
            "state": "APPROVED",
            "body": "ship it",
            "submitted_at": "2026-05-01T11:00:00Z",
            "user": { "login": "octocat" }
        },
        {
            "id": 402,
            "state": "CHANGES_REQUESTED",
            "body": "",
            "submitted_at": null,
            "user": null
        }
    ]);

    let reviews = pull_request_reviews_from_value(value).unwrap();

    assert_eq!(reviews.len(), 2);
    assert_eq!(reviews[0].id, "401");
    assert_eq!(reviews[0].node_id.as_deref(), Some("review-node-401"));
    assert_eq!(reviews[0].author, "octocat");
    assert_eq!(reviews[0].state, PullRequestReviewState::Approved);
    assert_eq!(reviews[0].body.as_deref(), Some("ship it"));
    assert_eq!(reviews[1].author, "ghost");
    assert_eq!(reviews[1].state, PullRequestReviewState::ChangesRequested);
    assert_eq!(reviews[1].body, None);
}

#[test]
fn maps_pull_request_comments() {
    let value = json!([
        {
            "id": 501,
            "body": "Can we do this?",
            "user": {
                "login": "octocat",
                "avatar_url": "https://avatars.githubusercontent.com/u/1?v=4"
            },
            "created_at": "2026-05-01T12:00:00Z",
            "updated_at": "2026-05-01T12:05:00Z"
        },
        {
            "id": 502,
            "body": "",
            "user": null,
            "created_at": "2026-05-01T13:00:00Z",
            "updated_at": null
        }
    ]);

    let comments = pull_request_comments_from_value(value).unwrap();

    assert_eq!(comments.len(), 2);
    assert_eq!(comments[0].id, "501");
    assert_eq!(comments[0].author, "octocat");
    assert_eq!(
        comments[0].author_avatar_url.as_deref(),
        Some("https://avatars.githubusercontent.com/u/1?v=4")
    );
    assert_eq!(comments[0].body, "Can we do this?");
    assert_eq!(comments[1].id, "502");
    assert_eq!(comments[1].author, "ghost");
}

#[test]
fn maps_review_threads_from_graphql() {
    let value: serde_json::Value = serde_json::from_str(
        r#"
        {
          "data": {
            "repository": {
              "pullRequest": {
                "reviewThreads": {
                  "nodes": [
                    {
                      "id": "thread-1",
                      "path": "src/app.rs",
                      "line": 42,
                      "diffSide": "RIGHT",
                      "startLine": 40,
                      "startDiffSide": "RIGHT",
                      "originalLine": 40,
                      "isResolved": false,
                      "isOutdated": false,
                      "comments": {
                        "nodes": [
                          {
                            "id": "comment-1",
                            "url": "https://github.com/octo/harbor/pull/7#discussion_r1",
                            "pullRequestReview": {
                              "id": "review-node-401",
                              "databaseId": 401
                            },
                            "body": "This can be cheaper.",
                            "author": {
                              "login": "reviewer",
                              "avatarUrl": "https://avatars.githubusercontent.com/u/1?v=4"
                            },
                            "createdAt": "2026-05-01T10:00:00Z",
                            "updatedAt": "2026-05-01T10:05:00Z",
                            "path": "src/app.rs",
                            "line": 42,
                            "originalLine": 40,
                            "viewerDidAuthor": false,
                            "viewerCanUpdate": false,
                            "viewerCanDelete": false,
                            "viewerCanReact": true,
                            "reactionGroups": [
                              {
                                "content": "THUMBS_UP",
                                "viewerHasReacted": true,
                                "users": { "totalCount": 3 }
                              },
                              {
                                "content": "HEART",
                                "viewerHasReacted": false,
                                "users": { "totalCount": 1 }
                              }
                            ]
                          },
                          {
                            "id": "comment-2",
                            "body": "Updated.",
                            "author": null,
                            "createdAt": "2026-05-01T10:10:00Z",
                            "updatedAt": null,
                            "path": null,
                            "line": null,
                            "originalLine": null,
                            "viewerDidAuthor": true,
                            "viewerCanUpdate": true,
                            "viewerCanDelete": true,
                            "viewerCanReact": true,
                            "reactionGroups": []
                          }
                        ]
                      }
                    },
                    {
                      "id": "thread-2",
                      "path": "src/old.rs",
                      "line": null,
                      "diffSide": "LEFT",
                      "startLine": null,
                      "startDiffSide": null,
                      "originalLine": 9,
                      "isResolved": false,
                      "isOutdated": true,
                      "comments": { "nodes": [] }
                    }
                  ]
                }
              }
            }
          }
        }
    "#,
    )
    .expect("valid review thread JSON");

    let threads = review_threads_from_graphql_value(value).unwrap();

    assert_eq!(threads.len(), 2);
    assert_eq!(threads[0].id, "thread-1");
    assert_eq!(threads[0].path, "src/app.rs");
    assert_eq!(
        threads[0]
            .range
            .as_ref()
            .map(|range| (range.line, range.start_line)),
        Some((42, Some(40)))
    );
    assert_eq!(threads[0].state, ReviewThreadState::Unresolved);
    assert_eq!(threads[0].comments.len(), 2);
    assert_eq!(threads[0].comments[0].author, "reviewer");
    assert_eq!(
        threads[0].comments[0].url.as_deref(),
        Some("https://github.com/octo/harbor/pull/7#discussion_r1")
    );
    assert_eq!(
        threads[0].comments[0].pull_request_review_id.as_deref(),
        Some("401")
    );
    assert_eq!(
        threads[0].comments[0]
            .pull_request_review_node_id
            .as_deref(),
        Some("review-node-401")
    );
    assert_eq!(
        threads[0].comments[0].author_avatar_url.as_deref(),
        Some("https://avatars.githubusercontent.com/u/1?v=4")
    );
    assert!(!threads[0].comments[0].viewer_did_author);
    assert!(!threads[0].comments[0].viewer_can_update);
    assert!(!threads[0].comments[0].viewer_can_delete);
    assert!(threads[0].comments[0].viewer_can_react);
    assert_eq!(threads[0].comments[0].reactions.len(), 2);
    assert_eq!(
        threads[0].comments[0]
            .reactions
            .iter()
            .map(|reaction| {
                (
                    reaction.content,
                    reaction.count,
                    reaction.viewer_has_reacted,
                )
            })
            .collect::<Vec<_>>(),
        vec![
            (ReactionContent::ThumbsUp, 3, true),
            (ReactionContent::Heart, 1, false),
        ]
    );
    assert_eq!(
        threads[0].comments[0]
            .position
            .as_ref()
            .map(|position| position.line),
        Some(Some(42))
    );
    assert_eq!(threads[0].comments[1].author, "ghost");
    assert!(threads[0].comments[1].viewer_did_author);
    assert!(threads[0].comments[1].viewer_can_update);
    assert!(threads[0].comments[1].viewer_can_delete);
    assert_eq!(threads[1].state, ReviewThreadState::Outdated);
    assert_eq!(
        threads[1]
            .range
            .as_ref()
            .map(|range| (range.side, range.line, range.start_line)),
        Some((ReviewSide::Left, 9, None))
    );
}
