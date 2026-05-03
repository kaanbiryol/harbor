use harbor_domain::{
    ChecksSummary, DiffFile, FileStatus, Label, MergeState, PullRequest, PullRequestReview,
    PullRequestReviewState, PullRequestState, RepoId, ReviewComment, ReviewCommentPosition,
    ReviewDecision, ReviewSide, ReviewThread, ReviewThreadState,
};

pub(crate) fn fake_pull_requests() -> Vec<PullRequest> {
    let repo = RepoId::new("sixt", "mobile-app");

    vec![
        PullRequest {
            repo: repo.clone(),
            number: 1842,
            title: "speed up pull request inbox refresh".to_string(),
            body: Some("Cache first, refresh in the background.".to_string()),
            author: "alex".to_string(),
            url: "https://github.com/sixt/mobile-app/pull/1842".to_string(),
            state: PullRequestState::Open,
            is_draft: false,
            head_ref: "feature/pr-cache".to_string(),
            base_ref: "main".to_string(),
            head_sha: "a1b2c3d".to_string(),
            review_decision: Some(ReviewDecision::ReviewRequired),
            merge_state: Some(MergeState::Clean),
            labels: vec![Label {
                name: "performance".to_string(),
                color: Some("34d399".to_string()),
            }],
            checks_summary: ChecksSummary {
                total: 18,
                passed: 16,
                failed: 0,
                pending: 2,
                skipped: 0,
            },
            unresolved_threads: 3,
        },
        PullRequest {
            repo: repo.clone(),
            number: 1837,
            title: "render failed action steps inline".to_string(),
            body: None,
            author: "maria".to_string(),
            url: "https://github.com/sixt/mobile-app/pull/1837".to_string(),
            state: PullRequestState::Open,
            is_draft: false,
            head_ref: "ci/failed-step-focus".to_string(),
            base_ref: "main".to_string(),
            head_sha: "d4e5f6a".to_string(),
            review_decision: Some(ReviewDecision::ChangesRequested),
            merge_state: Some(MergeState::Blocked),
            labels: vec![Label {
                name: "ci".to_string(),
                color: Some("fbbf24".to_string()),
            }],
            checks_summary: ChecksSummary {
                total: 21,
                passed: 18,
                failed: 2,
                pending: 1,
                skipped: 0,
            },
            unresolved_threads: 7,
        },
        PullRequest {
            repo,
            number: 1829,
            title: "add review thread domain model".to_string(),
            body: None,
            author: "kaan".to_string(),
            url: "https://github.com/sixt/mobile-app/pull/1829".to_string(),
            state: PullRequestState::Open,
            is_draft: true,
            head_ref: "review/thread-model".to_string(),
            base_ref: "main".to_string(),
            head_sha: "f7a8b9c".to_string(),
            review_decision: None,
            merge_state: Some(MergeState::Unknown),
            labels: vec![Label {
                name: "review".to_string(),
                color: Some("93c5fd".to_string()),
            }],
            checks_summary: ChecksSummary {
                total: 17,
                passed: 17,
                failed: 0,
                pending: 0,
                skipped: 0,
            },
            unresolved_threads: 0,
        },
    ]
}

pub(crate) fn configured_repo_from_env() -> Option<RepoId> {
    std::env::var("HARBOR_REPO")
        .ok()
        .or_else(|| std::env::var("GH_REPO").ok())
        .and_then(|value| parse_repo_id(&value))
}

pub(crate) fn parse_repo_id(value: &str) -> Option<RepoId> {
    let (owner, name) = value.split_once('/')?;

    if owner.is_empty() || name.is_empty() || name.contains('/') {
        None
    } else {
        Some(RepoId::new(owner, name))
    }
}

pub(crate) fn fake_files() -> Vec<DiffFile> {
    vec![
        DiffFile {
            path: "crates/ui/src/inbox.rs".to_string(),
            previous_path: None,
            status: FileStatus::Modified,
            additions: 42,
            deletions: 11,
            changes: 53,
            patch: Some(
                "@@ -14,6 +14,13 @@\n pub struct InboxState {\n+    selected: usize,\n+    visible_rows: Range<usize>,\n }\n+\n+impl InboxState {\n+    pub fn move_selection(&mut self, delta: i32) { /* fake diff */ }\n+}\n"
                    .to_string(),
            ),
        },
        DiffFile {
            path: "crates/github/src/transport.rs".to_string(),
            previous_path: None,
            status: FileStatus::Added,
            additions: 88,
            deletions: 0,
            changes: 88,
            patch: None,
        },
        DiffFile {
            path: "crates/logs/src/parser.rs".to_string(),
            previous_path: None,
            status: FileStatus::Modified,
            additions: 65,
            deletions: 22,
            changes: 87,
            patch: None,
        },
    ]
}

pub(crate) fn fake_pull_request_reviews() -> Vec<PullRequestReview> {
    vec![
        PullRequestReview {
            id: "review-1".to_string(),
            author: "maria".to_string(),
            state: PullRequestReviewState::ChangesRequested,
            body: Some("A couple of small issues before this is ready.".to_string()),
            submitted_at: Some(fake_time("2026-05-01T10:00:00Z")),
        },
        PullRequestReview {
            id: "review-2".to_string(),
            author: "alex".to_string(),
            state: PullRequestReviewState::Commented,
            body: Some("The scroll path feels much better now.".to_string()),
            submitted_at: Some(fake_time("2026-05-01T11:30:00Z")),
        },
    ]
}

pub(crate) fn fake_review_threads() -> Vec<ReviewThread> {
    vec![
        ReviewThread {
            id: "thread-1".to_string(),
            path: "crates/ui/src/inbox.rs".to_string(),
            state: ReviewThreadState::Unresolved,
            comments: vec![
                ReviewComment {
                    id: "comment-1".to_string(),
                    author: "maria".to_string(),
                    body: "This row update still looks broader than it needs to be.".to_string(),
                    created_at: fake_time("2026-05-01T10:00:00Z"),
                    updated_at: None,
                    position: Some(ReviewCommentPosition {
                        path: "crates/ui/src/inbox.rs".to_string(),
                        line: Some(42),
                        original_line: Some(39),
                        side: ReviewSide::Right,
                    }),
                },
                ReviewComment {
                    id: "comment-2".to_string(),
                    author: "kaan".to_string(),
                    body: "I will narrow the state update to the selected row.".to_string(),
                    created_at: fake_time("2026-05-01T10:20:00Z"),
                    updated_at: None,
                    position: Some(ReviewCommentPosition {
                        path: "crates/ui/src/inbox.rs".to_string(),
                        line: Some(42),
                        original_line: Some(39),
                        side: ReviewSide::Right,
                    }),
                },
            ],
        },
        ReviewThread {
            id: "thread-2".to_string(),
            path: "crates/logs/src/parser.rs".to_string(),
            state: ReviewThreadState::Resolved,
            comments: vec![ReviewComment {
                id: "comment-3".to_string(),
                author: "alex".to_string(),
                body: "Resolved after the parser stopped cloning full log lines.".to_string(),
                created_at: fake_time("2026-05-01T11:00:00Z"),
                updated_at: None,
                position: Some(ReviewCommentPosition {
                    path: "crates/logs/src/parser.rs".to_string(),
                    line: Some(17),
                    original_line: Some(17),
                    side: ReviewSide::Right,
                }),
            }],
        },
    ]
}

pub(crate) fn fake_time(value: &str) -> chrono::DateTime<chrono::Utc> {
    chrono::DateTime::parse_from_rfc3339(value)
        .map(|datetime| datetime.with_timezone(&chrono::Utc))
        .unwrap_or_else(|_| chrono::Utc::now())
}
