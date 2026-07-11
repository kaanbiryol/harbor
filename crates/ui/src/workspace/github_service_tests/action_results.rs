use std::sync::Arc;

use gpui::TestAppContext;
use harbor_domain::PullRequestComment;

use crate::{
    actions::{PullRequestAction, PullRequestMetadataField, WorkflowAction},
    test_fixtures::{pull_request, workflow_run},
    workspace::github_service::test_support::FakeGitHubApi,
};

use super::{github_error, init_workspace_service_test};

#[gpui::test]
async fn pull_request_description_edit_preserves_failed_draft(cx: &mut TestAppContext) {
    let success_api = Arc::new(FakeGitHubApi::default());
    success_api.push_update_pull_request_body(Ok(()));
    let (success_view, cx) = init_workspace_service_test(cx, success_api.clone());

    success_view.update_in(cx, |view, window, cx| {
        let mut pull_request = pull_request();
        pull_request.body = Some("Old description".to_string());
        view.pull_requests = vec![pull_request];
        view.selection_state.reset_pull_request_index();
        view.start_pull_request_description_edit(window, cx);
        view.pull_request_description_input.update(cx, |input, cx| {
            input.set_value("Updated description", window, cx);
        });
        view.save_pull_request_description(window, cx);
    });
    cx.run_until_parked();

    success_view.read_with(cx, |view, _| {
        assert_eq!(
            view.pull_requests[0].body.as_deref(),
            Some("Updated description")
        );
        assert!(!view.pull_request_description_editing);
        assert_eq!(
            view.action_runtime.pull_request_description_action_error(),
            None
        );
    });
    assert_eq!(success_api.calls(), vec!["update_pull_request_body"]);

    let failure_api = Arc::new(FakeGitHubApi::default());
    failure_api.push_update_pull_request_body(Err(github_error("permission denied")));
    let (failure_view, cx) = init_workspace_service_test(cx, failure_api);

    failure_view.update_in(cx, |view, window, cx| {
        view.pull_requests = vec![pull_request()];
        view.selection_state.reset_pull_request_index();
        view.start_pull_request_description_edit(window, cx);
        view.pull_request_description_input.update(cx, |input, cx| {
            input.set_value("Keep this draft", window, cx);
        });
        view.save_pull_request_description(window, cx);
    });
    cx.run_until_parked();

    failure_view.read_with(cx, |view, cx| {
        assert!(view.pull_request_description_editing);
        assert_eq!(
            view.pull_request_description_input
                .read(cx)
                .value()
                .as_ref(),
            "Keep this draft"
        );
        assert!(
            view.action_runtime
                .pull_request_description_action_error()
                .is_some_and(|error| error.contains("permission denied"))
        );
    });
}

#[gpui::test]
async fn pull_request_metadata_actions_update_people_and_labels(cx: &mut TestAppContext) {
    let api = Arc::new(FakeGitHubApi::default());
    api.push_request_pull_request_reviewer(Ok(()));
    api.push_add_pull_request_assignee(Ok(()));
    api.push_add_pull_request_label(Ok(()));
    let (view, cx) = init_workspace_service_test(cx, api.clone());

    view.update_in(cx, |view, window, cx| {
        view.pull_requests = vec![pull_request()];
        view.selection_state.reset_pull_request_index();
        view.pull_request_reviewer_input
            .update(cx, |input, cx| input.set_value("reviewer", window, cx));
        view.add_pull_request_metadata(PullRequestMetadataField::Reviewer, window, cx);
    });
    cx.run_until_parked();

    view.update_in(cx, |view, window, cx| {
        view.pull_request_assignee_input
            .update(cx, |input, cx| input.set_value("assignee", window, cx));
        view.add_pull_request_metadata(PullRequestMetadataField::Assignee, window, cx);
    });
    cx.run_until_parked();

    view.update_in(cx, |view, window, cx| {
        view.pull_request_label_input
            .update(cx, |input, cx| input.set_value("needs review", window, cx));
        view.add_pull_request_metadata(PullRequestMetadataField::Label, window, cx);
    });
    cx.run_until_parked();

    view.read_with(cx, |view, cx| {
        let pull_request = &view.pull_requests[0];
        assert_eq!(pull_request.requested_reviewers[0].login, "reviewer");
        assert_eq!(pull_request.assignees[0].login, "assignee");
        assert_eq!(pull_request.labels[0].name, "needs review");
        assert!(view.pull_request_reviewer_input.read(cx).value().is_empty());
        assert!(view.pull_request_assignee_input.read(cx).value().is_empty());
        assert!(view.pull_request_label_input.read(cx).value().is_empty());
    });
    assert_eq!(
        api.calls(),
        vec![
            "request_pull_request_reviewer",
            "add_pull_request_assignee",
            "add_pull_request_label",
        ]
    );
}

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
async fn pull_request_comment_action_posts_and_refreshes_review_data(cx: &mut TestAppContext) {
    let api = Arc::new(FakeGitHubApi::default());
    api.push_create_pull_request_comment(Ok(()));
    api.push_current_user(Ok("octocat".to_string()));
    api.push_reviews(Ok(Vec::new()));
    api.push_pull_request_comments(Ok(Vec::<PullRequestComment>::new()));
    api.push_review_threads(Ok(Vec::new()));
    let (view, cx) = init_workspace_service_test(cx, api.clone());

    view.update_in(cx, |view, window, cx| {
        view.pull_requests = vec![pull_request()];
        view.selection_state.reset_pull_request_index();
        view.overview_comment_input.update(cx, |input, cx| {
            input.set_value("Looks ready to me.", window, cx);
        });
        view.submit_overview_comment(window, cx);
        assert!(view.action_runtime.pull_request_action_running());
        assert_eq!(view.status, "Posting comment on PR #7");
        assert!(view.overview_comment_input.read(cx).value().is_empty());
    });
    cx.run_until_parked();

    view.read_with(cx, |view, _| {
        assert!(!view.action_runtime.pull_request_action_running());
        assert_eq!(view.action_runtime.pull_request_action_error(), None);
    });
    assert_eq!(
        api.calls(),
        vec![
            "create_pull_request_comment",
            "current_user",
            "list_pull_request_reviews",
            "list_pull_request_comments",
            "list_review_threads",
        ]
    );
}

#[gpui::test]
async fn pull_request_action_reports_success_and_failure_from_service(cx: &mut TestAppContext) {
    let success_api = Arc::new(FakeGitHubApi::default());
    success_api.push_approve_pull_request(Ok(()));
    let (success_view, cx) = init_workspace_service_test(cx, success_api.clone());

    success_view.update_in(cx, |view, window, cx| {
        view.pull_requests = vec![pull_request()];
        view.selection_state.reset_pull_request_index();
        view.run_pull_request_action(PullRequestAction::Approve { body: None }, window, cx);
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
        view.run_pull_request_action(PullRequestAction::Approve { body: None }, window, cx);
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
