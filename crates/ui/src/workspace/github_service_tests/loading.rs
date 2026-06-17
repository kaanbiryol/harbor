use std::sync::Arc;

use gpui::TestAppContext;
use harbor_domain::{RepoId, ReviewThreadState};
use harbor_github::ConditionalFetch;

use crate::{
    actions::PanelTab,
    test_fixtures::{patched_diff_file, pull_request, review_thread},
    workspace::github_service::test_support::FakeGitHubApi,
};

use super::{enqueue_successful_detail_load, init_workspace_service_test};

#[gpui::test]
async fn loads_diff_review_threads_and_defers_other_panel_fetches(cx: &mut TestAppContext) {
    let api = Arc::new(FakeGitHubApi::default());
    let pull_request = pull_request();
    let thread = review_thread(ReviewThreadState::Unresolved);
    api.push_pull_request_detail(Ok(pull_request.clone()));
    api.push_files(Ok(vec![patched_diff_file()]));
    api.push_current_user(Ok("octocat".to_string()));
    api.push_reviews(Ok(Vec::new()));
    api.push_pull_request_comments(Ok(Vec::new()));
    api.push_review_threads(Ok(vec![thread.clone()]));
    api.push_check_runs(Ok(Vec::new()));
    let (view_entity, cx) = init_workspace_service_test(cx, api.clone());

    view_entity.update(cx, |view, cx| {
        view.pull_requests = vec![pull_request];
        view.selection_state.reset_pull_request_index();
        view.load_selected_pull_request(cx);
    });
    cx.run_until_parked();

    assert_eq!(
        api.calls(),
        vec![
            "get_pull_request",
            "list_pull_request_files",
            "current_user",
            "list_pull_request_reviews",
            "list_pull_request_comments",
            "list_review_threads"
        ]
    );
    view_entity.read_with(cx, |view, _| {
        assert_eq!(view.review_state.review_threads, vec![thread]);
    });

    view_entity.update(cx, |view, cx| {
        view.select_panel_tab(PanelTab::Checks, cx);
    });
    cx.run_until_parked();

    assert_eq!(
        api.calls(),
        vec![
            "get_pull_request",
            "list_pull_request_files",
            "current_user",
            "list_pull_request_reviews",
            "list_pull_request_comments",
            "list_review_threads",
            "list_check_runs"
        ]
    );

    view_entity.update(cx, |view, cx| {
        view.select_panel_tab(PanelTab::Diff, cx);
        view.select_panel_tab(PanelTab::Checks, cx);
    });
    cx.run_until_parked();

    assert_eq!(
        api.calls(),
        vec![
            "get_pull_request",
            "list_pull_request_files",
            "current_user",
            "list_pull_request_reviews",
            "list_pull_request_comments",
            "list_review_threads",
            "list_check_runs"
        ]
    );
}

#[gpui::test]
async fn typed_repository_lookup_loads_pull_requests_after_validation(cx: &mut TestAppContext) {
    let api = Arc::new(FakeGitHubApi::default());
    let repository = RepoId::new("acme", "app");
    let pull_request = pull_request();
    api.push_repository_lookup(Ok(repository.clone()));
    api.push_light_pull_requests(Ok(ConditionalFetch::Modified {
        value: vec![pull_request.clone()],
        validator: None,
    }));
    enqueue_successful_detail_load(&api, &pull_request);
    let (view_entity, cx) = init_workspace_service_test(cx, api.clone());

    view_entity.update(cx, |view, cx| {
        view.open_typed_repository_from_switcher(repository.clone(), cx);
    });
    cx.run_until_parked();

    view_entity.read_with(cx, |view, _| {
        assert_eq!(view.current_repository(), Some(&repository));
        assert_eq!(view.pull_requests.len(), 1);
        assert!(!view.repository_state.is_loading());
    });
    assert_eq!(
        api.calls(),
        vec![
            "get_repository",
            "list_repository_pull_requests_light",
            "get_pull_request",
            "list_pull_request_files",
            "current_user",
            "list_pull_request_reviews",
            "list_pull_request_comments",
            "list_review_threads"
        ]
    );
}
