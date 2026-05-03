use harbor_domain::{
    CheckConclusion, CheckRun, CheckStatus, ChecksSummary, MergeState, PullRequest,
    PullRequestState, RepoId, ReviewThread, ReviewThreadState,
};

use crate::panels::{
    checks_summary_from_runs, merge_blocker, review_action_blocker, review_thread_counts,
};
use crate::workspace::parse_repo_id;

#[test]
fn parses_owner_and_repo() {
    let repo = parse_repo_id("acme/app").unwrap();

    assert_eq!(repo.owner, "acme");
    assert_eq!(repo.name, "app");
}

#[test]
fn rejects_invalid_repo_values() {
    assert!(parse_repo_id("").is_none());
    assert!(parse_repo_id("acme").is_none());
    assert!(parse_repo_id("/app").is_none());
    assert!(parse_repo_id("acme/").is_none());
    assert!(parse_repo_id("acme/app/extra").is_none());
}

#[test]
fn summarizes_check_runs() {
    let check_runs = vec![
        check_run(CheckStatus::Completed, Some(CheckConclusion::Success)),
        check_run(CheckStatus::Completed, Some(CheckConclusion::Failure)),
        check_run(CheckStatus::Completed, Some(CheckConclusion::Skipped)),
        check_run(CheckStatus::InProgress, None),
    ];

    let summary = checks_summary_from_runs(&check_runs);

    assert_eq!(summary.total, 4);
    assert_eq!(summary.passed, 1);
    assert_eq!(summary.failed, 1);
    assert_eq!(summary.skipped, 1);
    assert_eq!(summary.pending, 1);
}

#[test]
fn allows_review_actions_for_open_pull_requests() {
    assert_eq!(review_action_blocker(&pull_request()), None);
}

#[test]
fn blocks_merge_until_pull_request_is_ready() {
    let mut pr = pull_request();
    pr.checks_summary.pending = 1;

    assert_eq!(
        merge_blocker(&pr).as_deref(),
        Some("PR #7 still has pending checks")
    );

    pr.checks_summary.pending = 0;
    pr.unresolved_threads = 2;

    assert_eq!(
        merge_blocker(&pr).as_deref(),
        Some("PR #7 still has 2 unresolved review threads")
    );
}

#[test]
fn allows_clean_pull_request_merge() {
    assert_eq!(merge_blocker(&pull_request()), None);
}

#[test]
fn counts_review_threads_by_state() {
    let threads = vec![
        review_thread(ReviewThreadState::Unresolved),
        review_thread(ReviewThreadState::Resolved),
        review_thread(ReviewThreadState::Outdated),
        review_thread(ReviewThreadState::Unresolved),
    ];

    assert_eq!(review_thread_counts(&threads), (2, 1, 1));
}

fn check_run(status: CheckStatus, conclusion: Option<CheckConclusion>) -> CheckRun {
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

fn pull_request() -> PullRequest {
    PullRequest {
        repo: RepoId::new("acme", "app"),
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
        checks_summary: ChecksSummary {
            total: 1,
            passed: 1,
            failed: 0,
            pending: 0,
            skipped: 0,
        },
        unresolved_threads: 0,
    }
}

fn review_thread(state: ReviewThreadState) -> ReviewThread {
    ReviewThread {
        id: "thread".to_string(),
        path: "src/app.rs".to_string(),
        state,
        comments: Vec::new(),
    }
}
