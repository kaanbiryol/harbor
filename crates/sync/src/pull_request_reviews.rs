use futures_lite::future::zip;
use harbor_domain::{
    PullRequestComment, PullRequestReview, PullRequestReviewState, RepoId, ReviewThread,
    reviews::PendingReviewSession,
};
use harbor_github::{GitHubRepositoryApi, GitHubReviewApi, Result};
use harbor_storage::{PullRequestDetailSection, SqliteStore, detail_target_key};

pub struct PullRequestReviewRefresh {
    pub current_user: Result<String>,
    pub reviews: Result<Vec<PullRequestReview>>,
    pub pending_review_comment_count: Option<Result<usize>>,
    pub comments: Result<Vec<PullRequestComment>>,
    pub threads: Result<Vec<ReviewThread>>,
    pub cache_error: Option<String>,
}

pub struct PullRequestReviewRefreshRequest<'a> {
    pub store: Option<&'a SqliteStore>,
    pub repository: &'a RepoId,
    pub number: u64,
    pub head_sha: &'a str,
    pub cached_current_user: Option<String>,
    pub existing_pending_review: Option<&'a PendingReviewSession>,
}

pub async fn refresh_pull_request_reviews<S>(
    source: &S,
    request: PullRequestReviewRefreshRequest<'_>,
) -> PullRequestReviewRefresh
where
    S: GitHubRepositoryApi + GitHubReviewApi + ?Sized,
{
    let owner = &request.repository.owner;
    let repo = &request.repository.name;
    let current_user = load_current_user(source, request.cached_current_user);
    let reviews = source.list_pull_request_reviews(owner, repo, request.number);
    let comments = source.list_pull_request_comments(owner, repo, request.number);
    let threads = source.list_review_threads(owner, repo, request.number);
    let (current_user, (reviews, (comments, threads))) =
        zip(current_user, zip(reviews, zip(comments, threads))).await;

    let pending_review_comment_count = match reviews.as_ref().ok().and_then(|reviews| {
        pending_review_rest_id(
            reviews,
            current_user.as_ref().ok().map(String::as_str),
            request.existing_pending_review,
        )
    }) {
        Some(review_id) => Some(
            source
                .pull_request_review_comment_count(owner, repo, request.number, &review_id)
                .await,
        ),
        None => None,
    };

    let cache_error = cache_review_refresh(
        request.store,
        request.repository,
        request.number,
        request.head_sha,
        &reviews,
        &comments,
        &threads,
    )
    .await
    .err();

    PullRequestReviewRefresh {
        current_user,
        reviews,
        pending_review_comment_count,
        comments,
        threads,
        cache_error,
    }
}

async fn load_current_user<S>(source: &S, cached_current_user: Option<String>) -> Result<String>
where
    S: GitHubRepositoryApi + ?Sized,
{
    match cached_current_user {
        Some(login) => Ok(login),
        None => source.current_user().await,
    }
}

async fn cache_review_refresh(
    store: Option<&SqliteStore>,
    repository: &RepoId,
    number: u64,
    head_sha: &str,
    reviews: &Result<Vec<PullRequestReview>>,
    comments: &Result<Vec<PullRequestComment>>,
    threads: &Result<Vec<ReviewThread>>,
) -> std::result::Result<(), String> {
    let Some(store) = store else {
        return Ok(());
    };

    match (reviews, comments, threads) {
        (Ok(reviews), Ok(comments), Ok(threads)) => store
            .save_pull_request_reviews(repository, number, head_sha, reviews, comments, threads)
            .await
            .map_err(|error| error.to_string()),
        (Err(error), _, _) | (_, Err(error), _) | (_, _, Err(error)) => store
            .record_sync_failure(
                &detail_target_key(repository, number, PullRequestDetailSection::Reviews),
                &error.to_string(),
            )
            .await
            .map_err(|error| error.to_string()),
    }
}

fn pending_review_rest_id(
    reviews: &[PullRequestReview],
    current_user_login: Option<&str>,
    existing_pending_review: Option<&PendingReviewSession>,
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
    fn pending_review_id_matches_current_user_and_skips_existing_session() {
        let reviews = vec![
            review("review-1", "maria", PullRequestReviewState::Pending),
            review("review-2", "alex", PullRequestReviewState::Pending),
        ];
        assert_eq!(
            pending_review_rest_id(&reviews, Some("alex"), None),
            Some("review-2".to_string())
        );

        let existing = PendingReviewSession {
            node_id: "review-2-node".to_string(),
            comment_count: 2,
        };
        assert_eq!(
            pending_review_rest_id(&reviews, Some("alex"), Some(&existing)),
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
