use gpui::Context;
use harbor_domain::RepoId;
use harbor_sync::{PullRequestReviewRefreshRequest, SyncTarget, refresh_pull_request_reviews};

use crate::workspace::{AppView, async_updates::AppViewAsyncUpdateExt};

mod load_mode;

pub(super) use load_mode::ReviewDataLoadMode;

#[derive(Clone, Debug)]
pub(super) struct ReviewDataLoadTarget {
    repo: RepoId,
    number: u64,
    head_sha: String,
    generation: u64,
}

impl ReviewDataLoadTarget {
    pub(super) fn new(
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
}

impl AppView {
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

        let review_data_generation = self.next_review_data_generation();
        let target =
            ReviewDataLoadTarget::new(repo, number, pull_request.head_sha, review_data_generation);

        self.spawn_review_data_loader(target, ReviewDataLoadMode::Refresh, cx);
    }

    pub(super) fn spawn_review_data_loader(
        &mut self,
        target: ReviewDataLoadTarget,
        mode: ReviewDataLoadMode,
        cx: &mut Context<Self>,
    ) {
        self.review_state.start_reviews_load();
        let cached_current_user_login = self.review_state.current_user_login().map(str::to_string);
        let existing_pending_review = self.review_state.pending_review_cloned();
        let github_api = self.github_api.clone();
        let store = self.repository_state.store();
        self.tasks.push_selected_pull_request_task(cx.spawn(async move |this, cx| {
            tracing::info!(
                repository = %target.repo.full_name(),
                pull_request = target.number,
                mode = ?mode,
                "github graphql source: selected pull request review threads"
            );
            let refresh = refresh_pull_request_reviews(
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
            .await;

            this.update_or_log(cx, mode.update_error_log_message(), move |view, cx| {
                if !selected_pull_request_matches(view, &target.repo, target.number) {
                    return;
                }
                if view.review_data_generation() != target.generation {
                    return;
                }

                if let Some(error) = refresh.cache_error {
                    view.repository_state.set_error(error);
                }
                let mut review_error = None;
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
                        view.mark_sync_success(SyncTarget::SelectedPullRequestReviews);
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
        }));
    }
}

pub(super) fn selected_pull_request_matches(
    view: &AppView,
    repository: &RepoId,
    number: u64,
) -> bool {
    view.selected_pull_request().is_some_and(|pull_request| {
        &pull_request.repo == repository && pull_request.number == number
    })
}

fn append_review_error(error: &mut Option<String>, message: String) {
    *error = Some(match error.take() {
        Some(existing) => format!("{existing}; {message}"),
        None => message,
    });
}
