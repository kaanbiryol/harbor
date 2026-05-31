use chrono::Utc;
use harbor_domain::{
    ReviewComment, ReviewCommentPosition, ReviewCommentRange, ReviewThread, ReviewThreadState,
};

use crate::workspace::{
    ReviewRuntimeState,
    reviews::{
        LOCAL_REVIEW_COMMENT_ID_PREFIX, LOCAL_REVIEW_THREAD_ID_PREFIX,
        OptimisticReviewCommentHandle, review_position_from_range,
    },
};

impl ReviewRuntimeState {
    pub(crate) fn insert_optimistic_review_thread(
        &mut self,
        range: ReviewCommentRange,
        body: String,
    ) -> OptimisticReviewCommentHandle {
        let sequence = self.next_local_review_comment_sequence();
        let comment_id = format!("{LOCAL_REVIEW_COMMENT_ID_PREFIX}{sequence}");
        let comment = self.optimistic_review_comment(
            comment_id.clone(),
            Some(review_position_from_range(&range)),
            body,
        );

        self.review_threads.push(ReviewThread {
            id: format!("{LOCAL_REVIEW_THREAD_ID_PREFIX}{sequence}"),
            path: range.path.clone(),
            range: Some(range),
            state: ReviewThreadState::Unresolved,
            comments: vec![comment],
        });

        OptimisticReviewCommentHandle { comment_id }
    }

    pub(crate) fn append_optimistic_review_reply(
        &mut self,
        thread_id: &str,
        body: String,
    ) -> Option<OptimisticReviewCommentHandle> {
        let thread_index = self
            .review_threads
            .iter()
            .position(|thread| thread.id == thread_id)?;

        let position = self.review_threads[thread_index]
            .range
            .as_ref()
            .map(review_position_from_range)
            .or_else(|| {
                self.review_threads[thread_index]
                    .comments
                    .iter()
                    .find_map(|comment| comment.position.clone())
            });
        let sequence = self.next_local_review_comment_sequence();
        let comment_id = format!("{LOCAL_REVIEW_COMMENT_ID_PREFIX}{sequence}");
        let comment = self.optimistic_review_comment(comment_id.clone(), position, body);

        self.review_threads[thread_index].comments.push(comment);

        Some(OptimisticReviewCommentHandle { comment_id })
    }

    fn optimistic_review_comment(
        &self,
        id: String,
        position: Option<ReviewCommentPosition>,
        body: String,
    ) -> ReviewComment {
        ReviewComment {
            id,
            author: self
                .current_user_login
                .clone()
                .unwrap_or_else(|| "you".to_string()),
            author_avatar_url: None,
            body,
            created_at: Utc::now(),
            updated_at: None,
            position,
            viewer_did_author: true,
            viewer_can_update: false,
            viewer_can_delete: false,
            viewer_can_react: false,
            reactions: Vec::new(),
        }
    }

    fn next_local_review_comment_sequence(&mut self) -> u64 {
        self.local_review_comment_sequence = self.local_review_comment_sequence.saturating_add(1);
        self.local_review_comment_sequence
    }
}
