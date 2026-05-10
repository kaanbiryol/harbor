use gpui::Context;
use harbor_domain::{PullRequestReview, PullRequestReviewState, RepoId};

use crate::workspace::{AppView, async_updates::AppViewAsyncUpdateExt};

#[derive(Clone, Debug)]
pub(super) struct ReviewDataLoadTarget {
    repo: RepoId,
    owner: String,
    name: String,
    number: u64,
    generation: u64,
}

impl ReviewDataLoadTarget {
    pub(super) fn new(repo: RepoId, number: u64, generation: u64) -> Self {
        Self {
            owner: repo.owner.clone(),
            name: repo.name.clone(),
            repo,
            number,
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
        let repo = pull_request.repo;
        let number = pull_request.number;

        self.detail_loading.reviews = true;
        self.reviews_error = None;
        self.status = format!("Refreshing review data for PR #{number}");
        cx.notify();

        let review_data_generation = self.next_review_data_generation();
        let target = ReviewDataLoadTarget::new(repo, number, review_data_generation);

        self.spawn_review_data_loader(target, ReviewDataLoadMode::Refresh, cx);
    }

    pub(super) fn spawn_review_data_loader(
        &mut self,
        target: ReviewDataLoadTarget,
        mode: ReviewDataLoadMode,
        cx: &mut Context<Self>,
    ) {
        self.detail_loading.reviews = true;
        self.detail_loaded.reviews = false;
        let cached_current_user_login = self.current_user_login.clone();
        let existing_pending_review = self.pending_review.clone();
        let github_api = self.github_api.clone();
        self.pr_detail_tasks.push(cx.spawn(async move |this, cx| {
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
            let threads_result = github_api
                .list_review_threads(&target.owner, &target.name, target.number)
                .await;

            this.update_or_log(cx, mode.update_error_log_message(), move |view, cx| {
                if !selected_pull_request_matches(view, &target.repo, target.number) {
                    return;
                }
                if view.review_data_generation() != target.generation {
                    return;
                }

                view.detail_loading.reviews = false;
                view.detail_loaded.reviews = true;
                view.reviews_error = None;
                let current_user_login = match current_user_result {
                    Ok(login) => Some(login),
                    Err(error) => {
                        view.reviews_error =
                            Some(format!("Failed to detect current user: {error}"));
                        None
                    }
                };
                let pending_review_comment_count = match pending_review_comment_count_result {
                    Some(Ok(count)) => Some(count),
                    Some(Err(error)) => {
                        let message = format!("Failed to count pending review comments: {error}");
                        view.reviews_error = Some(match view.reviews_error.take() {
                            Some(existing) => format!("{existing}; {message}"),
                            None => message,
                        });
                        None
                    }
                    None => None,
                };

                match (reviews_result, threads_result) {
                    (Ok(reviews), Ok(threads)) => {
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
                            view.reviews_error.is_some(),
                        );
                    }
                    (Err(reviews_error), Ok(threads)) => {
                        let thread_count = threads.len();
                        view.pull_request_reviews.clear();
                        view.replace_loaded_review_threads(threads);
                        let message = format!("Failed to load review history: {reviews_error}");
                        view.reviews_error = Some(match view.reviews_error.take() {
                            Some(existing) => format!("{existing}; {message}"),
                            None => message,
                        });
                        view.status = mode.loaded_threads_only_status(target.number, thread_count);
                    }
                    (Ok(reviews), Err(threads_error)) => {
                        view.apply_loaded_review_data(
                            reviews,
                            Vec::new(),
                            current_user_login,
                            pending_review_comment_count,
                        );
                        view.refresh_owned_file_filters(cx);
                        let message = format!("Failed to load review threads: {threads_error}");
                        view.reviews_error = Some(match view.reviews_error.take() {
                            Some(existing) => format!("{existing}; {message}"),
                            None => message,
                        });
                        view.status = mode.loaded_reviews_only_status(target.number);
                    }
                    (Err(reviews_error), Err(threads_error)) => {
                        view.pull_request_reviews.clear();
                        let message = format!(
                            "Failed to load review history: {reviews_error}; Failed to load review threads: {threads_error}"
                        );
                        view.reviews_error = Some(match view.reviews_error.take() {
                            Some(existing) => format!("{existing}; {message}"),
                            None => message,
                        });
                        view.status = mode.failed_status(target.number);
                    }
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
