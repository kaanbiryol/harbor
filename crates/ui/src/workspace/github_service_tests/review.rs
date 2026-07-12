use std::sync::Arc;

use gpui::TestAppContext;
use harbor_domain::{PullRequestReview, PullRequestReviewState, ReviewThreadState};

use crate::{
    test_fixtures::{pull_request, review_thread, test_time},
    workspace::github_service::test_support::FakeGitHubApi,
};

use super::{github_error, init_workspace_service_test};

#[gpui::test]
async fn refresh_review_data_keeps_reviews_when_threads_fail(cx: &mut TestAppContext) {
    let api = Arc::new(FakeGitHubApi::default());
    let pull_request = pull_request();
    let review = pull_request_review("review-1", PullRequestReviewState::Approved);
    api.push_current_user(Ok("octocat".to_string()));
    api.push_reviews(Ok(vec![review.clone()]));
    api.push_pull_request_comments(Ok(Vec::new()));
    api.push_review_threads(Err(github_error("threads failed")));
    let (view_entity, cx) = init_workspace_service_test(cx, api);

    view_entity.update(cx, |view, cx| {
        view.pull_requests = vec![pull_request];
        view.selection_state.reset_pull_request_index();
        view.load_selected_review_data(cx);
    });
    cx.run_until_parked();

    view_entity.read_with(cx, |view, _| {
        assert_eq!(view.review_state.pull_request_reviews(), &[review]);
        assert!(view.review_state.review_threads().is_empty());
        assert!(
            view.review_state
                .reviews_error()
                .is_some_and(|error| error.contains("Failed to load review threads"))
        );
        assert_eq!(
            view.status,
            "Refreshed review history for PR #7, but threads failed"
        );
    });
}

fn pull_request_review(id: &str, state: PullRequestReviewState) -> PullRequestReview {
    PullRequestReview {
        id: id.to_string(),
        node_id: Some(format!("{id}-node")),
        author: "octocat".to_string(),
        state,
        body: None,
        submitted_at: Some(test_time()),
    }
}

#[gpui::test]
async fn refresh_review_data_keeps_threads_when_reviews_fail(cx: &mut TestAppContext) {
    let api = Arc::new(FakeGitHubApi::default());
    let pull_request = pull_request();
    let thread = review_thread(ReviewThreadState::Unresolved);
    api.push_current_user(Ok("octocat".to_string()));
    api.push_reviews(Err(github_error("reviews failed")));
    api.push_pull_request_comments(Ok(Vec::new()));
    api.push_review_threads(Ok(vec![thread.clone()]));
    let (view_entity, cx) = init_workspace_service_test(cx, api);

    view_entity.update(cx, |view, cx| {
        view.pull_requests = vec![pull_request];
        view.selection_state.reset_pull_request_index();
        view.load_selected_review_data(cx);
    });
    cx.run_until_parked();

    view_entity.read_with(cx, |view, _| {
        assert!(view.review_state.pull_request_reviews().is_empty());
        assert_eq!(view.review_state.review_threads(), &[thread]);
        assert!(
            view.review_state
                .reviews_error()
                .is_some_and(|error| error.contains("Failed to load review history"))
        );
        assert_eq!(
            view.status,
            "Refreshed 1 review threads for PR #7, but review history failed"
        );
    });
}
