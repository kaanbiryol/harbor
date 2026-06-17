use std::collections::HashMap;

use harbor_domain::{
    PullRequestComment, PullRequestReview, ReactionContent, ReviewComment, ReviewThread,
    ReviewThreadState,
};

use crate::workspace::{
    PendingReviewSession, ReviewCommentUiError, ReviewComposerState, ReviewReactionAction,
    ReviewReactionKey, ReviewThreadUiError,
    reviews::{
        apply_review_reaction_overrides, apply_review_thread_state_overrides,
        increment_pending_review_comment_count, merge_optimistic_review_threads,
        pending_review_from_reviews, remove_review_comment_from_threads,
        rollback_pending_review_comment_count, set_review_comment_reaction_state,
        unresolved_review_thread_count,
    },
    status::LoadStatus,
};

#[path = "review_runtime/actions.rs"]
mod actions;
#[path = "review_runtime/optimistic_comments.rs"]
mod optimistic_comments;

pub(crate) struct ReviewRuntimeState {
    pub(crate) pull_request_reviews: Vec<PullRequestReview>,
    pub(crate) pull_request_comments: Vec<PullRequestComment>,
    pub(crate) review_threads: Vec<ReviewThread>,
    pub(crate) review_composer_state: ReviewComposerState,
    pending_review: Option<PendingReviewSession>,
    is_submitting_review_comment: bool,
    is_submitting_review_thread_reply: bool,
    is_submitting_review_comment_edit: bool,
    is_submitting_pending_review: bool,
    review_thread_action_thread_id: Option<String>,
    review_comment_action_comment_id: Option<String>,
    review_reaction_action: Option<ReviewReactionAction>,
    review_thread_state_overrides: HashMap<String, ReviewThreadState>,
    review_reaction_overrides: HashMap<ReviewReactionKey, bool>,
    reviews_load: LoadStatus,
    review_comment_error: Option<String>,
    review_thread_reply_error: Option<ReviewThreadUiError>,
    review_thread_action_error: Option<ReviewThreadUiError>,
    review_comment_edit_error: Option<ReviewCommentUiError>,
    review_comment_action_error: Option<ReviewCommentUiError>,
    review_reaction_error: Option<ReviewCommentUiError>,
    pending_review_error: Option<String>,
    pub(crate) current_user_login: Option<String>,
    local_review_comment_sequence: u64,
    review_data_generation: u64,
}

impl ReviewRuntimeState {
    pub(crate) fn new(
        pull_request_reviews: Vec<PullRequestReview>,
        review_threads: Vec<ReviewThread>,
        review_composer_state: ReviewComposerState,
    ) -> Self {
        Self {
            pull_request_reviews,
            pull_request_comments: Vec::new(),
            review_threads,
            review_composer_state,
            pending_review: None,
            is_submitting_review_comment: false,
            is_submitting_review_thread_reply: false,
            is_submitting_review_comment_edit: false,
            is_submitting_pending_review: false,
            review_thread_action_thread_id: None,
            review_comment_action_comment_id: None,
            review_reaction_action: None,
            review_thread_state_overrides: HashMap::new(),
            review_reaction_overrides: HashMap::new(),
            reviews_load: LoadStatus::Idle,
            review_comment_error: None,
            review_thread_reply_error: None,
            review_thread_action_error: None,
            review_comment_edit_error: None,
            review_comment_action_error: None,
            review_reaction_error: None,
            pending_review_error: None,
            current_user_login: None,
            local_review_comment_sequence: 0,
            review_data_generation: 0,
        }
    }

    pub(crate) fn reset_reviews_load(&mut self) {
        self.reviews_load.reset();
    }

    pub(crate) fn mark_reviews_stale(&mut self) {
        self.reviews_load.reset();
    }

    pub(crate) fn start_reviews_load(&mut self) {
        self.reviews_load.start();
    }

    pub(crate) fn apply_reviews_success(&mut self) {
        self.reviews_load.succeed();
    }

    pub(crate) fn apply_reviews_failure(&mut self, error: impl Into<String>) {
        self.reviews_load.fail(error);
    }

    pub(crate) fn reviews_loading(&self) -> bool {
        self.reviews_load.is_loading()
    }

    pub(crate) fn reviews_finished(&self) -> bool {
        self.reviews_load.is_finished()
    }

    pub(crate) fn should_load_reviews(&self) -> bool {
        !self.reviews_load.is_loading() && !self.reviews_load.is_finished()
    }

    pub(crate) fn reviews_error(&self) -> Option<&str> {
        self.reviews_load.error()
    }

    pub(crate) fn clear_reviews_error(&mut self) {
        self.reviews_load.clear_error();
    }

    pub(crate) fn pending_review(&self) -> Option<&PendingReviewSession> {
        self.pending_review.as_ref()
    }

    pub(crate) fn pending_review_cloned(&self) -> Option<PendingReviewSession> {
        self.pending_review.clone()
    }

    pub(crate) fn has_pending_review(&self) -> bool {
        self.pending_review.is_some()
    }

    pub(crate) fn clear_pending_review(&mut self) {
        self.pending_review = None;
    }

    pub(crate) fn increment_pending_review_comment_count(&mut self) {
        increment_pending_review_comment_count(&mut self.pending_review);
    }

    pub(crate) fn set_review_thread_state_override(
        &mut self,
        thread_id: String,
        state: ReviewThreadState,
    ) {
        self.review_thread_state_overrides.insert(thread_id, state);
    }

    pub(crate) fn remove_review_thread_state_override(&mut self, thread_id: &str) {
        self.review_thread_state_overrides.remove(thread_id);
    }

    pub(crate) fn set_review_reaction_override(
        &mut self,
        key: ReviewReactionKey,
        viewer_has_reacted: bool,
    ) {
        self.review_reaction_overrides
            .insert(key, viewer_has_reacted);
    }

    pub(crate) fn remove_review_reaction_override(&mut self, key: &ReviewReactionKey) {
        self.review_reaction_overrides.remove(key);
    }

    pub(crate) fn next_review_data_generation(&mut self) -> u64 {
        self.review_data_generation = self.review_data_generation.saturating_add(1);
        self.review_data_generation
    }

    pub(crate) fn review_data_generation(&self) -> u64 {
        self.review_data_generation
    }

    pub(crate) fn clear_review_data(&mut self) {
        self.pull_request_reviews.clear();
        self.pull_request_comments.clear();
        self.review_threads.clear();
        self.clear_composer_and_action_state();
        self.pending_review = None;
    }

    pub(crate) fn restore_review_snapshot(
        &mut self,
        pull_request_reviews: Vec<PullRequestReview>,
        pull_request_comments: Vec<PullRequestComment>,
        review_threads: Vec<ReviewThread>,
        pending_review: Option<PendingReviewSession>,
        current_user_login: Option<String>,
        reviews_loaded: bool,
    ) {
        self.pull_request_reviews = pull_request_reviews;
        self.pull_request_comments = pull_request_comments;
        self.review_threads = review_threads;
        self.pending_review = pending_review;
        self.current_user_login = current_user_login;
        if reviews_loaded {
            self.apply_reviews_success();
        } else {
            self.reset_reviews_load();
        }
    }

    pub(crate) fn apply_loaded_review_data(
        &mut self,
        reviews: Vec<PullRequestReview>,
        pull_request_comments: Vec<PullRequestComment>,
        review_threads: Vec<ReviewThread>,
        current_user_login: Option<String>,
        pending_review_comment_count: Option<usize>,
    ) -> usize {
        let existing_pending_review = self.pending_review.clone();
        self.current_user_login = current_user_login;
        self.pending_review = pending_review_from_reviews(
            &reviews,
            self.current_user_login.as_deref(),
            existing_pending_review.as_ref(),
            pending_review_comment_count,
        );
        self.pull_request_reviews = reviews;
        self.pull_request_comments = pull_request_comments;
        self.apply_loaded_review_threads(review_threads)
    }

    pub(crate) fn replace_pull_request_comments(
        &mut self,
        pull_request_comments: Vec<PullRequestComment>,
    ) {
        self.pull_request_comments = pull_request_comments;
    }

    pub(crate) fn replace_loaded_review_threads(
        &mut self,
        review_threads: Vec<ReviewThread>,
    ) -> usize {
        self.apply_loaded_review_threads(review_threads)
    }

    pub(crate) fn replace_reviews_and_loaded_threads(
        &mut self,
        reviews: Vec<PullRequestReview>,
        pull_request_comments: Vec<PullRequestComment>,
        review_threads: Vec<ReviewThread>,
    ) -> usize {
        self.pull_request_reviews = reviews;
        self.pull_request_comments = pull_request_comments;
        self.apply_loaded_review_threads(review_threads)
    }

    pub(crate) fn clear_pull_request_reviews(&mut self) {
        self.pull_request_reviews.clear();
    }

    fn apply_loaded_review_threads(&mut self, mut review_threads: Vec<ReviewThread>) -> usize {
        let settled_thread_state_overrides = apply_review_thread_state_overrides(
            &mut review_threads,
            &self.review_thread_state_overrides,
        );
        let settled_reaction_overrides =
            apply_review_reaction_overrides(&mut review_threads, &self.review_reaction_overrides);
        self.remove_review_thread_state_overrides(settled_thread_state_overrides);
        self.remove_review_reaction_overrides(settled_reaction_overrides);
        self.review_threads = merge_optimistic_review_threads(review_threads, &self.review_threads);
        self.unresolved_thread_count()
    }

    pub(crate) fn unresolved_thread_count(&self) -> usize {
        unresolved_review_thread_count(&self.review_threads)
    }

    pub(crate) fn set_review_thread_state(&mut self, thread_id: &str, state: ReviewThreadState) {
        if let Some(thread) = self
            .review_threads
            .iter_mut()
            .find(|thread| thread.id == thread_id)
        {
            thread.state = state;
        }
    }

    pub(crate) fn review_comment(&self, comment_id: &str) -> Option<&ReviewComment> {
        self.review_threads
            .iter()
            .flat_map(|thread| thread.comments.iter())
            .find(|comment| comment.id == comment_id)
    }

    pub(crate) fn review_comment_mut(&mut self, comment_id: &str) -> Option<&mut ReviewComment> {
        self.review_threads
            .iter_mut()
            .flat_map(|thread| thread.comments.iter_mut())
            .find(|comment| comment.id == comment_id)
    }

    pub(crate) fn remove_review_comment(&mut self, comment_id: &str) {
        remove_review_comment_from_threads(&mut self.review_threads, comment_id);
    }

    pub(crate) fn rollback_pending_review_comment_count(
        &mut self,
        previous_pending_review: Option<&PendingReviewSession>,
    ) {
        rollback_pending_review_comment_count(&mut self.pending_review, previous_pending_review);
    }

    pub(crate) fn set_pending_review(&mut self, pending_review: PendingReviewSession) {
        self.pending_review = Some(pending_review);
    }

    pub(crate) fn set_review_comment_reaction(
        &mut self,
        comment_id: &str,
        content: ReactionContent,
        viewer_has_reacted: bool,
    ) {
        if let Some(comment) = self.review_comment_mut(comment_id) {
            set_review_comment_reaction_state(comment, content, viewer_has_reacted);
        }
    }

    fn remove_review_reaction_overrides(&mut self, keys: Vec<ReviewReactionKey>) {
        for key in keys {
            self.review_reaction_overrides.remove(&key);
        }
    }

    fn remove_review_thread_state_overrides(&mut self, thread_ids: Vec<String>) {
        for thread_id in thread_ids {
            self.review_thread_state_overrides.remove(&thread_id);
        }
    }
}
