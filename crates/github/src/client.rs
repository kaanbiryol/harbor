#[path = "client/pull_requests.rs"]
mod pull_requests;
#[path = "client/repositories.rs"]
mod repositories;
#[path = "client/requests.rs"]
mod requests;
#[path = "client/reviews.rs"]
mod reviews;
#[path = "client/workflows.rs"]
mod workflows;

#[cfg(test)]
use requests::{REPOSITORY_PAGE_SIZE, REVIEW_COMMENT_PAGE_SIZE};

#[derive(Clone, Debug)]
pub struct GitHubClient<T> {
    transport: T,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SubmitPullRequestReviewEvent {
    Approve,
    Comment,
    RequestChanges,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PullRequestListFilter {
    Open,
    Closed,
    NeedsReview,
}

impl<T> GitHubClient<T> {
    pub fn new(transport: T) -> Self {
        Self { transport }
    }

    pub fn transport(&self) -> &T {
        &self.transport
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use async_trait::async_trait;
    use harbor_domain::{ReactionContent, RepoId, ReviewCommentRange, ReviewSide};
    use serde_json::{Value, json};

    use super::*;
    use crate::{GitHubError, GitHubTransport, Result};

    type RecordedGet = (String, Vec<(String, String)>);

    #[derive(Clone, Default)]
    struct RecordingTransport {
        gets: Arc<Mutex<Vec<RecordedGet>>>,
        get_response: Arc<Mutex<Option<Value>>>,
        get_responses: Arc<Mutex<Vec<Value>>>,
        posts: Arc<Mutex<Vec<(String, Value)>>>,
        puts: Arc<Mutex<Vec<(String, Value)>>>,
        graphql_calls: Arc<Mutex<Vec<(String, Value)>>>,
        graphql_responses: Arc<Mutex<Vec<Value>>>,
        graphql_response: Arc<Mutex<Option<Value>>>,
        log: Arc<Mutex<Option<String>>>,
    }

    #[async_trait]
    impl GitHubTransport for RecordingTransport {
        async fn rest_get(&self, path: &str, query: &[(&str, &str)]) -> Result<Value> {
            self.gets
                .lock()
                .expect("gets mutex should not be poisoned")
                .push((
                    path.to_string(),
                    query
                        .iter()
                        .map(|(key, value)| (key.to_string(), value.to_string()))
                        .collect(),
                ));

            {
                let mut responses = self
                    .get_responses
                    .lock()
                    .expect("get responses mutex should not be poisoned");
                if !responses.is_empty() {
                    return Ok(responses.remove(0));
                }
            }

            self.get_response
                .lock()
                .expect("get response mutex should not be poisoned")
                .clone()
                .ok_or_else(|| GitHubError::Transport("missing GET response".to_string()))
        }

        async fn rest_post(&self, path: &str, body: Value) -> Result<Value> {
            self.posts
                .lock()
                .expect("posts mutex should not be poisoned")
                .push((path.to_string(), body));
            Ok(Value::Null)
        }

        async fn rest_put(&self, path: &str, body: Value) -> Result<Value> {
            self.puts
                .lock()
                .expect("puts mutex should not be poisoned")
                .push((path.to_string(), body));
            Ok(Value::Null)
        }

        async fn workflow_run_log(&self, owner: &str, repo: &str, run_id: u64) -> Result<String> {
            let log = format!("{owner}/{repo}#{run_id}");
            *self.log.lock().expect("log mutex should not be poisoned") = Some(log.clone());
            Ok(log)
        }

        async fn graphql(&self, query: &str, variables: Value) -> Result<Value> {
            self.graphql_calls
                .lock()
                .expect("graphql calls mutex should not be poisoned")
                .push((query.to_string(), variables));
            let mut responses = self
                .graphql_responses
                .lock()
                .expect("graphql responses mutex should not be poisoned");
            if !responses.is_empty() {
                return Ok(responses.remove(0));
            }

            self.graphql_response
                .lock()
                .expect("graphql response mutex should not be poisoned")
                .clone()
                .ok_or_else(|| GitHubError::Transport("missing GraphQL response".to_string()))
        }
    }

    fn review_thread_comment_json(id: &str, body: &str) -> Value {
        json!({
            "id": id,
            "body": body,
            "author": {
                "login": "reviewer",
                "avatarUrl": null
            },
            "createdAt": "2026-05-01T10:00:00Z",
            "updatedAt": null,
            "path": "src/app.rs",
            "line": 42,
            "originalLine": 42,
            "viewerDidAuthor": false,
            "viewerCanUpdate": false,
            "viewerCanDelete": false,
            "viewerCanReact": true,
            "reactionGroups": []
        })
    }

    #[test]
    fn posts_rerun_failed_jobs_endpoint() {
        let transport = RecordingTransport::default();
        let client = GitHubClient::new(transport.clone());

        smol::block_on(client.rerun_failed_jobs("acme", "app", 42)).unwrap();

        let posts = transport
            .posts
            .lock()
            .expect("posts mutex should not be poisoned");
        assert_eq!(posts.len(), 1);
        assert_eq!(
            posts[0].0,
            "/repos/acme/app/actions/runs/42/rerun-failed-jobs"
        );
        assert_eq!(posts[0].1, json!({}));
    }

    #[test]
    fn posts_workflow_dispatch_ref() {
        let transport = RecordingTransport::default();
        let client = GitHubClient::new(transport.clone());

        smol::block_on(client.dispatch_workflow("acme", "app", 9, "feature/build")).unwrap();

        let posts = transport
            .posts
            .lock()
            .expect("posts mutex should not be poisoned");
        assert_eq!(posts.len(), 1);
        assert_eq!(posts[0].0, "/repos/acme/app/actions/workflows/9/dispatches");
        assert_eq!(posts[0].1, json!({ "ref": "feature/build" }));
    }

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

        let count =
            smol::block_on(client.pull_request_review_comment_count("acme", "app", 7, "12345"))
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
    fn gets_user_repositories_endpoint() {
        let transport = RecordingTransport::default();
        *transport
            .get_response
            .lock()
            .expect("get response mutex should not be poisoned") = Some(json!([]));
        let client = GitHubClient::new(transport.clone());

        smol::block_on(client.list_repositories()).unwrap();

        let gets = transport
            .gets
            .lock()
            .expect("gets mutex should not be poisoned");
        assert_eq!(gets.len(), 1);
        assert_eq!(gets[0].0, "/user/repos");
        assert_eq!(
            gets[0].1,
            vec![
                (
                    "affiliation".to_string(),
                    "owner,collaborator,organization_member".to_string()
                ),
                ("per_page".to_string(), "100".to_string()),
                ("sort".to_string(), "updated".to_string()),
            ]
        );
    }

    #[test]
    fn gets_current_user_login() {
        let transport = RecordingTransport::default();
        *transport
            .get_response
            .lock()
            .expect("get response mutex should not be poisoned") =
            Some(json!({ "login": "octocat" }));
        let client = GitHubClient::new(transport.clone());

        let login = smol::block_on(client.current_user()).unwrap();

        assert_eq!(login, "octocat");
        let gets = transport
            .gets
            .lock()
            .expect("gets mutex should not be poisoned");
        assert_eq!(gets[0].0, "/user");
    }

    #[test]
    fn paginates_user_repositories_endpoint() {
        let transport = RecordingTransport::default();
        *transport
            .get_responses
            .lock()
            .expect("get responses mutex should not be poisoned") = vec![
            Value::Array(
                (0..REPOSITORY_PAGE_SIZE)
                    .map(|index| {
                        json!({
                            "name": format!("app-{index}"),
                            "owner": { "login": "acme" },
                        })
                    })
                    .collect(),
            ),
            json!([
                {
                    "name": "last",
                    "owner": { "login": "acme" },
                }
            ]),
        ];
        let client = GitHubClient::new(transport.clone());

        let repositories = smol::block_on(client.list_repositories()).unwrap();

        assert_eq!(repositories.len(), REPOSITORY_PAGE_SIZE + 1);
        assert_eq!(repositories[REPOSITORY_PAGE_SIZE].full_name(), "acme/last");

        let gets = transport
            .gets
            .lock()
            .expect("gets mutex should not be poisoned");
        assert_eq!(gets.len(), 2);
        assert_eq!(gets[0].0, "/user/repos");
        assert_eq!(
            gets[1].1,
            vec![
                (
                    "affiliation".to_string(),
                    "owner,collaborator,organization_member".to_string()
                ),
                ("per_page".to_string(), "100".to_string()),
                ("sort".to_string(), "updated".to_string()),
                ("page".to_string(), "2".to_string()),
            ]
        );
    }

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

            smol::block_on(
                client.list_repository_pull_requests(&RepoId::new("acme", "app"), filter),
            )
            .unwrap();

            let calls = transport
                .graphql_calls
                .lock()
                .expect("graphql calls mutex should not be poisoned");
            assert_eq!(calls.len(), 1);
            assert!(calls[0].0.contains("HarborRepositoryPullRequests"));
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

        smol::block_on(client.list_repository_pull_requests(
            &RepoId::new("acme", "app"),
            PullRequestListFilter::Open,
        ))
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
        assert!(calls[0].0.contains("reviewThreads"));
        assert_eq!(
            calls[0].1,
            json!({
                "owner": "acme",
                "repo": "app",
                "number": 7,
                "after": null,
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
        ];
        let client = GitHubClient::new(transport.clone());

        let threads = smol::block_on(client.list_review_threads("acme", "app", 7)).unwrap();

        assert_eq!(threads.len(), 1);
        assert_eq!(
            threads[0]
                .comments
                .iter()
                .map(|comment| comment.id.as_str())
                .collect::<Vec<_>>(),
            vec!["comment-1", "comment-2"]
        );
        let calls = transport
            .graphql_calls
            .lock()
            .expect("graphql calls mutex should not be poisoned");
        assert_eq!(calls.len(), 2);
        assert!(calls[1].0.contains("HarborPullRequestReviewThreadComments"));
        assert_eq!(
            calls[1].1,
            json!({
                "threadId": "thread-1",
                "after": "comment-cursor-1",
            })
        );
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

    #[test]
    fn delegates_workflow_run_log() {
        let transport = RecordingTransport::default();
        let client = GitHubClient::new(transport.clone());

        let log = smol::block_on(client.workflow_run_log("acme", "app", 42)).unwrap();

        assert_eq!(log, "acme/app#42");
        assert_eq!(
            transport
                .log
                .lock()
                .expect("log mutex should not be poisoned")
                .as_deref(),
            Some("acme/app#42")
        );
    }
}
