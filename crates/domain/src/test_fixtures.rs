use chrono::{DateTime, Utc};

use crate::{
    ChecksSummary, MergeState, PullRequest, PullRequestComment, PullRequestState, RepoId,
    ReviewComment, ReviewCommentPosition, ReviewSide, ReviewThread, ReviewThreadState,
};

pub(crate) fn test_time() -> DateTime<Utc> {
    DateTime::parse_from_rfc3339("2026-05-01T10:00:00Z")
        .expect("valid test timestamp")
        .to_utc()
}

pub(crate) fn pull_request() -> PullRequest {
    PullRequest {
        repo: RepoId::new("acme", "app"),
        node_id: "pr-node".to_string(),
        number: 7,
        title: "Add feature".to_string(),
        body: None,
        author: "octocat".to_string(),
        author_avatar_url: None,
        url: "https://github.com/acme/app/pull/7".to_string(),
        state: PullRequestState::Open,
        is_draft: false,
        head_ref: "feature".to_string(),
        base_ref: "main".to_string(),
        head_sha: "abc123".to_string(),
        review_decision: None,
        merge_state: Some(MergeState::Clean),
        merge_capabilities: Default::default(),
        labels: Vec::new(),
        assignees: Vec::new(),
        requested_reviewers: Vec::new(),
        requested_teams: Vec::new(),
        checks_summary: ChecksSummary {
            total: 1,
            passed: 1,
            failed: 0,
            pending: 0,
            skipped: 0,
        },
        unresolved_threads: 0,
        created_at: Some(test_time()),
        updated_at: Some(test_time()),
    }
}

pub(crate) fn review_thread(state: ReviewThreadState) -> ReviewThread {
    ReviewThread {
        comments_complete: true,
        id: "thread-1".to_string(),
        path: "src/lib.rs".to_string(),
        range: None,
        state,
        comments: vec![ReviewComment {
            id: "comment-1".to_string(),
            url: Some("https://github.com/octo/harbor/pull/7#discussion_r1".to_string()),
            pull_request_review_id: None,
            pull_request_review_node_id: None,
            author: "maria".to_string(),
            author_avatar_url: None,
            body: "Please check this line.".to_string(),
            created_at: test_time(),
            updated_at: None,
            position: Some(ReviewCommentPosition {
                path: "src/lib.rs".to_string(),
                line: Some(12),
                original_line: Some(11),
                side: ReviewSide::Right,
            }),
            viewer_did_author: true,
            viewer_can_update: true,
            viewer_can_delete: false,
            viewer_can_react: true,
            reactions: Vec::new(),
        }],
    }
}

pub(crate) fn pull_request_comment(id: &str, body: &str) -> PullRequestComment {
    PullRequestComment {
        id: id.to_string(),
        author: "alex".to_string(),
        author_avatar_url: None,
        body: body.to_string(),
        created_at: test_time(),
        updated_at: None,
    }
}
