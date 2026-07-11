use std::sync::Arc;

use gpui::TestAppContext;
use harbor_domain::{RepoId, ReviewThreadState, Workflow, WorkflowState};
use harbor_github::ConditionalFetch;

use crate::{
    actions::PanelTab,
    test_fixtures::{patched_diff_file, pull_request, review_thread, test_time, workflow_run},
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
async fn overview_loads_review_activity_for_its_timeline(cx: &mut TestAppContext) {
    let api = Arc::new(FakeGitHubApi::default());
    let pull_request = pull_request();
    let thread = review_thread(ReviewThreadState::Unresolved);
    api.push_pull_request_detail(Ok(pull_request.clone()));
    api.push_files(Ok(vec![patched_diff_file()]));
    api.push_current_user(Ok("octocat".to_string()));
    api.push_reviews(Ok(Vec::new()));
    api.push_pull_request_comments(Ok(Vec::new()));
    api.push_review_threads(Ok(vec![thread.clone()]));
    let (view_entity, cx) = init_workspace_service_test(cx, api.clone());

    view_entity.update(cx, |view, cx| {
        view.pull_requests = vec![pull_request];
        view.active_tab = PanelTab::Overview;
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

#[gpui::test]
async fn actions_tab_loads_repository_workflows_and_runs(cx: &mut TestAppContext) {
    let api = Arc::new(FakeGitHubApi::default());
    let repository = RepoId::new("acme", "app");
    let workflow = Workflow {
        id: 9,
        name: "CI".to_string(),
        path: ".github/workflows/ci.yml".to_string(),
        state: WorkflowState::Active,
        html_url: "https://github.com/acme/app/blob/main/.github/workflows/ci.yml".to_string(),
        badge_url: None,
        created_at: test_time(),
        updated_at: test_time(),
    };
    let run = workflow_run();
    api.push_workflows(Ok(vec![workflow.clone()]));
    api.push_repository_workflow_runs(Ok(vec![run.clone()]));
    let (view_entity, cx) = init_workspace_service_test(cx, api.clone());

    view_entity.update(cx, |view, cx| {
        view.repository_state.select_repository(repository.clone());
        view.select_panel_tab(PanelTab::Actions, cx);
    });
    cx.run_until_parked();

    assert_eq!(
        api.calls(),
        vec!["list_workflows", "list_repository_workflow_runs"]
    );
    view_entity.read_with(cx, |view, _| {
        assert_eq!(view.repository_actions_state.workflows(), &[workflow]);
        assert_eq!(view.repository_actions_state.workflow_runs(), &[run]);
    });

    api.push_workflow_runs_for_workflow(Ok(Vec::new()));
    view_entity.update(cx, |view, cx| {
        view.select_repository_actions_workflow(Some(9), cx);
    });
    cx.run_until_parked();

    assert_eq!(
        api.calls(),
        vec![
            "list_workflows",
            "list_repository_workflow_runs",
            "list_workflow_runs_for_workflow"
        ]
    );
    view_entity.read_with(cx, |view, _| {
        assert_eq!(
            view.repository_actions_state.selected_workflow_id(),
            Some(9)
        );
        assert!(view.repository_actions_state.workflow_runs().is_empty());
    });
}
