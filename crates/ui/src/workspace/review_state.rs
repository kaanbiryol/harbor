use harbor_domain::{
    PullRequestReview, ReactionContent, ReviewComment, ReviewCommentRange, ReviewThread,
    ReviewThreadState,
};

use crate::workspace::{
    AppView, PendingReviewSession, PullRequestDetailCacheKey,
    reviews::{
        OptimisticReviewCommentHandle, remove_review_comment_from_threads,
        rollback_pending_review_comment_count, unresolved_review_thread_count,
    },
};

impl AppView {
    pub(crate) fn clear_review_composer_state(&mut self) {
        self.review_state.clear_composer_and_action_state();
    }

    pub(crate) fn apply_loaded_review_data(
        &mut self,
        reviews: Vec<PullRequestReview>,
        review_threads: Vec<ReviewThread>,
        current_user_login: Option<String>,
        pending_review_comment_count: Option<usize>,
    ) -> usize {
        let unresolved_count = self.review_state.apply_loaded_review_data(
            reviews,
            review_threads,
            current_user_login,
            pending_review_comment_count,
        );

        self.set_selected_unresolved_thread_count(unresolved_count);
        unresolved_count
    }

    pub(crate) fn replace_loaded_review_threads(&mut self, review_threads: Vec<ReviewThread>) {
        let unresolved_count = self
            .review_state
            .replace_loaded_review_threads(review_threads);
        self.set_selected_unresolved_thread_count(unresolved_count);
    }

    pub(crate) fn replace_reviews_and_loaded_threads(
        &mut self,
        reviews: Vec<PullRequestReview>,
        review_threads: Vec<ReviewThread>,
    ) {
        let unresolved_count = self
            .review_state
            .replace_reviews_and_loaded_threads(reviews, review_threads);
        self.set_selected_unresolved_thread_count(unresolved_count);
    }

    pub(super) fn sync_unresolved_thread_count(&mut self) -> usize {
        let unresolved_count = self.review_state.unresolved_thread_count();
        self.set_selected_unresolved_thread_count(unresolved_count);
        unresolved_count
    }

    fn set_selected_unresolved_thread_count(&mut self, unresolved_count: usize) {
        if let Some(selected) = self.pull_requests.get_mut(self.selected_pr) {
            selected.unresolved_threads = unresolved_count;
        }
    }

    pub(super) fn set_review_thread_state(&mut self, thread_id: &str, state: ReviewThreadState) {
        self.review_state.set_review_thread_state(thread_id, state);
    }

    pub(super) fn next_review_data_generation(&mut self) -> u64 {
        self.review_state.review_data_generation =
            self.review_state.review_data_generation.saturating_add(1);
        self.review_state.review_data_generation
    }

    pub(super) fn review_data_generation(&self) -> u64 {
        self.review_state.review_data_generation
    }

    pub(crate) fn review_comment(&self, comment_id: &str) -> Option<&ReviewComment> {
        self.review_state.review_comment(comment_id)
    }

    pub(super) fn review_comment_mut(&mut self, comment_id: &str) -> Option<&mut ReviewComment> {
        self.review_state.review_comment_mut(comment_id)
    }

    pub(super) fn remove_review_comment(&mut self, comment_id: &str) {
        self.review_state.remove_review_comment(comment_id);
    }

    pub(super) fn remove_optimistic_review_comment_for_detail(
        &mut self,
        detail_key: &PullRequestDetailCacheKey,
        comment_id: &str,
    ) {
        if self.selected_pull_request_detail_key().as_ref() == Some(detail_key) {
            self.remove_review_comment(comment_id);
            self.sync_unresolved_thread_count();
        }

        if let Some(snapshot) = self
            .detail_state
            .pull_request_detail_cache
            .get_mut(detail_key)
        {
            remove_review_comment_from_threads(&mut snapshot.review_threads, comment_id);
            snapshot.pull_request.unresolved_threads =
                unresolved_review_thread_count(&snapshot.review_threads);
        }
    }

    pub(super) fn rollback_pending_review_comment_count_for_detail(
        &mut self,
        detail_key: &PullRequestDetailCacheKey,
        previous_pending_review: Option<&PendingReviewSession>,
    ) {
        if self.selected_pull_request_detail_key().as_ref() == Some(detail_key) {
            self.review_state
                .rollback_pending_review_comment_count(previous_pending_review);
        }

        if let Some(snapshot) = self
            .detail_state
            .pull_request_detail_cache
            .get_mut(detail_key)
        {
            rollback_pending_review_comment_count(
                &mut snapshot.pending_review,
                previous_pending_review,
            );
        }
    }

    pub(super) fn set_pending_review_for_detail(
        &mut self,
        detail_key: &PullRequestDetailCacheKey,
        pending_review: PendingReviewSession,
    ) {
        if self.selected_pull_request_detail_key().as_ref() == Some(detail_key) {
            self.review_state.set_pending_review(pending_review.clone());
        }

        if let Some(snapshot) = self
            .detail_state
            .pull_request_detail_cache
            .get_mut(detail_key)
        {
            snapshot.pending_review = Some(pending_review);
        }
    }

    pub(super) fn set_review_comment_reaction(
        &mut self,
        comment_id: &str,
        content: ReactionContent,
        viewer_has_reacted: bool,
    ) {
        self.review_state
            .set_review_comment_reaction(comment_id, content, viewer_has_reacted);
    }

    pub(super) fn insert_optimistic_review_thread(
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

    pub(super) fn append_optimistic_review_reply(
        &mut self,
        thread_id: &str,
        body: String,
    ) -> Option<OptimisticReviewCommentHandle> {
        self.review_state
            .append_optimistic_review_reply(thread_id, body)
    }
}
