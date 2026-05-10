use harbor_domain::{
    PullRequestReview, ReactionContent, ReviewComment, ReviewCommentPosition, ReviewCommentRange,
    ReviewThread, ReviewThreadState,
};

use crate::workspace::{
    AppView, PendingReviewSession, PullRequestDetailCacheKey,
    reviews::{
        LOCAL_REVIEW_COMMENT_ID_PREFIX, LOCAL_REVIEW_THREAD_ID_PREFIX,
        OptimisticReviewCommentHandle, ReviewReactionKey, apply_review_reaction_overrides,
        apply_review_thread_state_overrides, merge_optimistic_review_threads,
        pending_review_from_reviews, remove_review_comment_from_threads,
        review_position_from_range, rollback_pending_review_comment_count,
        set_review_comment_reaction_state, unresolved_review_thread_count,
    },
};

impl AppView {
    pub(crate) fn clear_review_composer_state(&mut self) {
        self.review_state.review_composer_state.composer = None;
        self.review_state.review_composer_state.line_selection = None;
        self.review_state.review_comment_error = None;
        self.review_state
            .review_composer_state
            .thread_reply_thread_id = None;
        self.review_state.review_thread_reply_error = None;
        self.review_state
            .review_composer_state
            .comment_edit_comment_id = None;
        self.review_state.review_comment_edit_error = None;
        self.review_state.review_comment_action_comment_id = None;
        self.review_state.review_comment_action_error = None;
        self.review_state.review_reaction_action = None;
        self.review_state.review_reaction_error = None;
    }

    pub(crate) fn apply_loaded_review_data(
        &mut self,
        reviews: Vec<PullRequestReview>,
        mut review_threads: Vec<ReviewThread>,
        current_user_login: Option<String>,
        pending_review_comment_count: Option<usize>,
    ) -> usize {
        let existing_pending_review = self.review_state.pending_review.clone();
        self.review_state.current_user_login = current_user_login;
        self.review_state.pending_review = pending_review_from_reviews(
            &reviews,
            self.review_state.current_user_login.as_deref(),
            existing_pending_review.as_ref(),
            pending_review_comment_count,
        );
        self.review_state.pull_request_reviews = reviews;
        let settled_thread_state_overrides = apply_review_thread_state_overrides(
            &mut review_threads,
            &self.review_state.review_thread_state_overrides,
        );
        let settled_reaction_overrides = apply_review_reaction_overrides(
            &mut review_threads,
            &self.review_state.review_reaction_overrides,
        );
        self.remove_review_thread_state_overrides(settled_thread_state_overrides);
        self.remove_review_reaction_overrides(settled_reaction_overrides);
        self.review_state.review_threads =
            merge_optimistic_review_threads(review_threads, &self.review_state.review_threads);

        self.sync_unresolved_thread_count()
    }

    pub(crate) fn replace_loaded_review_threads(&mut self, mut review_threads: Vec<ReviewThread>) {
        let settled_thread_state_overrides = apply_review_thread_state_overrides(
            &mut review_threads,
            &self.review_state.review_thread_state_overrides,
        );
        let settled_reaction_overrides = apply_review_reaction_overrides(
            &mut review_threads,
            &self.review_state.review_reaction_overrides,
        );
        self.remove_review_thread_state_overrides(settled_thread_state_overrides);
        self.remove_review_reaction_overrides(settled_reaction_overrides);
        self.review_state.review_threads =
            merge_optimistic_review_threads(review_threads, &self.review_state.review_threads);
        self.sync_unresolved_thread_count();
    }

    pub(super) fn sync_unresolved_thread_count(&mut self) -> usize {
        let unresolved_count = unresolved_review_thread_count(&self.review_state.review_threads);

        if let Some(selected) = self.pull_requests.get_mut(self.selected_pr) {
            selected.unresolved_threads = unresolved_count;
        }

        unresolved_count
    }

    pub(super) fn set_review_thread_state(&mut self, thread_id: &str, state: ReviewThreadState) {
        if let Some(thread) = self
            .review_state
            .review_threads
            .iter_mut()
            .find(|thread| thread.id == thread_id)
        {
            thread.state = state;
        }
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
        self.review_state
            .review_threads
            .iter()
            .flat_map(|thread| thread.comments.iter())
            .find(|comment| comment.id == comment_id)
    }

    pub(super) fn review_comment_mut(&mut self, comment_id: &str) -> Option<&mut ReviewComment> {
        self.review_state
            .review_threads
            .iter_mut()
            .flat_map(|thread| thread.comments.iter_mut())
            .find(|comment| comment.id == comment_id)
    }

    pub(super) fn remove_review_comment(&mut self, comment_id: &str) {
        remove_review_comment_from_threads(&mut self.review_state.review_threads, comment_id);
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
            rollback_pending_review_comment_count(
                &mut self.review_state.pending_review,
                previous_pending_review,
            );
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
            self.review_state.pending_review = Some(pending_review.clone());
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
        let Some(comment) = self.review_comment_mut(comment_id) else {
            return;
        };

        set_review_comment_reaction_state(comment, content, viewer_has_reacted);
    }

    fn remove_review_reaction_overrides(&mut self, keys: Vec<ReviewReactionKey>) {
        for key in keys {
            self.review_state.review_reaction_overrides.remove(&key);
        }
    }

    fn remove_review_thread_state_overrides(&mut self, thread_ids: Vec<String>) {
        for thread_id in thread_ids {
            self.review_state
                .review_thread_state_overrides
                .remove(&thread_id);
        }
    }

    pub(super) fn insert_optimistic_review_thread(
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

        self.review_state.review_threads.push(ReviewThread {
            id: format!("{LOCAL_REVIEW_THREAD_ID_PREFIX}{sequence}"),
            path: range.path.clone(),
            range: Some(range),
            state: ReviewThreadState::Unresolved,
            comments: vec![comment],
        });
        self.sync_unresolved_thread_count();

        OptimisticReviewCommentHandle { comment_id }
    }

    pub(super) fn append_optimistic_review_reply(
        &mut self,
        thread_id: &str,
        body: String,
    ) -> Option<OptimisticReviewCommentHandle> {
        let thread_index = self
            .review_state
            .review_threads
            .iter()
            .position(|thread| thread.id == thread_id)?;

        let position = self.review_state.review_threads[thread_index]
            .range
            .as_ref()
            .map(review_position_from_range)
            .or_else(|| {
                self.review_state.review_threads[thread_index]
                    .comments
                    .iter()
                    .find_map(|comment| comment.position.clone())
            });
        let sequence = self.next_local_review_comment_sequence();
        let comment_id = format!("{LOCAL_REVIEW_COMMENT_ID_PREFIX}{sequence}");
        let comment = self.optimistic_review_comment(comment_id.clone(), position, body);

        self.review_state.review_threads[thread_index]
            .comments
            .push(comment);

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
                .review_state
                .current_user_login
                .clone()
                .unwrap_or_else(|| "you".to_string()),
            author_avatar_url: None,
            body,
            created_at: chrono::Utc::now(),
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
        self.review_state.local_review_comment_sequence = self
            .review_state
            .local_review_comment_sequence
            .saturating_add(1);
        self.review_state.local_review_comment_sequence
    }
}
