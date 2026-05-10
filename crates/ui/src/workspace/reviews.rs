use harbor_domain::{ReactionContent, ReviewCommentRange, ReviewSide};

pub(crate) use harbor_domain::reviews::{
    LOCAL_REVIEW_COMMENT_ID_PREFIX, LOCAL_REVIEW_THREAD_ID_PREFIX, OptimisticReviewCommentHandle,
    PendingReviewSession, ReviewReactionKey, apply_review_reaction_overrides,
    apply_review_thread_state_overrides, increment_pending_review_comment_count,
    is_local_review_thread_id, merge_optimistic_review_threads, pending_review_from_reviews,
    remove_review_comment_from_threads, review_comment_pending_sync, review_position_from_range,
    review_reaction, rollback_pending_review_comment_count, set_review_comment_reaction_state,
    unresolved_review_thread_count,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ReviewLineTarget {
    pub(crate) hunk_index: usize,
    pub(crate) line_index: usize,
    pub(crate) range: ReviewCommentRange,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ReviewComposer {
    pub(crate) anchor: ReviewLineTarget,
    pub(crate) range: ReviewCommentRange,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ReviewLineSelection {
    pub(crate) anchor: ReviewLineTarget,
    pub(crate) current: ReviewLineTarget,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ReviewCommentSubmission {
    SingleComment,
    StartReview,
    AddToReview,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ReviewThreadUiError {
    pub(crate) thread_id: String,
    pub(crate) message: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ReviewCommentUiError {
    pub(crate) comment_id: String,
    pub(crate) message: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ReviewReactionAction {
    pub(crate) comment_id: String,
    pub(crate) content: ReactionContent,
}

pub(super) fn review_composer_from_selection(
    anchor: &ReviewLineTarget,
    current: &ReviewLineTarget,
) -> std::result::Result<ReviewComposer, String> {
    let range = review_range_from_targets(anchor, current)?;
    let anchor = if anchor.line_index >= current.line_index {
        anchor.clone()
    } else {
        current.clone()
    };

    Ok(ReviewComposer { anchor, range })
}

pub(crate) fn review_range_from_targets(
    anchor: &ReviewLineTarget,
    current: &ReviewLineTarget,
) -> std::result::Result<ReviewCommentRange, String> {
    if anchor.hunk_index != current.hunk_index {
        return Err("Review comments can only span lines in one diff hunk".to_string());
    }

    if anchor.range.path != current.range.path {
        return Err("Review comments can only span one file".to_string());
    }

    if anchor.range.side != current.range.side {
        return Err("Review comments can only span one diff side".to_string());
    }

    let (start, end) = if anchor.line_index <= current.line_index {
        (anchor, current)
    } else {
        (current, anchor)
    };
    let mut range = end.range.clone();

    if start.line_index != end.line_index {
        range.start_line = Some(start.range.line);
        range.start_side = Some(start.range.side);
    } else {
        range.start_line = None;
        range.start_side = None;
    }

    Ok(range)
}

pub(super) fn review_comment_range_label(range: &ReviewCommentRange) -> String {
    let side = match range.side {
        ReviewSide::Left => "left",
        ReviewSide::Right => "right",
    };

    if let Some(start_line) = range.start_line {
        format!("{side} lines {start_line}-{}", range.line)
    } else {
        format!("{side} line {}", range.line)
    }
}
