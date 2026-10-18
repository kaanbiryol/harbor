use chrono::Duration;

use super::*;
use crate::ReviewThreadState;
use crate::test_fixtures::{review_thread as test_review_thread, test_time};

#[test]
fn orders_timeline_activity_and_hides_pending_reviews() {
    let time = test_time();
    let commits = vec![PullRequestCommit {
        sha: "abc123".to_string(),
        message: "Initial commit".to_string(),
        author: "octocat".to_string(),
        author_avatar_url: None,
        authored_at: Some(time),
    }];
    let comments = vec![PullRequestComment {
        id: "comment".to_string(),
        author: "octocat".to_string(),
        author_avatar_url: None,
        body: "comment".to_string(),
        created_at: time + Duration::minutes(2),
        updated_at: None,
    }];
    let reviews = vec![
        PullRequestReview {
            id: "submitted".to_string(),
            node_id: None,
            author: "reviewer".to_string(),
            author_avatar_url: None,
            state: PullRequestReviewState::Approved,
            body: None,
            submitted_at: Some(time + Duration::minutes(1)),
        },
        PullRequestReview {
            id: "pending".to_string(),
            node_id: None,
            author: "reviewer".to_string(),
            author_avatar_url: None,
            state: PullRequestReviewState::Pending,
            body: None,
            submitted_at: Some(time),
        },
        PullRequestReview {
            id: "empty-commented".to_string(),
            node_id: None,
            author: "reviewer".to_string(),
            author_avatar_url: None,
            state: PullRequestReviewState::Commented,
            body: None,
            submitted_at: Some(time),
        },
    ];
    let mut thread = review_thread_with_state(ReviewThreadState::Unresolved);
    thread.comments[0].created_at = time + Duration::minutes(3);
    let threads = vec![thread];

    let items = overview_timeline_items(&commits, &reviews, &comments, &threads);

    assert_eq!(items.len(), 4);
    assert!(matches!(items[0], OverviewTimelineItem::Commit(_)));
    assert!(matches!(items[1], OverviewTimelineItem::Review(_)));
    assert!(matches!(items[2], OverviewTimelineItem::Comment(_)));
    assert!(matches!(items[3], OverviewTimelineItem::Thread(_)));
}

#[test]
fn keeps_only_meaningful_review_events() {
    let review = |state, body: Option<&str>| PullRequestReview {
        id: "review".to_string(),
        node_id: None,
        author: "reviewer".to_string(),
        author_avatar_url: None,
        state,
        body: body.map(str::to_string),
        submitted_at: Some(test_time()),
    };

    assert!(!overview_review_visible(&review(
        PullRequestReviewState::Pending,
        None
    )));
    assert!(!overview_review_visible(&review(
        PullRequestReviewState::Commented,
        None
    )));
    assert!(!overview_review_visible(&review(
        PullRequestReviewState::Commented,
        Some("  \n")
    )));
    assert!(overview_review_visible(&review(
        PullRequestReviewState::Commented,
        Some("Review summary")
    )));
    assert!(overview_review_visible(&review(
        PullRequestReviewState::Approved,
        None
    )));
    assert!(overview_review_visible(&review(
        PullRequestReviewState::ChangesRequested,
        None
    )));
}

fn review_thread_with_state(state: ReviewThreadState) -> ReviewThread {
    test_review_thread(state)
}
