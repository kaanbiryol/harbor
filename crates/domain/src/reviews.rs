use std::collections::HashMap;

use crate::{
    PullRequestReview, PullRequestReviewState, ReviewComment, ReviewCommentPosition,
    ReviewCommentRange, ReviewSide, ReviewThread, ReviewThreadState,
};

#[path = "reviews/reactions.rs"]
mod reactions;

pub use reactions::{
    ReviewReactionKey, apply_review_reaction_overrides, review_reaction,
    set_review_comment_reaction_state,
};

pub const LOCAL_REVIEW_THREAD_ID_PREFIX: &str = "local-review-thread-";
pub const LOCAL_REVIEW_COMMENT_ID_PREFIX: &str = "local-review-comment-";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PendingReviewSession {
    pub node_id: String,
    pub comment_count: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OptimisticReviewCommentHandle {
    pub comment_id: String,
}

pub fn pending_review_from_reviews(
    reviews: &[PullRequestReview],
    current_user_login: Option<&str>,
    existing_pending_review: Option<&PendingReviewSession>,
    pending_review_comment_count: Option<usize>,
) -> Option<PendingReviewSession> {
    reviews
        .iter()
        .find(|review| {
            review.state == PullRequestReviewState::Pending
                && current_user_login.is_none_or(|login| review.author == login)
                && review
                    .node_id
                    .as_ref()
                    .is_some_and(|node_id| !node_id.is_empty())
        })
        .and_then(|review| {
            let node_id = review.node_id.clone()?;
            let comment_count = pending_review_comment_count.unwrap_or_else(|| {
                existing_pending_review
                    .filter(|pending_review| pending_review.node_id == node_id)
                    .map_or(0, |pending_review| pending_review.comment_count)
            });

            Some(PendingReviewSession {
                node_id,
                comment_count,
            })
        })
        .or_else(|| existing_pending_review.cloned())
}

pub fn apply_review_thread_state_overrides(
    review_threads: &mut [ReviewThread],
    overrides: &HashMap<String, ReviewThreadState>,
) -> Vec<String> {
    if overrides.is_empty() {
        return Vec::new();
    }

    let mut settled_overrides = Vec::new();

    for thread in review_threads {
        let Some(overridden_state) = overrides.get(&thread.id).copied() else {
            continue;
        };

        if thread.state == overridden_state {
            settled_overrides.push(thread.id.clone());
        } else {
            thread.state = overridden_state;
        }
    }

    settled_overrides
}

pub fn merge_optimistic_review_threads(
    mut loaded_threads: Vec<ReviewThread>,
    existing_threads: &[ReviewThread],
) -> Vec<ReviewThread> {
    for existing_thread in existing_threads {
        if is_local_review_thread(existing_thread) {
            if !loaded_threads.iter().any(|loaded_thread| {
                review_thread_contains_optimistic_comment(loaded_thread, existing_thread)
            }) {
                loaded_threads.push(existing_thread.clone());
            }
            continue;
        }

        let optimistic_comments = existing_thread
            .comments
            .iter()
            .filter(|comment| is_local_review_comment(comment))
            .cloned()
            .collect::<Vec<_>>();
        if optimistic_comments.is_empty() {
            continue;
        }

        let Some(loaded_thread) = loaded_threads
            .iter_mut()
            .find(|loaded_thread| loaded_thread.id == existing_thread.id)
        else {
            continue;
        };

        for optimistic_comment in optimistic_comments {
            if !loaded_thread.comments.iter().any(|loaded_comment| {
                review_comment_matches_optimistic(loaded_comment, &optimistic_comment)
            }) {
                loaded_thread.comments.push(optimistic_comment);
            }
        }
    }

    loaded_threads
}

pub fn remove_review_comment_from_threads(threads: &mut Vec<ReviewThread>, comment_id: &str) {
    for thread in threads.iter_mut() {
        thread.comments.retain(|comment| comment.id != comment_id);
    }

    threads.retain(|thread| !thread.comments.is_empty());
}

pub fn unresolved_review_thread_count(threads: &[ReviewThread]) -> usize {
    threads
        .iter()
        .filter(|thread| thread.state == ReviewThreadState::Unresolved)
        .count()
}

pub fn increment_pending_review_comment_count(pending_review: &mut Option<PendingReviewSession>) {
    if let Some(pending_review) = pending_review.as_mut() {
        pending_review.comment_count = pending_review.comment_count.saturating_add(1);
    }
}

pub fn rollback_pending_review_comment_count(
    pending_review: &mut Option<PendingReviewSession>,
    previous_pending_review: Option<&PendingReviewSession>,
) {
    let (Some(pending_review), Some(previous_pending_review)) =
        (pending_review.as_mut(), previous_pending_review)
    else {
        return;
    };

    if pending_review.node_id == previous_pending_review.node_id
        && pending_review.comment_count > previous_pending_review.comment_count
    {
        pending_review.comment_count = pending_review.comment_count.saturating_sub(1);
    }
}

fn review_thread_contains_optimistic_comment(
    loaded_thread: &ReviewThread,
    optimistic_thread: &ReviewThread,
) -> bool {
    optimistic_thread
        .comments
        .iter()
        .filter(|comment| is_local_review_comment(comment))
        .all(|optimistic_comment| {
            loaded_thread.comments.iter().any(|loaded_comment| {
                review_comment_matches_optimistic(loaded_comment, optimistic_comment)
            })
        })
}

fn review_comment_matches_optimistic(
    loaded_comment: &ReviewComment,
    optimistic_comment: &ReviewComment,
) -> bool {
    loaded_comment.body == optimistic_comment.body
        && review_comment_positions_match(
            loaded_comment.position.as_ref(),
            optimistic_comment.position.as_ref(),
        )
}

fn review_comment_positions_match(
    left: Option<&ReviewCommentPosition>,
    right: Option<&ReviewCommentPosition>,
) -> bool {
    match (left, right) {
        (Some(left), Some(right)) => {
            left.path == right.path
                && left.side == right.side
                && review_comment_position_line(left) == review_comment_position_line(right)
        }
        (None, None) => true,
        _ => false,
    }
}

fn review_comment_position_line(position: &ReviewCommentPosition) -> Option<u32> {
    match position.side {
        ReviewSide::Left => position.original_line.or(position.line),
        ReviewSide::Right => position.line.or(position.original_line),
    }
}

fn is_local_review_thread(thread: &ReviewThread) -> bool {
    is_local_review_thread_id(&thread.id)
}

pub fn is_local_review_thread_id(thread_id: &str) -> bool {
    thread_id.starts_with(LOCAL_REVIEW_THREAD_ID_PREFIX)
}

fn is_local_review_comment(comment: &ReviewComment) -> bool {
    is_local_review_comment_id(&comment.id)
}

pub fn is_local_review_comment_id(comment_id: &str) -> bool {
    comment_id.starts_with(LOCAL_REVIEW_COMMENT_ID_PREFIX)
}

pub fn review_position_from_range(range: &ReviewCommentRange) -> ReviewCommentPosition {
    let (line, original_line) = match range.side {
        ReviewSide::Left => (None, Some(range.line)),
        ReviewSide::Right => (Some(range.line), None),
    };

    ReviewCommentPosition {
        path: range.path.clone(),
        line,
        original_line,
        side: range.side,
    }
}

pub fn review_comment_pending_sync(comment: &ReviewComment) -> bool {
    is_local_review_comment(comment)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ReactionContent, ReviewReaction};

    #[test]
    fn pending_review_uses_loaded_comment_count() {
        let reviews = vec![pending_review("review-rest", "review-node", "alex")];

        let pending_review = pending_review_from_reviews(&reviews, Some("alex"), None, Some(3))
            .expect("pending review should be detected");

        assert_eq!(pending_review.node_id, "review-node");
        assert_eq!(pending_review.comment_count, 3);
    }

    #[test]
    fn pending_review_keeps_existing_count_without_loaded_count() {
        let reviews = vec![pending_review("review-rest", "review-node", "alex")];
        let existing = PendingReviewSession {
            node_id: "review-node".to_string(),
            comment_count: 2,
        };

        let pending_review =
            pending_review_from_reviews(&reviews, Some("alex"), Some(&existing), None)
                .expect("pending review should be detected");

        assert_eq!(pending_review.comment_count, 2);
    }

    #[test]
    fn rollback_pending_review_count_only_removes_optimistic_increment() {
        let previous = PendingReviewSession {
            node_id: "review-node".to_string(),
            comment_count: 2,
        };
        let mut pending_review = Some(PendingReviewSession {
            node_id: "review-node".to_string(),
            comment_count: 3,
        });

        rollback_pending_review_comment_count(&mut pending_review, Some(&previous));

        assert_eq!(pending_review.expect("pending review").comment_count, 2);

        let mut refreshed_pending_review = Some(PendingReviewSession {
            node_id: "review-node".to_string(),
            comment_count: 2,
        });

        rollback_pending_review_comment_count(&mut refreshed_pending_review, Some(&previous));

        assert_eq!(
            refreshed_pending_review
                .expect("pending review should remain")
                .comment_count,
            2
        );
    }

    #[test]
    fn merge_preserves_optimistic_review_thread_until_refresh_loads_it() {
        let range = review_range("src/lib.rs", ReviewSide::Right, 14);
        let optimistic_thread = review_thread(
            "local-review-thread-1",
            "local-review-comment-1",
            range.clone(),
            "looks good",
        );

        let merged = merge_optimistic_review_threads(Vec::new(), &[optimistic_thread]);

        assert_eq!(merged.len(), 1);
        assert_eq!(merged[0].range, Some(range));
        assert_eq!(merged[0].comments[0].body, "looks good");
    }

    #[test]
    fn merge_drops_optimistic_review_thread_when_refresh_loads_matching_comment() {
        let range = review_range("src/lib.rs", ReviewSide::Right, 14);
        let loaded_thread = review_thread("thread-1", "comment-1", range.clone(), "looks good");
        let optimistic_thread = review_thread(
            "local-review-thread-1",
            "local-review-comment-1",
            range,
            "looks good",
        );

        let merged = merge_optimistic_review_threads(vec![loaded_thread], &[optimistic_thread]);

        assert_eq!(merged.len(), 1);
        assert_eq!(merged[0].id, "thread-1");
        assert_eq!(merged[0].comments.len(), 1);
    }

    #[test]
    fn merge_preserves_optimistic_reply_until_refresh_loads_it() {
        let range = review_range("src/lib.rs", ReviewSide::Right, 14);
        let loaded_thread = review_thread("thread-1", "comment-1", range.clone(), "first");
        let mut existing_thread = loaded_thread.clone();
        existing_thread
            .comments
            .push(review_comment("local-review-comment-2", range, "reply"));

        let merged = merge_optimistic_review_threads(vec![loaded_thread], &[existing_thread]);

        assert_eq!(merged.len(), 1);
        assert_eq!(
            merged[0]
                .comments
                .iter()
                .map(|comment| comment.body.as_str())
                .collect::<Vec<_>>(),
            vec!["first", "reply"]
        );
    }

    #[test]
    fn merge_drops_optimistic_reply_when_refresh_loads_matching_comment() {
        let range = review_range("src/lib.rs", ReviewSide::Right, 14);
        let mut loaded_thread = review_thread("thread-1", "comment-1", range.clone(), "first");
        loaded_thread
            .comments
            .push(review_comment("comment-2", range.clone(), "reply"));
        let mut existing_thread = review_thread("thread-1", "comment-1", range.clone(), "first");
        existing_thread
            .comments
            .push(review_comment("local-review-comment-2", range, "reply"));

        let merged = merge_optimistic_review_threads(vec![loaded_thread], &[existing_thread]);

        assert_eq!(merged.len(), 1);
        assert_eq!(merged[0].comments.len(), 2);
        assert!(
            merged[0]
                .comments
                .iter()
                .all(|comment| !is_local_review_comment(comment))
        );
    }

    #[test]
    fn thread_state_override_keeps_resolved_thread_resolved_in_stale_review_data() {
        let range = review_range("src/lib.rs", ReviewSide::Right, 14);
        let loaded_thread = review_thread("thread-1", "comment-1", range, "first");
        let mut loaded_threads = vec![loaded_thread];
        let overrides = HashMap::from([("thread-1".to_string(), ReviewThreadState::Resolved)]);

        let settled_overrides =
            apply_review_thread_state_overrides(&mut loaded_threads, &overrides);

        assert!(settled_overrides.is_empty());
        assert_eq!(loaded_threads[0].state, ReviewThreadState::Resolved);
    }

    #[test]
    fn thread_state_override_keeps_reopened_thread_unresolved_in_stale_review_data() {
        let range = review_range("src/lib.rs", ReviewSide::Right, 14);
        let mut loaded_thread = review_thread("thread-1", "comment-1", range, "first");
        loaded_thread.state = ReviewThreadState::Resolved;
        let mut loaded_threads = vec![loaded_thread];
        let overrides = HashMap::from([("thread-1".to_string(), ReviewThreadState::Unresolved)]);

        let settled_overrides =
            apply_review_thread_state_overrides(&mut loaded_threads, &overrides);

        assert!(settled_overrides.is_empty());
        assert_eq!(loaded_threads[0].state, ReviewThreadState::Unresolved);
    }

    #[test]
    fn thread_state_override_settles_when_loaded_review_data_matches() {
        let range = review_range("src/lib.rs", ReviewSide::Right, 14);
        let mut loaded_thread = review_thread("thread-1", "comment-1", range, "first");
        loaded_thread.state = ReviewThreadState::Resolved;
        let mut loaded_threads = vec![loaded_thread];
        let overrides = HashMap::from([("thread-1".to_string(), ReviewThreadState::Resolved)]);

        let settled_overrides =
            apply_review_thread_state_overrides(&mut loaded_threads, &overrides);

        assert_eq!(settled_overrides, vec!["thread-1".to_string()]);
        assert_eq!(loaded_threads[0].state, ReviewThreadState::Resolved);
    }

    #[test]
    fn reaction_override_keeps_removed_reaction_out_of_stale_review_data() {
        let range = review_range("src/lib.rs", ReviewSide::Right, 14);
        let mut loaded_thread = review_thread("thread-1", "comment-1", range, "first");
        loaded_thread.comments[0].reactions = vec![ReviewReaction {
            content: ReactionContent::Heart,
            count: 1,
            viewer_has_reacted: true,
        }];
        let mut loaded_threads = vec![loaded_thread];
        let key = ReviewReactionKey::new("comment-1", ReactionContent::Heart);
        let overrides = HashMap::from([(key, false)]);

        let settled_overrides = apply_review_reaction_overrides(&mut loaded_threads, &overrides);

        assert!(settled_overrides.is_empty());
        assert!(review_reaction(&loaded_threads[0].comments[0], ReactionContent::Heart).is_none());
    }

    #[test]
    fn reaction_override_adds_reaction_to_stale_review_data() {
        let range = review_range("src/lib.rs", ReviewSide::Right, 14);
        let loaded_thread = review_thread("thread-1", "comment-1", range, "first");
        let mut loaded_threads = vec![loaded_thread];
        let key = ReviewReactionKey::new("comment-1", ReactionContent::Rocket);
        let overrides = HashMap::from([(key, true)]);

        let settled_overrides = apply_review_reaction_overrides(&mut loaded_threads, &overrides);
        let reaction = review_reaction(&loaded_threads[0].comments[0], ReactionContent::Rocket)
            .expect("optimistic reaction should be preserved");

        assert!(settled_overrides.is_empty());
        assert_eq!(reaction.count, 1);
        assert!(reaction.viewer_has_reacted);
    }

    #[test]
    fn reaction_override_settles_when_loaded_review_data_matches() {
        let range = review_range("src/lib.rs", ReviewSide::Right, 14);
        let loaded_thread = review_thread("thread-1", "comment-1", range, "first");
        let mut loaded_threads = vec![loaded_thread];
        let key = ReviewReactionKey::new("comment-1", ReactionContent::Heart);
        let overrides = HashMap::from([(key.clone(), false)]);

        let settled_overrides = apply_review_reaction_overrides(&mut loaded_threads, &overrides);

        assert_eq!(settled_overrides, vec![key]);
        assert!(review_reaction(&loaded_threads[0].comments[0], ReactionContent::Heart).is_none());
    }

    fn pending_review(id: &str, node_id: &str, author: &str) -> PullRequestReview {
        PullRequestReview {
            id: id.to_string(),
            node_id: Some(node_id.to_string()),
            author: author.to_string(),
            state: PullRequestReviewState::Pending,
            body: None,
            submitted_at: None,
        }
    }

    fn review_thread(
        thread_id: &str,
        comment_id: &str,
        range: ReviewCommentRange,
        body: &str,
    ) -> ReviewThread {
        ReviewThread {
            id: thread_id.to_string(),
            path: range.path.clone(),
            range: Some(range.clone()),
            state: ReviewThreadState::Unresolved,
            comments: vec![review_comment(comment_id, range, body)],
        }
    }

    fn review_comment(comment_id: &str, range: ReviewCommentRange, body: &str) -> ReviewComment {
        ReviewComment {
            id: comment_id.to_string(),
            author: "alex".to_string(),
            author_avatar_url: None,
            body: body.to_string(),
            created_at: chrono::DateTime::parse_from_rfc3339("2026-05-04T20:30:00Z")
                .expect("valid timestamp")
                .with_timezone(&chrono::Utc),
            updated_at: None,
            position: Some(review_position_from_range(&range)),
            viewer_did_author: true,
            viewer_can_update: false,
            viewer_can_delete: false,
            viewer_can_react: false,
            reactions: Vec::new(),
        }
    }

    fn review_range(path: &str, side: ReviewSide, line: u32) -> ReviewCommentRange {
        ReviewCommentRange {
            path: path.to_string(),
            line,
            side,
            start_line: None,
            start_side: None,
        }
    }
}
