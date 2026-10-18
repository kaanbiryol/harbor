use gpui::{AppContext, Context};
use harbor_domain::{PullRequestComment, PullRequestReview, RepoId, ReviewThread};
use harbor_sync::{PullRequestReviewRefreshRequest, SyncTarget, refresh_pull_request_reviews};

use crate::workspace::{
    AppView, PullRequestDetailCacheKey, SelectedPullRequestTaskKind,
    async_updates::AppViewAsyncUpdateExt,
    pull_request_detail_loaders::selected_pull_request_matches,
};

#[derive(Clone, Debug)]
pub(in crate::workspace) struct ReviewDataLoadTarget {
    repo: RepoId,
    number: u64,
    head_sha: String,
    generation: u64,
}

impl ReviewDataLoadTarget {
    pub(in crate::workspace) fn new(
        repo: RepoId,
        number: u64,
        head_sha: impl Into<String>,
        generation: u64,
    ) -> Self {
        Self {
            repo,
            number,
            head_sha: head_sha.into(),
            generation,
        }
    }

    fn detail_key(&self) -> PullRequestDetailCacheKey {
        PullRequestDetailCacheKey::new(self.repo.clone(), self.number, self.head_sha.clone())
    }
}

impl AppView {
    pub(crate) fn apply_loaded_review_data(
        &mut self,
        reviews: Vec<PullRequestReview>,
        pull_request_comments: Vec<PullRequestComment>,
        review_threads: Vec<ReviewThread>,
        current_user_login: Option<String>,
        pending_review_comment_count: Option<usize>,
    ) -> usize {
        let unresolved_count = self.update_overview_review_data(move |review_state| {
            review_state.apply_loaded_review_data(
                reviews,
                pull_request_comments,
                review_threads,
                current_user_login,
                pending_review_comment_count,
            )
        });

        self.set_selected_unresolved_thread_count(unresolved_count);
        unresolved_count
    }

    pub(crate) fn replace_pull_request_comments(
        &mut self,
        pull_request_comments: Vec<PullRequestComment>,
    ) {
        self.update_overview_review_data(move |review_state| {
            review_state.replace_pull_request_comments(pull_request_comments);
        });
    }

    pub(crate) fn replace_loaded_review_threads(&mut self, review_threads: Vec<ReviewThread>) {
        let unresolved_count = self.update_overview_review_data(move |review_state| {
            review_state.replace_loaded_review_threads(review_threads)
        });
        self.set_selected_unresolved_thread_count(unresolved_count);
    }

    pub(crate) fn replace_reviews_and_loaded_threads(
        &mut self,
        reviews: Vec<PullRequestReview>,
        pull_request_comments: Vec<PullRequestComment>,
        review_threads: Vec<ReviewThread>,
    ) {
        let unresolved_count = self.update_overview_review_data(move |review_state| {
            review_state.replace_reviews_and_loaded_threads(
                reviews,
                pull_request_comments,
                review_threads,
            )
        });
        self.set_selected_unresolved_thread_count(unresolved_count);
    }

    pub(in crate::workspace) fn sync_unresolved_thread_count(&mut self) -> usize {
        let unresolved_count = self.review_state.unresolved_thread_count();
        self.set_selected_unresolved_thread_count(unresolved_count);
        unresolved_count
    }

    fn set_selected_unresolved_thread_count(&mut self, unresolved_count: usize) {
        if let Some(selected) = self
            .pull_requests
            .get_mut(self.selection_state.pull_request_index())
        {
            selected.unresolved_threads = unresolved_count;
        }
    }

    pub(crate) fn load_selected_review_data(&mut self, cx: &mut Context<Self>) {
        let Some(pull_request) = self.selected_pull_request().cloned() else {
            return;
        };
        self.mark_active_inbox_stale();
        let repo = pull_request.repo;
        let number = pull_request.number;

        self.review_state.start_reviews_load();
        self.status = format!("Refreshing review data for PR #{number}");
        cx.notify();

        let review_data_generation = self.review_state.next_review_data_generation();
        let target =
            ReviewDataLoadTarget::new(repo, number, pull_request.head_sha, review_data_generation);

        self.spawn_review_data_loader(target, ReviewDataLoadMode::Refresh, cx);
    }

    pub(in crate::workspace) fn spawn_review_data_loader(
        &mut self,
        target: ReviewDataLoadTarget,
        mode: ReviewDataLoadMode,
        cx: &mut Context<Self>,
    ) {
        let auth_generation = self.auth_generation;
        self.review_state.start_reviews_load();
        let cached_current_user_login = self.review_state.current_user_login().map(str::to_string);
        let existing_pending_review = self.review_state.pending_review_cloned();
        let github_api = self.github_api_for_request();
        let store = self.cache_store();
        let detail_key = target.detail_key();
        self.tasks.set_selected_pull_request_task(
            SelectedPullRequestTaskKind::Reviews,
            cx.spawn(async move |this, cx| {
            tracing::info!(
                repository = %target.repo.full_name(),
                pull_request = target.number,
                mode = ?mode,
                "github graphql source: selected pull request review threads"
            );
            let refresh = cx.background_spawn({ let target = target.clone(); async move { refresh_pull_request_reviews(
                github_api.as_ref(),
                PullRequestReviewRefreshRequest {
                    store: store.as_ref(),
                    repository: &target.repo,
                    number: target.number,
                    head_sha: &target.head_sha,
                    cached_current_user: cached_current_user_login,
                    existing_pending_review: existing_pending_review.as_ref(),
                },
            )
            .await } }).await;

            this.update_in_session_or_log(cx, auth_generation, mode.update_error_log_message(), move |view, cx| {
                if !selected_pull_request_matches(view, &detail_key) {
                    return;
                }
                if view.review_state.review_data_generation() != target.generation {
                    return;
                }

                if let Some(error) = refresh.cache_error {
                    view.repository_state.set_error(error.to_string());
                }
                let incomplete_comments = refresh.threads.as_ref().is_ok_and(|threads| threads.iter().any(|thread| !thread.comments_complete));
                let mut review_error = incomplete_comments.then(|| "Some review comments are not loaded because the pagination limit was reached. Open the thread on GitHub to see the full conversation.".to_string());
                let current_user_login = match refresh.current_user {
                    Ok(login) => Some(login),
                    Err(error) => {
                        append_review_error(
                            &mut review_error,
                            format!("Failed to detect current user: {error}"),
                        );
                        None
                    }
                };
                let pending_review_comment_count = match refresh.pending_review_comment_count {
                    Some(Ok(count)) => Some(count),
                    Some(Err(error)) => {
                        let message = format!("Failed to count pending review comments: {error}");
                        append_review_error(&mut review_error, message);
                        None
                    }
                    None => None,
                };
                let pull_request_comments = match refresh.comments {
                    Ok(comments) => comments,
                    Err(error) => {
                        append_review_error(
                            &mut review_error,
                            format!("Failed to load pull request comments: {error}"),
                        );
                        Vec::new()
                    }
                };

                match (refresh.reviews, refresh.threads) {
                    (Ok(reviews), Ok(threads)) => {
                        if incomplete_comments {
                            view.mark_sync_failure(SyncTarget::SelectedPullRequestReviews);
                        } else {
                            view.mark_sync_success(SyncTarget::SelectedPullRequestReviews);
                        }
                        let thread_count = threads.len();
                        view.apply_loaded_review_data(
                            reviews,
                            pull_request_comments,
                            threads,
                            current_user_login,
                            pending_review_comment_count,
                        );
                        view.sync_diff_list_items(cx);
                        view.refresh_owned_file_filters(cx);
                        view.status = mode.loaded_review_data_status(
                            target.number,
                            thread_count,
                            review_error.is_some(),
                        );
                    }
                    (Err(reviews_error), Ok(threads)) => {
                        view.mark_sync_failure(SyncTarget::SelectedPullRequestReviews);
                        let thread_count = threads.len();
                        view.review_state.clear_pull_request_reviews();
                        view.replace_pull_request_comments(pull_request_comments);
                        view.replace_loaded_review_threads(threads);
                        view.sync_diff_list_items(cx);
                        let message = format!("Failed to load review history: {reviews_error}");
                        append_review_error(&mut review_error, message);
                        view.status = mode.loaded_threads_only_status(target.number, thread_count);
                    }
                    (Ok(reviews), Err(threads_error)) => {
                        view.mark_sync_failure(SyncTarget::SelectedPullRequestReviews);
                        view.apply_loaded_review_data(
                            reviews,
                            pull_request_comments,
                            Vec::new(),
                            current_user_login,
                            pending_review_comment_count,
                        );
                        view.sync_diff_list_items(cx);
                        view.refresh_owned_file_filters(cx);
                        let message = format!("Failed to load review threads: {threads_error}");
                        append_review_error(&mut review_error, message);
                        view.status = mode.loaded_reviews_only_status(target.number);
                    }
                    (Err(reviews_error), Err(threads_error)) => {
                        view.mark_sync_failure(SyncTarget::SelectedPullRequestReviews);
                        view.review_state.clear_pull_request_reviews();
                        view.replace_pull_request_comments(pull_request_comments);
                        let message = format!(
                            "Failed to load review history: {reviews_error}; Failed to load review threads: {threads_error}"
                        );
                        append_review_error(&mut review_error, message);
                        view.status = mode.failed_status(target.number);
                    }
                }
                if let Some(error) = review_error {
                    view.review_state.apply_reviews_failure(error);
                } else {
                    view.review_state.apply_reviews_success();
                }

                view.cache_current_pull_request_detail_snapshot();
                cx.notify();
            });
            }),
        );
    }
}

fn append_review_error(error: &mut Option<String>, message: String) {
    *error = Some(match error.take() {
        Some(existing) => format!("{existing}; {message}"),
        None => message,
    });
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::workspace) enum ReviewDataLoadMode {
    Initial,
    Refresh,
}

impl ReviewDataLoadMode {
    fn loaded_review_data_status(
        self,
        number: u64,
        thread_count: usize,
        has_warnings: bool,
    ) -> String {
        match (self, has_warnings) {
            (Self::Initial, false) => {
                format!("Loaded review history and {thread_count} threads for PR #{number}")
            }
            (Self::Initial, true) => {
                format!(
                    "Loaded {thread_count} review threads for PR #{number}, with review warnings"
                )
            }
            (Self::Refresh, false) => {
                format!("Refreshed review data and {thread_count} threads for PR #{number}")
            }
            (Self::Refresh, true) => {
                format!(
                    "Refreshed review data and {thread_count} threads for PR #{number}, with warnings"
                )
            }
        }
    }

    fn loaded_threads_only_status(self, number: u64, thread_count: usize) -> String {
        match self {
            Self::Initial => {
                format!(
                    "Loaded {thread_count} review threads for PR #{number}, with review warnings"
                )
            }
            Self::Refresh => {
                format!(
                    "Refreshed {thread_count} review threads for PR #{number}, but review history failed"
                )
            }
        }
    }

    fn loaded_reviews_only_status(self, number: u64) -> String {
        match self {
            Self::Initial => format!("Failed to load review data for PR #{number}"),
            Self::Refresh => {
                format!("Refreshed review history for PR #{number}, but threads failed")
            }
        }
    }

    fn failed_status(self, number: u64) -> String {
        match self {
            Self::Initial => format!("Failed to load review data for PR #{number}"),
            Self::Refresh => format!("Failed to refresh review data for PR #{number}"),
        }
    }

    fn update_error_log_message(self) -> &'static str {
        match self {
            Self::Initial => "failed to update pull request review state",
            Self::Refresh => "failed to update refreshed review state",
        }
    }
}
