#[path = "dto_checks.rs"]
mod checks;
#[path = "dto_pull_requests.rs"]
mod pull_requests;
#[path = "dto_repositories.rs"]
mod repositories;
#[path = "dto_reviews.rs"]
mod reviews;
#[path = "dto_workflows.rs"]
mod workflows;

pub use checks::*;
pub use pull_requests::*;
pub use repositories::*;
pub use reviews::*;
pub use workflows::*;

#[cfg(test)]
mod tests {
    use harbor_domain::{
        CheckConclusion, CheckStatus, FileStatus, MergeState, PullRequestReviewState,
        PullRequestState, ReactionContent, RepoId, ReviewDecision, ReviewSide, ReviewThreadState,
        WorkflowConclusion, WorkflowStatus,
    };
    use serde_json::json;

    use super::*;

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
                "mergeable_state": "clean"
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
        assert_eq!(page.pull_requests[1].state, PullRequestState::Closed);
        assert_eq!(page.pull_requests[2].state, PullRequestState::Merged);
        assert_eq!(
            page.pull_requests[2].review_decision,
            Some(ReviewDecision::Approved)
        );
    }

    #[test]
    fn maps_repository_list() {
        let value = json!([
            {
                "name": "app",
                "owner": { "login": "acme" }
            },
            {
                "name": "tools",
                "owner": { "login": "octo" }
            }
        ]);

        let repositories = repositories_from_value(value).unwrap();

        assert_eq!(repositories.len(), 2);
        assert_eq!(repositories[0].full_name(), "acme/app");
        assert_eq!(repositories[1].full_name(), "octo/tools");
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
    }

    #[test]
    fn maps_check_runs() {
        let value = json!({
            "total_count": 2,
            "check_runs": [
                {
                    "id": 1001,
                    "name": "build",
                    "status": "completed",
                    "conclusion": "success",
                    "details_url": "https://ci.example/build",
                    "html_url": "https://github.com/acme/app/runs/1001",
                    "started_at": "2026-05-01T10:00:00Z",
                    "completed_at": "2026-05-01T10:05:00Z"
                },
                {
                    "id": 1002,
                    "name": "test",
                    "status": "in_progress",
                    "conclusion": null,
                    "details_url": null,
                    "html_url": "https://github.com/acme/app/runs/1002",
                    "started_at": null,
                    "completed_at": null
                }
            ]
        });

        let check_runs = check_runs_from_value(value).unwrap();

        assert_eq!(check_runs.len(), 2);
        assert_eq!(check_runs[0].status, CheckStatus::Completed);
        assert_eq!(check_runs[0].conclusion, Some(CheckConclusion::Success));
        assert_eq!(check_runs[1].status, CheckStatus::InProgress);
        assert_eq!(check_runs[1].conclusion, None);
    }

    #[test]
    fn maps_workflow_runs() {
        let value = json!({
            "total_count": 1,
            "workflow_runs": [
                {
                    "id": 2001,
                    "workflow_id": 901,
                    "name": "CI",
                    "display_title": "run tests",
                    "status": "completed",
                    "conclusion": "failure",
                    "head_branch": "feature/test",
                    "head_sha": "abc123",
                    "event": "pull_request",
                    "url": "https://api.github.com/repos/acme/app/actions/runs/2001",
                    "html_url": "https://github.com/acme/app/actions/runs/2001",
                    "created_at": "2026-05-01T10:00:00Z",
                    "updated_at": "2026-05-01T10:05:00Z"
                }
            ]
        });

        let workflow_runs = workflow_runs_from_value(value).unwrap();

        assert_eq!(workflow_runs.len(), 1);
        assert_eq!(workflow_runs[0].workflow_id, Some(901));
        assert_eq!(workflow_runs[0].name, "run tests");
        assert_eq!(workflow_runs[0].workflow_name.as_deref(), Some("CI"));
        assert_eq!(workflow_runs[0].status, WorkflowStatus::Completed);
        assert_eq!(
            workflow_runs[0].conclusion,
            Some(WorkflowConclusion::Failure)
        );
    }

    #[test]
    fn maps_workflow_jobs() {
        let value = json!({
            "total_count": 1,
            "jobs": [
                {
                    "id": 3001,
                    "name": "test",
                    "status": "completed",
                    "conclusion": "failure",
                    "steps": [
                        {
                            "name": "install",
                            "number": 1,
                            "status": "completed",
                            "conclusion": "success",
                            "started_at": "2026-05-01T10:00:00Z",
                            "completed_at": "2026-05-01T10:01:00Z"
                        },
                        {
                            "name": "unit tests",
                            "number": 2,
                            "status": "completed",
                            "conclusion": "failure",
                            "started_at": null,
                            "completed_at": null
                        }
                    ]
                }
            ]
        });

        let jobs = workflow_jobs_from_value(value).unwrap();

        assert_eq!(jobs.len(), 1);
        assert_eq!(jobs[0].id, 3001);
        assert_eq!(jobs[0].status, WorkflowStatus::Completed);
        assert_eq!(jobs[0].conclusion, Some(WorkflowConclusion::Failure));
        assert_eq!(jobs[0].steps.len(), 2);
        assert_eq!(jobs[0].steps[1].name, "unit tests");
        assert_eq!(
            jobs[0].steps[1].conclusion,
            Some(WorkflowConclusion::Failure)
        );
    }

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
}
