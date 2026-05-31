use std::sync::Arc;

use gpui::TestAppContext;
use harbor_domain::PullRequest;
use harbor_github::{
    ConditionalFetch, PullRequestEnrichment, PullRequestPage, PullRequestPageCursor,
};

use crate::{
    test_fixtures::pull_request,
    workspace::{
        PullRequestInboxCacheKey, PullRequestInboxMode, github_service::test_support::FakeGitHubApi,
    },
};

use super::{enqueue_successful_detail_load, github_error, init_workspace_service_test};

#[gpui::test]
async fn loads_pull_request_inbox_success_from_service(cx: &mut TestAppContext) {
    let api = Arc::new(FakeGitHubApi::default());
    let pull_request = pull_request();
    api.push_light_pull_requests(Ok(ConditionalFetch::Modified {
        value: vec![pull_request.clone()],
        validator: None,
    }));
    enqueue_successful_detail_load(&api, &pull_request);
    let (view_entity, cx) = init_workspace_service_test(cx, api.clone());

    view_entity.update(cx, |view, cx| {
        view.load_pull_requests(pull_request.repo.clone(), cx);
    });
    cx.run_until_parked();

    view_entity.read_with(cx, |view, _| {
        assert_eq!(view.pull_requests.len(), 1);
        assert_eq!(view.pull_requests[0].number, pull_request.number);
        assert_eq!(view.pull_requests[0].title, pull_request.title);
        assert_eq!(view.pull_request_inbox.load_error(), None);
        assert!(!view.pull_request_inbox.is_loading());
    });
    assert_eq!(
        api.calls(),
        vec![
            "list_repository_pull_requests_light",
            "get_pull_request",
            "list_pull_request_files",
            "current_user",
            "list_pull_request_reviews",
            "list_review_threads"
        ]
    );
    assert_eq!(api.light_pull_request_requests(), vec![(None, 10, false)]);
}

#[gpui::test]
async fn load_more_pull_requests_appends_next_page(cx: &mut TestAppContext) {
    let api = Arc::new(FakeGitHubApi::default());
    let first_pull_request = pull_request();
    let mut second_pull_request = pull_request();
    second_pull_request.number = 8;
    second_pull_request.title = "Follow-up feature".to_string();
    api.push_light_pull_request_page(Ok(ConditionalFetch::Modified {
        value: PullRequestPage {
            pull_requests: vec![first_pull_request.clone()],
            total_count: Some(2),
            next_cursor: Some(PullRequestPageCursor::RestPage(2)),
        },
        validator: None,
    }));
    enqueue_successful_detail_load(&api, &first_pull_request);
    let (view_entity, cx) = init_workspace_service_test(cx, api.clone());

    view_entity.update(cx, |view, cx| {
        view.load_pull_requests(first_pull_request.repo.clone(), cx);
    });
    cx.run_until_parked();

    view_entity.read_with(cx, |view, _| {
        assert_eq!(view.pull_requests.len(), 1);
        assert!(view.pull_request_inbox.has_next_page());
    });

    api.push_light_pull_request_page(Ok(ConditionalFetch::Modified {
        value: PullRequestPage {
            pull_requests: vec![second_pull_request.clone()],
            total_count: Some(2),
            next_cursor: None,
        },
        validator: None,
    }));
    view_entity.update(cx, |view, cx| {
        view.load_more_pull_requests(cx);
    });
    cx.run_until_parked();

    view_entity.read_with(cx, |view, _| {
        assert_eq!(view.pull_requests.len(), 2);
        assert_eq!(view.pull_requests[0].number, first_pull_request.number);
        assert_eq!(view.pull_requests[1].number, second_pull_request.number);
        assert_eq!(view.pull_request_inbox.total_count(), Some(2));
        assert!(!view.pull_request_inbox.has_next_page());
        assert_eq!(view.pull_request_inbox.load_more_error(), None);
    });
}

#[gpui::test]
async fn prefetches_inactive_inbox_counts_without_loading_items(cx: &mut TestAppContext) {
    let api = Arc::new(FakeGitHubApi::default());
    let pull_request = pull_request();
    api.push_pull_request_count(Ok(4));
    api.push_pull_request_count(Ok(2));
    api.push_light_pull_requests(Ok(ConditionalFetch::Modified {
        value: vec![pull_request.clone()],
        validator: None,
    }));
    enqueue_successful_detail_load(&api, &pull_request);
    let (view_entity, cx) = init_workspace_service_test(cx, api.clone());

    view_entity.update(cx, |view, cx| {
        view.prefetch_inbox_counts = true;
        view.load_pull_requests(pull_request.repo.clone(), cx);
    });
    cx.run_until_parked();

    view_entity.read_with(cx, |view, _| {
        let closed_key =
            PullRequestInboxCacheKey::new(pull_request.repo.clone(), PullRequestInboxMode::Closed);
        let needs_review_key = PullRequestInboxCacheKey::new(
            pull_request.repo.clone(),
            PullRequestInboxMode::NeedsReview,
        );

        assert_eq!(view.pull_request_inbox.snapshot_count(&closed_key), Some(4));
        assert_eq!(
            view.pull_request_inbox.snapshot_count(&needs_review_key),
            Some(2)
        );
        assert!(view.pull_request_inbox.snapshot(&closed_key).is_none());
        assert!(
            view.pull_request_inbox
                .snapshot(&needs_review_key)
                .is_none()
        );
    });

    let calls = api.calls();
    assert_eq!(
        calls
            .iter()
            .filter(|call| call.as_str() == "count_repository_pull_requests")
            .count(),
        2
    );
    assert!(
        !calls
            .iter()
            .any(|call| call.as_str() == "list_repository_pull_requests")
    );
}

#[gpui::test]
async fn reports_pull_request_inbox_failure_from_service(cx: &mut TestAppContext) {
    let api = Arc::new(FakeGitHubApi::default());
    let pull_request = pull_request();
    api.push_light_pull_requests(Err(github_error("inbox failed")));
    let (view_entity, cx) = init_workspace_service_test(cx, api.clone());

    view_entity.update(cx, |view, cx| {
        view.load_pull_requests(pull_request.repo.clone(), cx);
    });
    cx.run_until_parked();

    view_entity.read_with(cx, |view, _| {
        assert!(view.pull_requests.is_empty());
        assert!(
            view.pull_request_inbox
                .load_error()
                .is_some_and(|error| error.contains("inbox failed"))
        );
        assert_eq!(
            view.status,
            "Failed to load open pull requests from acme/app"
        );
        assert!(!view.pull_request_inbox.is_loading());
    });
    assert_eq!(api.calls(), vec!["list_repository_pull_requests_light"]);
}

#[gpui::test]
async fn inbox_refresh_failure_keeps_existing_rows(cx: &mut TestAppContext) {
    let api = Arc::new(FakeGitHubApi::default());
    let pull_request = pull_request();
    api.push_light_pull_requests(Err(github_error("refresh failed")));
    let (view_entity, cx) = init_workspace_service_test(cx, api.clone());

    view_entity.update(cx, |view, cx| {
        view.repository_state
            .select_repository(pull_request.repo.clone());
        view.pull_request_inbox.set_mode(PullRequestInboxMode::Open);
        view.pull_requests = vec![pull_request.clone()];
        view.refresh_pull_requests(pull_request.repo.clone(), cx);
    });
    cx.run_until_parked();

    view_entity.read_with(cx, |view, _| {
        assert_eq!(view.pull_requests, vec![pull_request.clone()]);
        assert!(
            view.pull_request_inbox
                .load_error()
                .is_some_and(|error| error.contains("refresh failed"))
        );
        assert_eq!(
            view.status,
            "Failed to load open pull requests from acme/app; showing existing data"
        );
        assert!(!view.pull_request_inbox.is_loading());
    });
    assert_eq!(api.calls(), vec!["list_repository_pull_requests_light"]);
}

#[gpui::test]
async fn manual_inbox_refresh_can_force_enrichment(cx: &mut TestAppContext) {
    let api = Arc::new(FakeGitHubApi::default());
    let pull_request = pull_request();
    api.push_light_pull_requests(Ok(ConditionalFetch::Modified {
        value: vec![pull_request.clone()],
        validator: None,
    }));
    api.push_pull_request_enrichments(Ok(vec![enrichment(&pull_request)]));
    let (view_entity, cx) = init_workspace_service_test(cx, api.clone());

    view_entity.update(cx, |view, cx| {
        view.repository_state
            .select_repository(pull_request.repo.clone());
        view.pull_requests = vec![pull_request.clone()];
        view.selection_state.reset_pull_request_index();
        view.refresh_pull_requests(pull_request.repo, cx);
    });
    cx.run_until_parked();

    assert_eq!(
        api.calls(),
        vec![
            "list_repository_pull_requests_light",
            "enrich_pull_requests_by_node_ids"
        ]
    );
}

fn enrichment(pull_request: &PullRequest) -> PullRequestEnrichment {
    PullRequestEnrichment {
        node_id: pull_request.node_id.clone(),
        review_decision: pull_request.review_decision,
        merge_state: pull_request.merge_state,
        checks_summary: Default::default(),
    }
}
