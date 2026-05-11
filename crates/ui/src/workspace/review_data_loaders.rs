use gpui::Context;
use harbor_domain::{PullRequestReview, PullRequestReviewState, RepoId};
use harbor_sync::SyncTarget;

use crate::workspace::{AppView, async_updates::AppViewAsyncUpdateExt};

#[derive(Clone, Debug)]
pub(super) struct ReviewDataLoadTarget {
    repo: RepoId,
    owner: String,
    name: String,
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
            owner: repo.owner.clone(),
            name: repo.name.clone(),
            repo,
            number,
            head_sha: head_sha.into(),
            generation,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ReviewDataLoadMode {
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
        let cached_current_user_login = self.review_state.current_user_login.clone();
        let existing_pending_review = self.review_state.pending_review.clone();
        let github_api = self.github_api.clone();
        let store = self.repository_state.store();
        self.tasks.push_pull_request_detail_task(cx.spawn(async move |this, cx| {
            let current_user_result = match cached_current_user_login {
                Some(login) => Ok(login),
                None => github_api.current_user().await,
            };
            let reviews_result = github_api
                .list_pull_request_reviews(&target.owner, &target.name, target.number)
                .await;
            let pending_review_comment_count_result = if let Ok(reviews) = reviews_result.as_ref()
            {
                if let Some(review_id) = pending_review_rest_id(
                    reviews,
                    current_user_result.as_ref().ok().map(String::as_str),
                    existing_pending_review.as_ref(),
                ) {
                    Some(
                        github_api
                            .pull_request_review_comment_count(
                                &target.owner,
                                &target.name,
                                target.number,
                                &review_id,
                            )
                            .await,
                    )
                } else {
                    None
                }
            } else {
                None
            };
            tracing::info!(
                repository = %target.repo.full_name(),
                pull_request = target.number,
                mode = ?mode,
                "github graphql source: selected pull request review threads"
            );
            let threads_result = github_api
                .list_review_threads(&target.owner, &target.name, target.number)
                .await;
            let cache_result = match (&store, reviews_result.as_ref(), threads_result.as_ref()) {
                (Some(store), Ok(reviews), Ok(threads)) => {
                    store
                        .save_pull_request_reviews(
                            &target.repo,
                            target.number,
                            &target.head_sha,
                            reviews,
                            threads,
                        )
                        .await
                        .map_err(|error| error.to_string())
                }
                (Some(store), Err(error), _) | (Some(store), _, Err(error)) => store
                    .record_sync_failure(
                        &harbor_storage::detail_target_key(
                            &target.repo,
                            target.number,
                            harbor_storage::PullRequestDetailSection::Reviews,
                        ),
                        &error.to_string(),
                    )
                    .await
                    .map_err(|error| error.to_string()),
                (None, _, _) => Ok(()),
            };

            this.update_or_log(cx, mode.update_error_log_message(), move |view, cx| {
                if !selected_pull_request_matches(view, &target.repo, target.number) {
                    return;
                }
                if view.review_data_generation() != target.generation {
                    return;
                }

                if let Err(error) = cache_result {
                    view.repository_state.set_error(error);
                }
                let mut review_error = None;
                let current_user_login = match current_user_result {
                    Ok(login) => Some(login),
                    Err(error) => {
                        append_review_error(
                            &mut review_error,
                            format!("Failed to detect current user: {error}"),
                        );
                        None
                    }
                };
                let pending_review_comment_count = match pending_review_comment_count_result {
                    Some(Ok(count)) => Some(count),
                    Some(Err(error)) => {
                        let message = format!("Failed to count pending review comments: {error}");
                        append_review_error(&mut review_error, message);
                        None
                    }
                    None => None,
                };

                match (reviews_result, threads_result) {
                    (Ok(reviews), Ok(threads)) => {
                        view.mark_sync_success(SyncTarget::SelectedPullRequestReviews);
                        let thread_count = threads.len();
                        view.apply_loaded_review_data(
                            reviews,
                            threads,
                            current_user_login,
                            pending_review_comment_count,
                        );
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
                        view.replace_loaded_review_threads(threads);
                        let message = format!("Failed to load review history: {reviews_error}");
                        append_review_error(&mut review_error, message);
                        view.status = mode.loaded_threads_only_status(target.number, thread_count);
                    }
                    (Ok(reviews), Err(threads_error)) => {
                        view.mark_sync_failure(SyncTarget::SelectedPullRequestReviews);
                        view.apply_loaded_review_data(
                            reviews,
                            Vec::new(),
                            current_user_login,
                            pending_review_comment_count,
                        );
                        view.refresh_owned_file_filters(cx);
                        let message = format!("Failed to load review threads: {threads_error}");
                        append_review_error(&mut review_error, message);
                        view.status = mode.loaded_reviews_only_status(target.number);
                    }
                    (Err(reviews_error), Err(threads_error)) => {
                        view.mark_sync_failure(SyncTarget::SelectedPullRequestReviews);
                        view.review_state.clear_pull_request_reviews();
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

pub(super) fn pending_review_rest_id(
    reviews: &[PullRequestReview],
    current_user_login: Option<&str>,
    existing_pending_review: Option<&crate::workspace::PendingReviewSession>,
) -> Option<String> {
    reviews
        .iter()
        .find(|review| {
            review.state == PullRequestReviewState::Pending
                && current_user_login.is_none_or(|login| review.author == login)
                && existing_pending_review.is_none_or(|pending_review| {
                    review.node_id.as_deref() != Some(pending_review.node_id.as_str())
                })
        })
        .map(|review| review.id.clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pending_review_rest_id_matches_current_user() {
        let reviews = vec![
            review("review-1", "maria", PullRequestReviewState::Pending),
            review("review-2", "alex", PullRequestReviewState::Pending),
        ];

        assert_eq!(
            pending_review_rest_id(&reviews, Some("alex"), None),
            Some("review-2".to_string())
        );
    }

    #[test]
    fn pending_review_rest_id_ignores_non_pending_reviews() {
        let reviews = vec![review("review-1", "alex", PullRequestReviewState::Approved)];

        assert_eq!(pending_review_rest_id(&reviews, Some("alex"), None), None);
    }

    #[test]
    fn pending_review_rest_id_skips_existing_pending_review() {
        let reviews = vec![review("review-1", "alex", PullRequestReviewState::Pending)];
        let existing_pending_review = crate::workspace::PendingReviewSession {
            node_id: "review-1-node".to_string(),
            comment_count: 2,
        };

        assert_eq!(
            pending_review_rest_id(&reviews, Some("alex"), Some(&existing_pending_review)),
            None
        );
    }

    fn review(id: &str, author: &str, state: PullRequestReviewState) -> PullRequestReview {
        PullRequestReview {
            id: id.to_string(),
            node_id: Some(format!("{id}-node")),
            author: author.to_string(),
            state,
            body: None,
            submitted_at: None,
        }
    }
}
