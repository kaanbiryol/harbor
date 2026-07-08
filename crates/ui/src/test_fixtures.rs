use chrono::{DateTime, Utc};
use harbor_domain::{
    CheckConclusion, CheckRun, CheckStatus, ChecksSummary, DiffFile, FileStatus, FileViewedState,
    MergeState, PullRequest, PullRequestState, ReactionContent, RepoId, ReviewComment,
    ReviewCommentPosition, ReviewReaction, ReviewSide, ReviewThread, ReviewThreadState,
    WorkflowConclusion, WorkflowRun, WorkflowStatus,
};

pub(crate) fn pull_request() -> PullRequest {
    PullRequest {
        repo: RepoId::new("acme", "app"),
        node_id: "pr-node".to_string(),
        number: 7,
        title: "Add feature".to_string(),
        body: None,
        author: "octocat".to_string(),
        url: "https://github.com/acme/app/pull/7".to_string(),
        state: PullRequestState::Open,
        is_draft: false,
        head_ref: "feature".to_string(),
        base_ref: "main".to_string(),
        head_sha: "abc123".to_string(),
        review_decision: None,
        merge_state: Some(MergeState::Clean),
        labels: Vec::new(),
        assignees: Vec::new(),
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

pub(crate) fn diff_file(path: &str, status: FileStatus) -> DiffFile {
    DiffFile {
        path: path.to_string(),
        previous_path: None,
        status,
        additions: 1,
        deletions: 0,
        changes: 1,
        patch: Some("@@ -1 +1 @@".to_string()),
        viewed_state: FileViewedState::Unviewed,
    }
}

pub(crate) fn patched_diff_file() -> DiffFile {
    let mut file = diff_file("src/lib.rs", FileStatus::Modified);
    file.patch = Some("@@ -1 +1 @@\n-old\n+new\n".to_string());
    file
}

pub(crate) fn check_run(status: CheckStatus, conclusion: Option<CheckConclusion>) -> CheckRun {
    CheckRun {
        id: None,
        name: "check".to_string(),
        status,
        conclusion,
        details_url: None,
        html_url: None,
        started_at: None,
        completed_at: None,
    }
}

pub(crate) fn workflow_run() -> WorkflowRun {
    WorkflowRun {
        id: 42,
        workflow_id: Some(9),
        name: "build".to_string(),
        workflow_name: Some("CI".to_string()),
        status: WorkflowStatus::Completed,
        conclusion: Some(WorkflowConclusion::Failure),
        head_branch: "feature".to_string(),
        head_sha: "abc123".to_string(),
        event: "pull_request".to_string(),
        url: "https://api.github.com/repos/acme/app/actions/runs/42".to_string(),
        html_url: "https://github.com/acme/app/actions/runs/42".to_string(),
        created_at: test_time(),
        updated_at: test_time(),
    }
}

pub(crate) fn review_comment() -> ReviewComment {
    ReviewComment {
        id: "comment".to_string(),
        pull_request_review_id: None,
        pull_request_review_node_id: None,
        author: "octocat".to_string(),
        author_avatar_url: None,
        body: "Looks good".to_string(),
        created_at: test_time(),
        updated_at: None,
        position: None,
        viewer_did_author: false,
        viewer_can_update: false,
        viewer_can_delete: false,
        viewer_can_react: true,
        reactions: Vec::new(),
    }
}

pub(crate) fn positioned_review_comment() -> ReviewComment {
    ReviewComment {
        id: "comment-1".to_string(),
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
    }
}

pub(crate) fn review_thread(state: ReviewThreadState) -> ReviewThread {
    ReviewThread {
        id: "thread-1".to_string(),
        path: "src/lib.rs".to_string(),
        range: None,
        state,
        comments: vec![positioned_review_comment()],
    }
}

pub(crate) fn review_reaction(
    content: ReactionContent,
    viewer_has_reacted: bool,
) -> ReviewReaction {
    ReviewReaction {
        content,
        count: 1,
        viewer_has_reacted,
    }
}

pub(crate) fn test_time() -> DateTime<Utc> {
    DateTime::parse_from_rfc3339("2026-05-01T10:00:00Z")
        .expect("valid test timestamp")
        .with_timezone(&Utc)
}
