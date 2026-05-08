use gpui::Context;
use harbor_domain::{PullRequestReview, PullRequestReviewState, RepoId};
use harbor_github::{GhCliTransport, GitHubClient};

use crate::workspace::AppView;

impl AppView {
    pub(crate) fn load_selected_review_data(&mut self, cx: &mut Context<Self>) {
        let Some(pull_request) = self.selected_pull_request().cloned() else {
            return;
        };
        let repo = pull_request.repo;
        let number = pull_request.number;

        self.is_loading_reviews = true;
        self.reviews_error = None;
        self.status = format!("Refreshing review data for PR #{number}");
        cx.notify();

        let review_data_generation = self.next_review_data_generation();
        let owner = repo.owner.clone();
        let name = repo.name.clone();

        self.pr_detail_tasks.push(cx.spawn(async move |this, cx| {
            let client = GitHubClient::new(GhCliTransport);
            let current_user_result = client.current_user().await;
            let reviews_result = client
                .list_pull_request_reviews(&owner, &name, number)
                .await;
            let pending_review_comment_count_result = if let Ok(reviews) = reviews_result.as_ref()
            {
                if let Some(review_id) = pending_review_rest_id(
                    reviews,
                    current_user_result.as_ref().ok().map(String::as_str),
                ) {
                    Some(
                        client
                            .pull_request_review_comment_count(&owner, &name, number, &review_id)
                            .await,
                    )
                } else {
                    None
                }
            } else {
                None
            };
            let threads_result = client.list_review_threads(&owner, &name, number).await;

            if let Err(error) = this.update(cx, move |view, cx| {
                if !selected_pull_request_matches(view, &repo, number) {
                    return;
                }
                if view.review_data_generation() != review_data_generation {
                    return;
                }

                view.is_loading_reviews = false;
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
                        if view.reviews_error.is_none() {
                            view.status =
                                format!("Refreshed review data and {thread_count} threads for PR #{number}");
                        } else {
                            view.status =
                                format!("Refreshed review data and {thread_count} threads for PR #{number}, with warnings");
                        }
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
                        view.status = format!(
                            "Refreshed {thread_count} review threads for PR #{number}, but review history failed"
                        );
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
                        view.status =
                            format!("Refreshed review history for PR #{number}, but threads failed");
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
                        view.status = format!("Failed to refresh review data for PR #{number}");
                    }
                }

                view.cache_current_pull_request_detail_snapshot();
                cx.notify();
            }) {
                tracing::warn!(%error, "failed to update refreshed review state");
            }
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
) -> Option<String> {
    reviews
        .iter()
        .find(|review| {
            review.state == PullRequestReviewState::Pending
                && current_user_login.is_none_or(|login| review.author == login)
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
            pending_review_rest_id(&reviews, Some("alex")),
            Some("review-2".to_string())
        );
    }

    #[test]
    fn pending_review_rest_id_ignores_non_pending_reviews() {
        let reviews = vec![review("review-1", "alex", PullRequestReviewState::Approved)];

        assert_eq!(pending_review_rest_id(&reviews, Some("alex")), None);
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
