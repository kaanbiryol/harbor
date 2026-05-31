use harbor_domain::{PullRequestReview, PullRequestReviewState};

use crate::workspace::PendingReviewSession;

pub(super) fn pending_review_rest_id(
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
        let existing_pending_review = PendingReviewSession {
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
