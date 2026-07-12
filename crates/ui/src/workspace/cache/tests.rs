use std::{collections::HashSet, sync::Arc};

use harbor_domain::ReviewThreadState;

use super::*;
use crate::{
    test_fixtures::{pull_request, review_thread},
    workspace::state::{PullRequestDetailUiState, PullRequestInboxState, WorkflowLogState},
};

fn detail_snapshot() -> PullRequestDetailSnapshot {
    let mut pull_request = pull_request();
    pull_request.unresolved_threads = 1;

    PullRequestDetailSnapshot {
        pull_request,
        files: Vec::new(),
        diffs: Vec::new(),
        check_runs: Vec::new(),
        commits: Vec::new(),
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
        changed_files_state: ChangedFilesUiState::default(),
        expanded_check_groups: HashSet::new(),
        active_file: 0,
        active_hunk: 0,
        active_tab: PanelTab::Diff,
    }
}

#[test]
fn detail_cache_evicts_the_oldest_snapshot() {
    let mut detail_state =
        PullRequestDetailUiState::new(Vec::new(), Vec::new(), WorkflowLogState::new());
    let repository = RepoId::new("acme", "app");

    for number in 1..=9 {
        let mut snapshot = detail_snapshot();
        snapshot.pull_request.number = number;
        snapshot.pull_request.head_sha = format!("head-{number}");
        detail_state.cache_snapshot(
            PullRequestDetailCacheKey::new(
                repository.clone(),
                number,
                snapshot.pull_request.head_sha.clone(),
            ),
            Arc::new(snapshot),
        );
    }

    assert!(
        detail_state
            .snapshot(&PullRequestDetailCacheKey::new(
                repository.clone(),
                1,
                "head-1".to_string(),
            ))
            .is_none()
    );
    assert!(
        detail_state
            .snapshot(&PullRequestDetailCacheKey::new(
                repository,
                9,
                "head-9".to_string(),
            ))
            .is_some()
    );
}

#[test]
fn inbox_cache_evicts_the_oldest_snapshot() {
    let mut inbox_state = PullRequestInboxState::default();

    for index in 1..=9 {
        inbox_state.insert_snapshot(
            PullRequestInboxCacheKey::new(
                RepoId::new("acme", format!("app-{index}")),
                PullRequestInboxMode::Open,
            ),
            PullRequestInboxSnapshot {
                pull_requests: Vec::new(),
                page_info: PullRequestInboxPageInfo::default(),
                detail: None,
                selected_pr: 0,
            },
        );
    }

    assert!(
        inbox_state
            .snapshot(&PullRequestInboxCacheKey::new(
                RepoId::new("acme", "app-1"),
                PullRequestInboxMode::Open,
            ))
            .is_none()
    );
    assert!(
        inbox_state
            .snapshot(&PullRequestInboxCacheKey::new(
                RepoId::new("acme", "app-9"),
                PullRequestInboxMode::Open,
            ))
            .is_some()
    );
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

    detail_state.cache_snapshot(key.clone(), Arc::new(snapshot));
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
