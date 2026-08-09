use std::{sync::Arc, time::Duration};

use gpui::TestAppContext;
use harbor_github::{PullRequestListFilter, PullRequestPage, PullRequestPageCursor};

use crate::{
    test_fixtures::pull_request,
    workspace::{PullRequestInboxMode, github_service::test_support::FakeGitHubApi},
};

use super::init_workspace_service_test;

#[gpui::test]
async fn searches_github_after_debounce_without_replacing_loaded_inbox(cx: &mut TestAppContext) {
    let api = Arc::new(FakeGitHubApi::default());
    let loaded_pull_request = pull_request();
    let mut search_result = pull_request();
    search_result.number = 99;
    search_result.title = "Authentication hardening".to_string();
    api.push_pull_request_search_page(Ok(PullRequestPage {
        pull_requests: vec![search_result.clone()],
        total_count: Some(1),
        next_cursor: None,
    }));
    let (view, cx) = init_workspace_service_test(cx, api.clone());

    view.update_in(cx, |view, window, cx| {
        view.repository_state
            .select_repository(loaded_pull_request.repo.clone());
        view.pull_requests = vec![loaded_pull_request.clone()];
        view.pull_request_search_input.update(cx, |input, cx| {
            input.set_value("authentication", window, cx);
        });
        view.schedule_pull_request_search(cx);
    });
    cx.run_until_parked();
    assert!(api.calls().is_empty());
    view.read_with(cx, |view, cx| {
        assert!(view.pull_request_switcher_results(cx).is_empty());
        assert_eq!(view.pull_requests, vec![loaded_pull_request.clone()]);
    });

    cx.executor().advance_clock(Duration::from_millis(250));
    cx.run_until_parked();

    view.read_with(cx, |view, cx| {
        assert_eq!(view.pull_requests, vec![loaded_pull_request]);
        assert_eq!(view.pull_request_search_state.results(), &[search_result]);
        assert_eq!(view.pull_request_search_state.total_count(), Some(1));
        assert!(!view.pull_request_search_state.is_loading());
        assert_eq!(view.pull_request_switcher_results(cx).len(), 1);
    });
    assert_eq!(api.calls(), vec!["search_repository_pull_requests"]);
    assert_eq!(
        api.pull_request_search_requests(),
        vec![(
            PullRequestListFilter::Open,
            "authentication".to_string(),
            None,
            25,
        )]
    );
}

#[gpui::test]
async fn changing_search_query_cancels_the_superseded_request(cx: &mut TestAppContext) {
    let api = Arc::new(FakeGitHubApi::default());
    api.push_pull_request_search_page(Ok(PullRequestPage {
        pull_requests: Vec::new(),
        total_count: Some(0),
        next_cursor: None,
    }));
    let pull_request = pull_request();
    let (view, cx) = init_workspace_service_test(cx, api.clone());

    view.update_in(cx, |view, window, cx| {
        view.repository_state
            .select_repository(pull_request.repo.clone());
        view.pull_request_search_input.update(cx, |input, cx| {
            input.set_value("auth", window, cx);
        });
        view.schedule_pull_request_search(cx);
        view.pull_request_search_input.update(cx, |input, cx| {
            input.set_value("authentication", window, cx);
        });
        view.schedule_pull_request_search(cx);
    });
    cx.run_until_parked();
    cx.executor().advance_clock(Duration::from_millis(250));
    cx.run_until_parked();

    assert_eq!(api.calls(), vec!["search_repository_pull_requests"]);
    assert_eq!(
        api.pull_request_search_requests()[0].1,
        "authentication".to_string()
    );
}

#[gpui::test]
async fn paginates_github_pull_request_search_results(cx: &mut TestAppContext) {
    let api = Arc::new(FakeGitHubApi::default());
    let first = pull_request();
    let mut second = pull_request();
    second.number = 8;
    api.push_pull_request_search_page(Ok(PullRequestPage {
        pull_requests: vec![first.clone()],
        total_count: Some(2),
        next_cursor: Some(PullRequestPageCursor::GraphQl("next".to_string())),
    }));
    api.push_pull_request_search_page(Ok(PullRequestPage {
        pull_requests: vec![second.clone()],
        total_count: Some(2),
        next_cursor: None,
    }));
    let (view, cx) = init_workspace_service_test(cx, api.clone());

    view.update_in(cx, |view, window, cx| {
        view.repository_state.select_repository(first.repo.clone());
        view.pull_request_inbox
            .set_mode(PullRequestInboxMode::Closed);
        view.pull_request_search_input.update(cx, |input, cx| {
            input.set_value("feature", window, cx);
        });
        view.schedule_pull_request_search(cx);
    });
    cx.run_until_parked();
    cx.executor().advance_clock(Duration::from_millis(250));
    cx.run_until_parked();
    view.update(cx, |view, cx| {
        view.load_more_pull_request_search_results(cx);
    });
    cx.run_until_parked();

    view.read_with(cx, |view, _| {
        assert_eq!(view.pull_request_search_state.results(), &[first, second]);
        assert!(view.pull_request_search_state.next_cursor().is_none());
    });
    assert_eq!(
        api.pull_request_search_requests(),
        vec![
            (
                PullRequestListFilter::Closed,
                "feature".to_string(),
                None,
                25,
            ),
            (
                PullRequestListFilter::Closed,
                "feature".to_string(),
                Some(PullRequestPageCursor::GraphQl("next".to_string())),
                25,
            ),
        ]
    );
}
