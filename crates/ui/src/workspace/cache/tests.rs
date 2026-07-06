use std::collections::HashSet;

use harbor_domain::ReviewThreadState;

use super::*;
use crate::{
    test_fixtures::{pull_request, review_thread},
    workspace::state::{PullRequestDetailUiState, WorkflowLogState},
};

fn detail_snapshot() -> PullRequestDetailSnapshot {
    let mut pull_request = pull_request();
    pull_request.unresolved_threads = 1;

    PullRequestDetailSnapshot {
        pull_request,
        files: Vec::new(),
        diffs: Vec::new(),
        check_runs: Vec::new(),
        workflow_runs: Vec::new(),
        workflow_jobs: Vec::new(),
        pull_request_reviews: Vec::new(),
        pull_request_comments: Vec::new(),
        review_threads: vec![review_thread(ReviewThreadState::Unresolved)],
        detail_loaded: PullRequestDetailLoadedState::default(),
        pending_review: Some(PendingReviewSession {
            node_id: "pending-review".to_string(),
            comment_count: 2,
        }),
        log_chunk: None,
        current_user_login: None,
        collapsed_file_tree_folders: HashSet::new(),
        expanded_diff_file_paths: HashSet::new(),
        collapsed_diff_file_paths: HashSet::new(),
        reviewed_file_paths: HashSet::new(),
        excluded_file_type_filters: HashSet::new(),
        show_files_owned_by_current_user: false,
        owned_file_paths: HashSet::new(),
        active_file: 0,
        active_hunk: 0,
        active_tab: PanelTab::Diff,
    }
}

#[test]
fn detail_cache_helpers_update_review_snapshot_consistently() {
    let mut detail_state =
        PullRequestDetailUiState::new(Vec::new(), Vec::new(), WorkflowLogState::new());
    let snapshot = detail_snapshot();
    let key = PullRequestDetailCacheKey::new(
        snapshot.pull_request.repo.clone(),
        snapshot.pull_request.number,
        snapshot.pull_request.head_sha.clone(),
    );
    let previous_pending_review = snapshot.pending_review.clone();

    detail_state.cache_snapshot(key.clone(), snapshot);
    detail_state.remove_optimistic_comment_from_snapshot(&key, "comment-1");
    let snapshot = detail_state.snapshot(&key).expect("snapshot should exist");
    assert!(snapshot.review_threads.is_empty());
    assert_eq!(snapshot.pull_request.unresolved_threads, 0);

    detail_state
        .rollback_pending_review_comment_count_in_snapshot(&key, previous_pending_review.as_ref());
    let snapshot = detail_state.snapshot(&key).expect("snapshot should exist");
    assert_eq!(
        snapshot
            .pending_review
            .as_ref()
            .map(|pending_review| pending_review.comment_count),
        Some(2)
    );

    detail_state.set_pending_review_in_snapshot(
        &key,
        PendingReviewSession {
            node_id: "new-pending-review".to_string(),
            comment_count: 1,
        },
    );
    let snapshot = detail_state.snapshot(&key).expect("snapshot should exist");
    assert_eq!(
        snapshot
            .pending_review
            .as_ref()
            .map(|pending_review| pending_review.node_id.as_str()),
        Some("new-pending-review")
    );
}
