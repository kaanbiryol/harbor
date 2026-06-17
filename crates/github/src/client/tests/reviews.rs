use serde_json::{Value, json};

use super::super::{
    GitHubClient, SubmitPullRequestReviewEvent,
    test_support::{REVIEW_COMMENT_PAGE_SIZE, RecordingTransport, review_thread_comment_json},
};
use harbor_domain::{ReactionContent, ReviewCommentRange, ReviewSide};

#[test]
fn gets_pull_request_reviews_endpoint() {
    let transport = RecordingTransport::default();
    *transport
        .get_response
        .lock()
        .expect("get response mutex should not be poisoned") = Some(json!([]));
    let client = GitHubClient::new(transport.clone());

    smol::block_on(client.list_pull_request_reviews("acme", "app", 7)).unwrap();

    let gets = transport
        .gets
        .lock()
        .expect("gets mutex should not be poisoned");
    assert_eq!(gets.len(), 1);
    assert_eq!(gets[0].0, "/repos/acme/app/pulls/7/reviews");
    assert_eq!(gets[0].1, vec![("per_page".to_string(), "100".to_string())]);
}

#[test]
fn paginates_pull_request_comments_endpoint() {
    let transport = RecordingTransport::default();
    *transport
        .get_responses
        .lock()
        .expect("get responses mutex should not be poisoned") = vec![
        Value::Array(
            (0..100)
                .map(|index| {
                    json!({
                        "id": index,
                        "body": "Can we do this?",
                        "user": { "login": "octocat", "avatar_url": null },
                        "created_at": "2026-05-01T10:00:00Z",
                        "updated_at": null,
                    })
                })
                .collect(),
        ),
        json!([
            {
                "id": 101,
                "body": "Follow-up",
                "user": null,
                "created_at": "2026-05-01T10:05:00Z",
                "updated_at": null,
            }
        ]),
    ];
    let client = GitHubClient::new(transport.clone());

    let comments = smol::block_on(client.list_pull_request_comments("acme", "app", 7)).unwrap();

    assert_eq!(comments.len(), 101);
    let gets = transport
        .gets
        .lock()
        .expect("gets mutex should not be poisoned");
    assert_eq!(gets.len(), 2);
    assert_eq!(gets[0].0, "/repos/acme/app/issues/7/comments");
    assert_eq!(
        gets[0].1,
        vec![
            ("per_page".to_string(), "100".to_string()),
            ("page".to_string(), "1".to_string()),
        ]
    );
    assert_eq!(
        gets[1].1,
        vec![
            ("per_page".to_string(), "100".to_string()),
            ("page".to_string(), "2".to_string()),
        ]
    );
}

#[test]
fn counts_pull_request_review_comments_endpoint() {
    let transport = RecordingTransport::default();
    *transport
        .get_responses
        .lock()
        .expect("get responses mutex should not be poisoned") = vec![
        Value::Array((0..REVIEW_COMMENT_PAGE_SIZE).map(|_| json!({})).collect()),
        json!([{}, {}]),
    ];
    let client = GitHubClient::new(transport.clone());

    let count = smol::block_on(client.pull_request_review_comment_count("acme", "app", 7, "12345"))
        .unwrap();

    assert_eq!(count, REVIEW_COMMENT_PAGE_SIZE + 2);
    let gets = transport
        .gets
        .lock()
        .expect("gets mutex should not be poisoned");
    assert_eq!(gets.len(), 2);
    assert_eq!(gets[0].0, "/repos/acme/app/pulls/7/reviews/12345/comments");
    assert_eq!(
        gets[0].1,
        vec![
            ("per_page".to_string(), "100".to_string()),
            ("page".to_string(), "1".to_string()),
        ]
    );
    assert_eq!(
        gets[1].1,
        vec![
            ("per_page".to_string(), "100".to_string()),
            ("page".to_string(), "2".to_string()),
        ]
    );
}

#[test]
fn queries_pull_request_review_threads() {
    let transport = RecordingTransport::default();
    *transport
        .graphql_response
        .lock()
        .expect("graphql response mutex should not be poisoned") = Some(json!({
        "data": {
            "repository": {
                "pullRequest": {
                    "reviewThreads": {
                        "nodes": []
                    }
                }
            }
        }
    }));
    let client = GitHubClient::new(transport.clone());

    smol::block_on(client.list_review_threads("acme", "app", 7)).unwrap();

    let calls = transport
        .graphql_calls
        .lock()
        .expect("graphql calls mutex should not be poisoned");
    assert_eq!(calls.len(), 1);
    assert!(calls[0].0.contains("reviewThreads(first: $threadPageSize"));
    assert!(calls[0].0.contains("comments(first: $commentPageSize"));
    assert_eq!(
        calls[0].1,
        json!({
            "owner": "acme",
            "repo": "app",
            "number": 7,
            "after": null,
            "threadPageSize": 5,
            "commentPageSize": 1,
        })
    );
}

#[test]
fn paginates_pull_request_review_threads() {
    let transport = RecordingTransport::default();
    *transport
        .graphql_responses
        .lock()
        .expect("graphql responses mutex should not be poisoned") = vec![
        json!({
            "data": {
                "repository": {
                    "pullRequest": {
                        "reviewThreads": {
                            "pageInfo": {
                                "hasNextPage": true,
                                "endCursor": "cursor-1"
                            },
                            "nodes": []
                        },
                    }
                }
            }
        }),
        json!({
            "data": {
                "repository": {
                    "pullRequest": {
                        "reviewThreads": {
                            "pageInfo": {
                                "hasNextPage": false,
                                "endCursor": null
                            },
                            "nodes": []
                        }
                    }
                }
            }
        }),
    ];
    let client = GitHubClient::new(transport.clone());

    smol::block_on(client.list_review_threads("acme", "app", 7)).unwrap();

    let calls = transport
        .graphql_calls
        .lock()
        .expect("graphql calls mutex should not be poisoned");
    assert_eq!(calls.len(), 2);
    assert_eq!(calls[0].1["after"], Value::Null);
    assert_eq!(calls[1].1["after"], "cursor-1");
    assert_eq!(calls[0].1["threadPageSize"], 5);
    assert_eq!(calls[0].1["commentPageSize"], 1);
}

#[test]
fn paginates_pull_request_review_thread_comments() {
    let transport = RecordingTransport::default();
    *transport
        .graphql_responses
        .lock()
        .expect("graphql responses mutex should not be poisoned") = vec![
        json!({
            "data": {
                "repository": {
                    "pullRequest": {
                        "reviewThreads": {
                            "pageInfo": {
                                "hasNextPage": false,
                                "endCursor": null
                            },
                            "nodes": [
                                {
                                    "id": "thread-1",
                                    "path": "src/app.rs",
                                    "line": 42,
                                    "diffSide": "RIGHT",
                                    "startLine": null,
                                    "startDiffSide": null,
                                    "originalLine": 42,
                                    "isResolved": false,
                                    "isOutdated": false,
                                    "comments": {
                                        "pageInfo": {
                                            "hasNextPage": true,
                                            "endCursor": "comment-cursor-1"
                                        },
                                        "nodes": [
                                            review_thread_comment_json("comment-1", "First comment")
                                        ]
                                    }
                                },
                                {
                                    "id": "thread-2",
                                    "path": "src/app.rs",
                                    "line": 44,
                                    "diffSide": "RIGHT",
                                    "startLine": null,
                                    "startDiffSide": null,
                                    "originalLine": 44,
                                    "isResolved": false,
                                    "isOutdated": false,
                                    "comments": {
                                        "pageInfo": {
                                            "hasNextPage": true,
                                            "endCursor": "comment-cursor-2"
                                        },
                                        "nodes": [
                                            review_thread_comment_json("comment-3", "Third comment")
                                        ]
                                    }
                                }
                            ]
                        }
                    }
                }
            }
        }),
        json!({
            "data": {
                "node": {
                    "comments": {
                        "pageInfo": {
                            "hasNextPage": false,
                            "endCursor": null
                        },
                        "nodes": [
                            review_thread_comment_json("comment-2", "Second comment")
                        ]
                    }
                }
            }
        }),
        json!({
            "data": {
                "node": {
                    "comments": {
                        "pageInfo": {
                            "hasNextPage": false,
                            "endCursor": null
                        },
                        "nodes": [
                            review_thread_comment_json("comment-4", "Fourth comment")
                        ]
                    }
                }
            }
        }),
    ];
    let client = GitHubClient::new(transport.clone());

    let threads = smol::block_on(client.list_review_threads("acme", "app", 7)).unwrap();

    assert_eq!(threads.len(), 2);
    assert_eq!(
        threads[0]
            .comments
            .iter()
            .map(|comment| comment.id.as_str())
            .collect::<Vec<_>>(),
        vec!["comment-1", "comment-2"]
    );
    assert_eq!(
        threads[1]
            .comments
            .iter()
            .map(|comment| comment.id.as_str())
            .collect::<Vec<_>>(),
        vec!["comment-3", "comment-4"]
    );
    let calls = transport
        .graphql_calls
        .lock()
        .expect("graphql calls mutex should not be poisoned");
    assert_eq!(calls.len(), 3);
    assert!(calls[1].0.contains("HarborPullRequestReviewThreadComments"));
    assert_eq!(
        calls[1].1,
        json!({
            "threadId": "thread-1",
            "after": "comment-cursor-1",
            "commentPageSize": 50,
        })
    );
    assert_eq!(
        calls[2].1,
        json!({
            "threadId": "thread-2",
            "after": "comment-cursor-2",
            "commentPageSize": 50,
        })
    );
}

#[test]
fn keeps_partial_review_thread_comments_when_pagination_budget_is_hit() {
    let transport = RecordingTransport::default();
    let mut responses = vec![json!({
        "data": {
            "repository": {
                "pullRequest": {
                    "reviewThreads": {
                        "pageInfo": {
                            "hasNextPage": false,
                            "endCursor": null
                        },
                        "nodes": [
                            {
                                "id": "thread-1",
                                "path": "src/app.rs",
                                "line": 42,
                                "diffSide": "RIGHT",
                                "startLine": null,
                                "startDiffSide": null,
                                "originalLine": 42,
                                "isResolved": false,
                                "isOutdated": false,
                                "comments": {
                                    "pageInfo": {
                                        "hasNextPage": true,
                                        "endCursor": "comment-cursor-1"
                                    },
                                    "nodes": [
                                        review_thread_comment_json("comment-1", "First comment")
                                    ]
                                }
                            }
                        ]
                    }
                }
            }
        }
    })];
    for index in 2..=9 {
        responses.push(json!({
            "data": {
                "node": {
                    "comments": {
                        "pageInfo": {
                            "hasNextPage": true,
                            "endCursor": format!("comment-cursor-{index}")
                        },
                        "nodes": [
                            review_thread_comment_json(
                                &format!("comment-{index}"),
                                &format!("Comment {index}")
                            )
                        ]
                    }
                }
            }
        }));
    }
    *transport
        .graphql_responses
        .lock()
        .expect("graphql responses mutex should not be poisoned") = responses;
    let client = GitHubClient::new(transport.clone());

    let threads = smol::block_on(client.list_review_threads("acme", "app", 7)).unwrap();

    assert_eq!(threads.len(), 1);
    assert_eq!(threads[0].comments.len(), 9);
    let calls = transport
        .graphql_calls
        .lock()
        .expect("graphql calls mutex should not be poisoned");
    assert_eq!(calls.len(), 9);
}

#[test]
fn posts_single_review_comment_body() {
    let transport = RecordingTransport::default();
    let client = GitHubClient::new(transport.clone());
    let range = ReviewCommentRange {
        path: "src/lib.rs".to_string(),
        line: 42,
        side: ReviewSide::Right,
        start_line: Some(40),
        start_side: Some(ReviewSide::Right),
    };

    smol::block_on(client.create_pull_request_review_comment(
        "acme",
        "app",
        7,
        "abc123",
        &range,
        "Can we simplify this?",
    ))
    .unwrap();

    let posts = transport
        .posts
        .lock()
        .expect("posts mutex should not be poisoned");
    assert_eq!(posts[0].0, "/repos/acme/app/pulls/7/comments");
    assert_eq!(
        posts[0].1,
        json!({
            "body": "Can we simplify this?",
            "commit_id": "abc123",
            "path": "src/lib.rs",
            "line": 42,
            "side": "RIGHT",
            "start_line": 40,
            "start_side": "RIGHT",
        })
    );
}

#[test]
fn starts_pull_request_review_with_thread_variables() {
    let transport = RecordingTransport::default();
    *transport
        .graphql_response
        .lock()
        .expect("graphql response mutex should not be poisoned") = Some(json!({
        "data": {
            "addPullRequestReview": {
                "pullRequestReview": {
                    "id": "review-node",
                    "state": "PENDING"
                }
            }
        }
    }));
    let client = GitHubClient::new(transport.clone());
    let range = ReviewCommentRange {
        path: "src/lib.rs".to_string(),
        line: 9,
        side: ReviewSide::Left,
        start_line: None,
        start_side: None,
    };

    let review_node_id = smol::block_on(client.start_pull_request_review(
        "pr-node",
        "abc123",
        &range,
        "This moved?",
    ))
    .unwrap();

    assert_eq!(review_node_id, "review-node");
    let calls = transport
        .graphql_calls
        .lock()
        .expect("graphql calls mutex should not be poisoned");
    assert!(calls[0].0.contains("addPullRequestReview"));
    assert_eq!(
        calls[0].1,
        json!({
            "input": {
                "pullRequestId": "pr-node",
                "commitOID": "abc123",
                "threads": [{
                    "body": "This moved?",
                    "path": "src/lib.rs",
                    "line": 9,
                    "side": "LEFT",
                }]
            }
        })
    );
}

#[test]
fn adds_pending_review_thread_variables() {
    let transport = RecordingTransport::default();
    *transport
        .graphql_response
        .lock()
        .expect("graphql response mutex should not be poisoned") = Some(json!({
        "data": {
            "addPullRequestReviewThread": {
                "thread": { "id": "thread-node" }
            }
        }
    }));
    let client = GitHubClient::new(transport.clone());
    let range = ReviewCommentRange {
        path: "src/lib.rs".to_string(),
        line: 42,
        side: ReviewSide::Right,
        start_line: Some(40),
        start_side: Some(ReviewSide::Right),
    };

    smol::block_on(client.add_pending_review_thread(
        "review-node",
        &range,
        "Can we simplify this?",
    ))
    .unwrap();

    let calls = transport
        .graphql_calls
        .lock()
        .expect("graphql calls mutex should not be poisoned");
    assert!(calls[0].0.contains("addPullRequestReviewThread"));
    assert_eq!(
        calls[0].1,
        json!({
            "input": {
                "pullRequestReviewId": "review-node",
                "body": "Can we simplify this?",
                "path": "src/lib.rs",
                "line": 42,
                "side": "RIGHT",
                "startLine": 40,
                "startSide": "RIGHT",
            }
        })
    );
}

#[test]
fn adds_review_thread_reply_variables() {
    let transport = RecordingTransport::default();
    *transport
        .graphql_response
        .lock()
        .expect("graphql response mutex should not be poisoned") = Some(json!({
        "data": {
            "addPullRequestReviewThreadReply": {
                "comment": { "id": "comment-node" }
            }
        }
    }));
    let client = GitHubClient::new(transport.clone());

    smol::block_on(client.add_review_thread_reply(
        "thread-node",
        Some("review-node"),
        "Replying here.",
    ))
    .unwrap();

    let calls = transport
        .graphql_calls
        .lock()
        .expect("graphql calls mutex should not be poisoned");
    assert!(calls[0].0.contains("addPullRequestReviewThreadReply"));
    assert_eq!(
        calls[0].1,
        json!({
            "input": {
                "pullRequestReviewThreadId": "thread-node",
                "pullRequestReviewId": "review-node",
                "body": "Replying here.",
            }
        })
    );
}

#[test]
fn resolves_review_thread_variables() {
    let transport = RecordingTransport::default();
    *transport
        .graphql_response
        .lock()
        .expect("graphql response mutex should not be poisoned") = Some(json!({
        "data": {
            "resolveReviewThread": {
                "thread": {
                    "id": "thread-node",
                    "isResolved": true
                }
            }
        }
    }));
    let client = GitHubClient::new(transport.clone());

    smol::block_on(client.resolve_review_thread("thread-node")).unwrap();

    let calls = transport
        .graphql_calls
        .lock()
        .expect("graphql calls mutex should not be poisoned");
    assert!(calls[0].0.contains("resolveReviewThread"));
    assert_eq!(
        calls[0].1,
        json!({
            "input": {
                "threadId": "thread-node",
            }
        })
    );
}

#[test]
fn unresolves_review_thread_variables() {
    let transport = RecordingTransport::default();
    *transport
        .graphql_response
        .lock()
        .expect("graphql response mutex should not be poisoned") = Some(json!({
        "data": {
            "unresolveReviewThread": {
                "thread": {
                    "id": "thread-node",
                    "isResolved": false
                }
            }
        }
    }));
    let client = GitHubClient::new(transport.clone());

    smol::block_on(client.unresolve_review_thread("thread-node")).unwrap();

    let calls = transport
        .graphql_calls
        .lock()
        .expect("graphql calls mutex should not be poisoned");
    assert!(calls[0].0.contains("unresolveReviewThread"));
    assert_eq!(
        calls[0].1,
        json!({
            "input": {
                "threadId": "thread-node",
            }
        })
    );
}

#[test]
fn updates_review_comment_variables() {
    let transport = RecordingTransport::default();
    *transport
        .graphql_response
        .lock()
        .expect("graphql response mutex should not be poisoned") = Some(json!({
        "data": {
            "updatePullRequestReviewComment": {
                "pullRequestReviewComment": {
                    "id": "comment-node",
                    "body": "Updated body."
                }
            }
        }
    }));
    let client = GitHubClient::new(transport.clone());

    smol::block_on(client.update_review_comment("comment-node", "Updated body.")).unwrap();

    let calls = transport
        .graphql_calls
        .lock()
        .expect("graphql calls mutex should not be poisoned");
    assert!(calls[0].0.contains("updatePullRequestReviewComment"));
    assert_eq!(
        calls[0].1,
        json!({
            "input": {
                "pullRequestReviewCommentId": "comment-node",
                "body": "Updated body.",
            }
        })
    );
}

#[test]
fn deletes_review_comment_variables() {
    let transport = RecordingTransport::default();
    *transport
        .graphql_response
        .lock()
        .expect("graphql response mutex should not be poisoned") = Some(json!({
        "data": {
            "deletePullRequestReviewComment": {
                "pullRequestReviewComment": {
                    "id": "comment-node"
                }
            }
        }
    }));
    let client = GitHubClient::new(transport.clone());

    smol::block_on(client.delete_review_comment("comment-node")).unwrap();

    let calls = transport
        .graphql_calls
        .lock()
        .expect("graphql calls mutex should not be poisoned");
    assert!(calls[0].0.contains("deletePullRequestReviewComment"));
    assert_eq!(
        calls[0].1,
        json!({
            "input": {
                "id": "comment-node",
            }
        })
    );
}

#[test]
fn adds_review_comment_reaction_variables() {
    let transport = RecordingTransport::default();
    *transport
        .graphql_response
        .lock()
        .expect("graphql response mutex should not be poisoned") = Some(json!({
        "data": {
            "addReaction": {
                "reaction": {
                    "id": "reaction-node"
                }
            }
        }
    }));
    let client = GitHubClient::new(transport.clone());

    smol::block_on(client.add_review_comment_reaction("comment-node", ReactionContent::Heart))
        .unwrap();

    let calls = transport
        .graphql_calls
        .lock()
        .expect("graphql calls mutex should not be poisoned");
    assert!(calls[0].0.contains("addReaction"));
    assert_eq!(
        calls[0].1,
        json!({
            "input": {
                "subjectId": "comment-node",
                "content": "HEART",
            }
        })
    );
}

#[test]
fn removes_review_comment_reaction_variables() {
    let transport = RecordingTransport::default();
    *transport
        .graphql_response
        .lock()
        .expect("graphql response mutex should not be poisoned") = Some(json!({
        "data": {
            "removeReaction": {
                "reaction": {
                    "id": "reaction-node"
                }
            }
        }
    }));
    let client = GitHubClient::new(transport.clone());

    smol::block_on(
        client.remove_review_comment_reaction("comment-node", ReactionContent::ThumbsUp),
    )
    .unwrap();

    let calls = transport
        .graphql_calls
        .lock()
        .expect("graphql calls mutex should not be poisoned");
    assert!(calls[0].0.contains("removeReaction"));
    assert_eq!(
        calls[0].1,
        json!({
            "input": {
                "subjectId": "comment-node",
                "content": "THUMBS_UP",
            }
        })
    );
}

#[test]
fn submits_pending_review_variables() {
    let transport = RecordingTransport::default();
    *transport
        .graphql_response
        .lock()
        .expect("graphql response mutex should not be poisoned") = Some(json!({
        "data": {
            "submitPullRequestReview": {
                "pullRequestReview": {
                    "id": "review-node",
                    "state": "APPROVED"
                }
            }
        }
    }));
    let client = GitHubClient::new(transport.clone());

    smol::block_on(client.submit_pull_request_review(
        "review-node",
        SubmitPullRequestReviewEvent::Approve,
        Some("Looks good."),
    ))
    .unwrap();

    let calls = transport
        .graphql_calls
        .lock()
        .expect("graphql calls mutex should not be poisoned");
    assert!(calls[0].0.contains("submitPullRequestReview"));
    assert_eq!(
        calls[0].1,
        json!({
            "input": {
                "pullRequestReviewId": "review-node",
                "event": "APPROVE",
                "body": "Looks good.",
            }
        })
    );
}
