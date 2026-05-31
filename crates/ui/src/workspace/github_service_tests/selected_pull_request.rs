use std::sync::Arc;

use gpui::TestAppContext;
use harbor_domain::{ChecksSummary, MergeState, ReviewDecision};

use crate::{test_fixtures::pull_request, workspace::github_service::test_support::FakeGitHubApi};

use super::init_workspace_service_test;

#[gpui::test]
async fn selected_metadata_refresh_does_not_refetch_files(cx: &mut TestAppContext) {
    let api = Arc::new(FakeGitHubApi::default());
    let mut updated_pull_request = pull_request();
    updated_pull_request.title = "Updated title".to_string();
    api.push_pull_request_detail(Ok(updated_pull_request.clone()));
    let (view_entity, cx) = init_workspace_service_test(cx, api.clone());

    view_entity.update(cx, |view, cx| {
        view.pull_requests = vec![pull_request()];
        view.selection_state.reset_pull_request_index();
        view.refresh_selected_pull_request_metadata_only(cx);
    });
    cx.run_until_parked();

    assert_eq!(api.calls(), vec!["get_pull_request"]);
    view_entity.read_with(cx, |view, _| {
        assert_eq!(view.pull_requests[0].title, "Updated title");
        assert!(view.detail_state.files.is_empty());
    });
}

#[gpui::test]
async fn ignores_stale_pull_request_detail_results_after_selection_changes(
    cx: &mut TestAppContext,
) {
    let api = Arc::new(FakeGitHubApi::default());
    let first_pull_request = pull_request();
    let mut stale_detail = first_pull_request.clone();
    stale_detail.title = "Stale detail".to_string();
    let mut second_pull_request = pull_request();
    second_pull_request.number = 8;
    second_pull_request.title = "Selected detail".to_string();
    second_pull_request.head_sha = "def456".to_string();
    api.push_pull_request_detail(Ok(stale_detail));
    let (view_entity, cx) = init_workspace_service_test(cx, api);

    view_entity.update(cx, |view, cx| {
        view.pull_requests = vec![first_pull_request, second_pull_request.clone()];
        view.selection_state.reset_pull_request_index();
        let generation_before = view.review_data_generation();
        view.refresh_selected_pull_request(cx);
        assert!(view.review_data_generation() > generation_before);
        view.selection_state.set_pull_request_index(1);
    });
    cx.run_until_parked();

    view_entity.read_with(cx, |view, _| {
        assert_eq!(view.selected_pull_request_index(), 1);
        assert_eq!(view.pull_requests[1].title, "Selected detail");
        assert!(view.detail_state.files.is_empty());
        assert!(view.review_state.review_threads.is_empty());
    });
}

#[gpui::test]
async fn selected_metadata_replace_preserves_cached_row_signals(cx: &mut TestAppContext) {
    let api = Arc::new(FakeGitHubApi::default());
    let mut row_pull_request = pull_request();
    row_pull_request.review_decision = Some(ReviewDecision::Approved);
    row_pull_request.merge_state = Some(MergeState::Clean);
    row_pull_request.checks_summary = ChecksSummary {
        total: 3,
        passed: 3,
        failed: 0,
        pending: 0,
        skipped: 0,
    };
    row_pull_request.unresolved_threads = 2;
    let mut metadata = row_pull_request.clone();
    metadata.title = "REST detail".to_string();
    metadata.review_decision = None;
    metadata.merge_state = Some(MergeState::Unknown);
    metadata.checks_summary = ChecksSummary::default();
    metadata.unresolved_threads = 0;
    let (view_entity, cx) = init_workspace_service_test(cx, api);

    view_entity.update(cx, |view, _| {
        view.pull_requests = vec![row_pull_request.clone()];
        view.selection_state.reset_pull_request_index();
        view.replace_selected_pull_request_preserving_row_fields(metadata);
    });

    view_entity.read_with(cx, |view, _| {
        let selected = &view.pull_requests[0];
        assert_eq!(selected.title, "REST detail");
        assert_eq!(selected.review_decision, Some(ReviewDecision::Approved));
        assert_eq!(selected.merge_state, Some(MergeState::Clean));
        assert_eq!(selected.checks_summary, row_pull_request.checks_summary);
        assert_eq!(selected.unresolved_threads, 2);
    });
}
