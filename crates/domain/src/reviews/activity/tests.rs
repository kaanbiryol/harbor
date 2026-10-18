use chrono::Duration;

use super::*;
use crate::test_fixtures::{pull_request_comment, review_thread as test_review_thread, test_time};

#[test]
fn counts_review_threads_by_state() {
    let threads = vec![
        review_thread_with_state(ReviewThreadState::Unresolved),
        review_thread_with_state(ReviewThreadState::Resolved),
        review_thread_with_state(ReviewThreadState::Outdated),
        review_thread_with_state(ReviewThreadState::Unresolved),
    ];

    assert_eq!(review_thread_counts(&threads), (2, 1, 1));
}

#[test]
fn panel_prioritizes_unresolved_threads_grouped_by_file_and_line() {
    let mut later_thread = review_thread();
    later_thread.id = "thread-later".to_string();
    set_thread_location(&mut later_thread, "src/app.rs", 30);
    let mut earlier_thread = review_thread();
    earlier_thread.id = "thread-earlier".to_string();
    set_thread_location(&mut earlier_thread, "src/app.rs", 10);
    let mut other_file_thread = review_thread();
    other_file_thread.id = "thread-other-file".to_string();
    set_thread_location(&mut other_file_thread, "src/z.rs", 5);
    let mut resolved_thread = review_thread_with_state(ReviewThreadState::Resolved);
    resolved_thread.id = "thread-resolved".to_string();
    set_thread_location(&mut resolved_thread, "src/app.rs", 2);
    let pull_request_comment = pull_request_comment("comment-1", "Can we do this?");

    let items = review_panel_items(
        &[],
        &[
            later_thread,
            resolved_thread,
            other_file_thread,
            earlier_thread,
        ],
        &[pull_request_comment],
    );

    assert_eq!(
        items,
        vec![
            ReviewPanelItem::Section {
                section: ReviewPanelSection::NeedsAttention,
                item_count: 3,
            },
            ReviewPanelItem::FileHeader {
                path: "src/app.rs".to_string(),
                thread_count: 2,
            },
            ReviewPanelItem::Thread {
                thread_id: "thread-earlier".to_string(),
            },
            ReviewPanelItem::Thread {
                thread_id: "thread-later".to_string(),
            },
            ReviewPanelItem::FileHeader {
                path: "src/z.rs".to_string(),
                thread_count: 1,
            },
            ReviewPanelItem::Thread {
                thread_id: "thread-other-file".to_string(),
            },
            ReviewPanelItem::Section {
                section: ReviewPanelSection::Conversation,
                item_count: 1,
            },
            ReviewPanelItem::Comment {
                comment_id: "comment-1".to_string(),
            },
            ReviewPanelItem::Section {
                section: ReviewPanelSection::Resolved,
                item_count: 1,
            },
            ReviewPanelItem::FileHeader {
                path: "src/app.rs".to_string(),
                thread_count: 1,
            },
            ReviewPanelItem::Thread {
                thread_id: "thread-resolved".to_string(),
            },
        ]
    );
}

#[test]
fn conversation_includes_review_summaries() {
    let mut review = pull_request_review("401", None, Some("Overall direction looks right."));
    review.submitted_at = Some(test_time());

    let items = review_conversation_items(&[review], &[], &[]);

    assert_eq!(items.len(), 1);
    assert!(matches!(
        &items[0].kind,
        ReviewConversationItemKind::Review { review_id } if review_id == "401"
    ));
}

#[test]
fn conversation_includes_review_state_without_inline_comments() {
    let mut review = pull_request_review("401", Some("review-node-401"), None);
    review.state = PullRequestReviewState::Approved;
    review.submitted_at = Some(test_time());

    let items = review_conversation_items(&[review], &[], &[]);

    assert_eq!(items.len(), 1);
    assert!(matches!(
        &items[0].kind,
        ReviewConversationItemKind::Review { review_id } if review_id == "401"
    ));
}

#[test]
fn conversation_skips_empty_review_when_inline_thread_represents_it() {
    let mut review = pull_request_review("401", Some("review-node-401"), None);
    review.state = PullRequestReviewState::ChangesRequested;
    review.submitted_at = Some(test_time());
    let mut thread = review_thread();
    thread.comments[0].pull_request_review_id = Some("401".to_string());
    thread.comments[0].pull_request_review_node_id = Some("review-node-401".to_string());

    let items = review_conversation_items(&[review], &[thread], &[]);

    assert!(items.is_empty());
}

#[test]
fn conversation_includes_pull_request_comments() {
    let comment = pull_request_comment("comment-1", "Can we do this?");

    let items = review_conversation_items(&[], &[], &[comment]);

    assert_eq!(items.len(), 1);
    assert!(matches!(
        &items[0].kind,
        ReviewConversationItemKind::Comment { comment_id } if comment_id == "comment-1"
    ));
}

#[test]
fn conversation_orders_older_summaries_before_recent_comments() {
    let mut review = pull_request_review("401", None, Some("Older summary."));
    review.submitted_at = Some(test_time());
    let mut comment = pull_request_comment("comment-1", "Newer comment.");
    comment.created_at = test_time() + Duration::minutes(5);

    let items = review_conversation_items(&[review], &[], &[comment]);

    assert_eq!(items.len(), 2);
    assert!(matches!(
        &items[0].kind,
        ReviewConversationItemKind::Review { review_id } if review_id == "401"
    ));
}

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

fn review_thread() -> ReviewThread {
    review_thread_with_state(ReviewThreadState::Unresolved)
}

fn review_thread_with_state(state: ReviewThreadState) -> ReviewThread {
    test_review_thread(state)
}

fn set_thread_location(thread: &mut ReviewThread, path: &str, line: u32) {
    thread.path = path.to_string();
    let position = thread
        .comments
        .first_mut()
        .and_then(|comment| comment.position.as_mut())
        .expect("test thread should have a positioned comment");
    position.path = path.to_string();
    position.line = Some(line);
    position.original_line = None;
}

fn pull_request_review(id: &str, node_id: Option<&str>, body: Option<&str>) -> PullRequestReview {
    PullRequestReview {
        id: id.to_string(),
        node_id: node_id.map(str::to_string),
        author: "alex".to_string(),
        author_avatar_url: None,
        state: PullRequestReviewState::Commented,
        body: body.map(str::to_string),
        submitted_at: None,
    }
}
