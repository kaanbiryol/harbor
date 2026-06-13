use std::sync::Arc;

use gpui::TestAppContext;

use crate::{
    actions::{PullRequestAction, WorkflowAction},
    test_fixtures::{pull_request, workflow_run},
    workspace::github_service::test_support::FakeGitHubApi,
};

use super::{github_error, init_workspace_service_test};

#[gpui::test]
async fn workflow_action_reports_success_and_failure_from_service(cx: &mut TestAppContext) {
    let success_api = Arc::new(FakeGitHubApi::default());
    success_api.push_dispatch_workflow(Ok(()));
    let (success_view, cx) = init_workspace_service_test(cx, success_api.clone());

    success_view.update(cx, |view, cx| {
        view.pull_requests = vec![pull_request()];
        view.selection_state.reset_pull_request_index();
        view.detail_state
            .replace_workflow_runs(vec![workflow_run()]);
        view.run_workflow_action(WorkflowAction::DispatchBuild, cx);
        assert!(view.action_runtime.workflow_action_running());
        assert_eq!(view.status, "Dispatching CI on feature");
        view.pull_requests.clear();
    });
    cx.run_until_parked();

    success_view.read_with(cx, |view, _| {
        assert!(!view.action_runtime.workflow_action_running());
        assert_eq!(view.action_runtime.workflow_action_error(), None);
        assert_eq!(view.status, "Dispatched CI on feature");
    });
    assert_eq!(success_api.calls(), vec!["dispatch_workflow"]);

    let failure_api = Arc::new(FakeGitHubApi::default());
    failure_api.push_dispatch_workflow(Err(github_error("dispatch failed")));
    let (failure_view, cx) = init_workspace_service_test(cx, failure_api);

    failure_view.update(cx, |view, cx| {
        view.pull_requests = vec![pull_request()];
        view.selection_state.reset_pull_request_index();
        view.detail_state
            .replace_workflow_runs(vec![workflow_run()]);
        view.run_workflow_action(WorkflowAction::DispatchBuild, cx);
        assert_eq!(view.status, "Dispatching CI on feature");
    });
    cx.run_until_parked();

    failure_view.read_with(cx, |view, _| {
        assert!(!view.action_runtime.workflow_action_running());
        assert!(
            view.action_runtime
                .workflow_action_error()
                .is_some_and(|error| error.contains("Failed to dispatch workflow"))
        );
        assert!(view.status.contains("dispatch failed"));
    });
}

#[gpui::test]
async fn pull_request_action_reports_success_and_failure_from_service(cx: &mut TestAppContext) {
    let success_api = Arc::new(FakeGitHubApi::default());
    success_api.push_approve_pull_request(Ok(()));
    let (success_view, cx) = init_workspace_service_test(cx, success_api.clone());

    success_view.update_in(cx, |view, window, cx| {
        view.pull_requests = vec![pull_request()];
        view.selection_state.reset_pull_request_index();
        view.run_pull_request_action(PullRequestAction::Approve, window, cx);
        assert!(view.action_runtime.pull_request_action_running());
        assert_eq!(view.status, "Approving PR #7");
    });
    cx.run_until_parked();

    success_view.read_with(cx, |view, _| {
        assert!(!view.action_runtime.pull_request_action_running());
        assert_eq!(view.action_runtime.pull_request_action_error(), None);
        assert_eq!(view.status, "Approved PR #7");
    });
    assert_eq!(success_api.calls(), vec!["approve_pull_request"]);

    let failure_api = Arc::new(FakeGitHubApi::default());
    failure_api.push_approve_pull_request(Err(github_error("approval failed")));
    let (failure_view, cx) = init_workspace_service_test(cx, failure_api);

    failure_view.update_in(cx, |view, window, cx| {
        view.pull_requests = vec![pull_request()];
        view.selection_state.reset_pull_request_index();
        view.run_pull_request_action(PullRequestAction::Approve, window, cx);
        assert_eq!(view.status, "Approving PR #7");
    });
    cx.run_until_parked();

    failure_view.read_with(cx, |view, _| {
        assert!(!view.action_runtime.pull_request_action_running());
        assert!(
            view.action_runtime
                .pull_request_action_error()
                .is_some_and(|error| error.contains("Failed to approve pull request"))
        );
        assert!(view.status.contains("approval failed"));
    });
}
