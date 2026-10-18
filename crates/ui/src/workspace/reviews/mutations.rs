mod comments;
mod reactions;
mod submissions;
mod threads;

use harbor_domain::ReviewCommentRange;

use crate::workspace::{
    AppView, PendingReviewSession, PullRequestDetailCacheKey,
    reviews::OptimisticReviewCommentHandle,
};

impl AppView {
    fn remove_optimistic_review_comment_for_detail(
        &mut self,
        detail_key: &PullRequestDetailCacheKey,
        comment_id: &str,
    ) {
        if self.selected_pull_request_detail_key().as_ref() == Some(detail_key) {
            self.review_state.remove_review_comment(comment_id);
            self.sync_unresolved_thread_count();
        }

        self.detail_state
            .remove_optimistic_comment_from_snapshot(detail_key, comment_id);
    }

    fn rollback_pending_review_comment_count_for_detail(
        &mut self,
        detail_key: &PullRequestDetailCacheKey,
        previous_pending_review: Option<&PendingReviewSession>,
    ) {
        if self.selected_pull_request_detail_key().as_ref() == Some(detail_key) {
            self.review_state
                .rollback_pending_review_comment_count(previous_pending_review);
        }

        self.detail_state
            .rollback_pending_review_comment_count_in_snapshot(detail_key, previous_pending_review);
    }

    fn set_pending_review_for_detail(
        &mut self,
        detail_key: &PullRequestDetailCacheKey,
        pending_review: PendingReviewSession,
    ) {
        if self.selected_pull_request_detail_key().as_ref() == Some(detail_key) {
            self.review_state.set_pending_review(pending_review.clone());
        }

        self.detail_state
            .set_pending_review_in_snapshot(detail_key, pending_review);
    }

    fn insert_optimistic_review_thread(
        &mut self,
        range: ReviewCommentRange,
        body: String,
    ) -> OptimisticReviewCommentHandle {
        let handle = self
            .review_state
            .insert_optimistic_review_thread(range, body);
        self.sync_unresolved_thread_count();
        handle
    }
}
