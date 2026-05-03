use harbor_domain::{
    CheckConclusion, CheckRun, CheckStatus, ChecksSummary, DiffFile, FileStatus, MergeState,
    PullRequest, PullRequestState, RepoId, ReviewThread, ReviewThreadState,
};
use harbor_git::{ExternalApp, OpenTarget};

use crate::panels::{
    checks_summary_from_runs, merge_blocker, review_action_blocker, review_thread_counts,
};
use crate::workspace::{
    OpenTargetStatus, PullRequestInboxMode, github_file_url, next_switcher_index,
    normalized_search_query, open_target_for_app, open_with_app_disabled, parse_repo_id,
    pull_request_matches_query, repository_matches_query,
};

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
fn normalizes_switcher_search_queries() {
    assert_eq!(normalized_search_query("  Acme/App  "), "acme/app");
}

#[test]
fn matches_repositories_for_switcher_search() {
    let repository = RepoId::new("Acme", "Mobile-App");

    assert!(repository_matches_query(&repository, ""));
    assert!(repository_matches_query(&repository, "mobile"));
    assert!(repository_matches_query(&repository, "acme/mobile"));
    assert!(!repository_matches_query(&repository, "backend"));
}

#[test]
fn matches_pull_requests_for_switcher_search() {
    let pull_request = pull_request();

    assert!(pull_request_matches_query(&pull_request, ""));
    assert!(pull_request_matches_query(&pull_request, "feature"));
    assert!(pull_request_matches_query(&pull_request, "7"));
    assert!(pull_request_matches_query(&pull_request, "octo"));
    assert!(!pull_request_matches_query(&pull_request, "backend"));
}

#[test]
fn wraps_switcher_selection_indexes() {
    assert_eq!(next_switcher_index(0, 0, 1), 0);
    assert_eq!(next_switcher_index(0, 3, 1), 1);
    assert_eq!(next_switcher_index(2, 3, 1), 0);
    assert_eq!(next_switcher_index(0, 3, -1), 2);
    assert_eq!(next_switcher_index(10, 3, 1), 0);
}

#[test]
fn defaults_pull_request_inbox_to_open_mode() {
    assert_eq!(PullRequestInboxMode::default(), PullRequestInboxMode::Open);
    assert_eq!(PullRequestInboxMode::Open.label(), "Open");
    assert_eq!(PullRequestInboxMode::Closed.label(), "Closed");
    assert_eq!(PullRequestInboxMode::NeedsReview.label(), "Needs review");
    assert_eq!(
        PullRequestInboxMode::Closed.empty_message(),
        "No closed pull requests"
    );
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

#[test]
fn builds_active_file_github_url() {
    let file = diff_file("src/ui/app view.rs", FileStatus::Modified);

    assert_eq!(
        github_file_url(&pull_request(), &file).as_deref(),
        Some("https://github.com/acme/app/blob/abc123/src/ui/app%20view.rs")
    );
}

#[test]
fn falls_back_for_removed_github_files() {
    let file = diff_file("src/deleted.rs", FileStatus::Removed);

    assert_eq!(github_file_url(&pull_request(), &file), None);
}

#[test]
fn opens_worktree_root_for_removed_local_files() {
    let root = std::path::Path::new("/tmp/harbor-worktree");
    let file = diff_file("src/deleted.rs", FileStatus::Removed);

    let (target, status) = open_target_for_app(ExternalApp::Zed, root, Some(&file));

    assert_eq!(target, OpenTarget::Directory(root.to_path_buf()));
    assert_eq!(status, OpenTargetStatus::RemovedFile);
}

#[test]
fn disables_open_with_apps_without_local_path() {
    assert!(open_with_app_disabled(false, false, ExternalApp::Finder));
    assert!(open_with_app_disabled(true, true, ExternalApp::Finder));
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
        range: None,
        state,
        comments: Vec::new(),
    }
}

fn diff_file(path: &str, status: FileStatus) -> DiffFile {
    DiffFile {
        path: path.to_string(),
        previous_path: None,
        status,
        additions: 1,
        deletions: 0,
        changes: 1,
        patch: Some("@@ -1 +1 @@".to_string()),
    }
}
