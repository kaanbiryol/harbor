use std::sync::Arc;

use gpui::TestAppContext;
use harbor_domain::{
    MergeQueueState, MergeState, PullRequestStack, PullRequestStackMembership, StackMergeOutcome,
};

use super::{github_error, init_workspace_service_test};
use crate::{
    actions::{PullRequestAction, SelectStackDown, SelectStackUp},
    test_fixtures::pull_request,
    workspace::test_support::FakeGitHubApi,
};

fn stack() -> PullRequestStack {
    let mut bottom = pull_request();
    bottom.number = 7;
    bottom.merge_state = Some(MergeState::Clean);
    bottom.merge_capabilities.queue_state = MergeQueueState::Enabled;
    bottom.merge_capabilities.viewer_can_queue = true;
    bottom.stack = Some(PullRequestStackMembership {
        number: 9,
        position: 1,
        size: 2,
        base_ref: "main".into(),
    });
    let mut top = bottom.clone();
    top.number = 8;
    top.stack.as_mut().unwrap().position = 2;
    PullRequestStack {
        number: 9,
        base_ref: "main".into(),
        pull_requests: vec![bottom, top],
    }
}

#[gpui::test]
async fn metadata_loads_native_stack_and_navigation_can_open_a_member_outside_the_inbox(
    cx: &mut TestAppContext,
) {
    let stack = stack();
    let api = Arc::new(FakeGitHubApi::default());
    api.push_pull_request_detail(Ok(stack.pull_requests[1].clone()));
    api.push_stack(Ok(stack.clone()));
    let (entity, cx) = init_workspace_service_test(cx, api.clone());
    entity.update(cx, |view, cx| {
        view.pull_requests = vec![stack.pull_requests[1].clone()];
        view.refresh_selected_pull_request_metadata_only(cx);
    });
    cx.run_until_parked();
    entity.read_with(cx, |view, _| {
        assert_eq!(view.detail_state.stack.as_deref(), Some(&stack));
        assert!(view.detail_state.stack_load.is_loaded());
        assert!(
            view.pull_request_action_request(PullRequestAction::MergeWhenReady)
                .unwrap_err()
                .contains("stack")
        );
    });
    api.push_pull_request_detail(Ok(stack.pull_requests[0].clone()));
    api.push_stack(Ok(stack.clone()));
    entity.update_in(cx, |view, window, cx| {
        view.select_stack_down(&SelectStackDown, window, cx);
    });
    cx.run_until_parked();
    entity.read_with(cx, |view, _| {
        assert_eq!(view.selected_pull_request().unwrap().number, 7);
        assert_eq!(view.pull_requests.len(), 2);
        assert_eq!(view.detail_state.stack.as_deref(), Some(&stack));
    });
    entity.update_in(cx, |view, window, cx| {
        view.select_stack_up(&SelectStackUp, window, cx);
    });
    entity.read_with(cx, |view, _| {
        assert_eq!(view.selected_pull_request().unwrap().number, 8)
    });
}

#[gpui::test]
async fn stack_load_results_do_not_leak_to_another_selection(cx: &mut TestAppContext) {
    let stack = stack();
    let api = Arc::new(FakeGitHubApi::default());
    let pending = api.pending_stack();
    let (entity, cx) = init_workspace_service_test(cx, api);
    entity.update(cx, |view, cx| {
        view.pull_requests = vec![stack.pull_requests[1].clone(), pull_request()];
        view.load_selected_pull_request_stack(cx);
    });
    cx.run_until_parked();
    entity.update(cx, |view, cx| {
        view.select_pull_request(1, cx);
    });
    cx.run_until_parked();
    assert!(pending.send(Ok(stack)).is_err());
    entity.read_with(cx, |view, _| assert!(view.detail_state.stack.is_none()));
}

#[gpui::test]
async fn enqueue_stack_reports_confirmation_and_background_failures(cx: &mut TestAppContext) {
    for result in [
        Ok(StackMergeOutcome::Enqueued),
        Err(github_error("requires approval")),
    ] {
        let api = Arc::new(FakeGitHubApi::default());
        let should_fail = result.is_err();
        api.push_stack_merge(result);
        let (entity, cx) = init_workspace_service_test(cx, api.clone());
        let stack = stack();
        let expected_repository = stack.pull_requests[0].repo.clone();
        let expected_head_sha = stack.pull_requests[1].head_sha.clone();
        entity.update(cx, |view, cx| {
            view.pull_requests = vec![stack.pull_requests[0].clone()];
            view.detail_state.stack = Some(Arc::new(stack));
            view.detail_state.stack_load.succeed();
            view.run_stack_merge(cx);
            assert!(view.action_runtime.pull_request_action_running());
        });
        cx.run_until_parked();
        assert!(
            api.calls()
                .contains(&"merge_pull_request_stack".to_string())
        );
        assert!(
            !api.calls()
                .contains(&"merge_pull_request_when_ready".to_string())
        );
        assert_eq!(
            api.stack_merge_requests(),
            vec![(
                expected_repository,
                8,
                expected_head_sha,
                harbor_domain::StackMergeAction::Enqueue
            )]
        );
        entity.read_with(cx, |view, _| {
            assert!(!view.action_runtime.pull_request_action_running());
            assert_eq!(
                view.action_runtime.pull_request_action_error().is_some(),
                should_fail
            );
            if let Some(error) = view.action_runtime.pull_request_action_error() {
                assert!(error.contains("requires approval"));
            }
        });
    }
}

#[gpui::test]
async fn conflicts_in_a_lower_layer_prevent_enqueueing(cx: &mut TestAppContext) {
    let api = Arc::new(FakeGitHubApi::default());
    let (entity, cx) = init_workspace_service_test(cx, api.clone());
    let mut stack = stack();
    stack.pull_requests[0].merge_state = Some(MergeState::Dirty);
    entity.update(cx, |view, cx| {
        view.pull_requests = vec![stack.pull_requests[1].clone()];
        view.detail_state.stack = Some(Arc::new(stack));
        view.run_stack_merge(cx);
        assert!(
            view.action_runtime
                .pull_request_action_error()
                .unwrap()
                .contains("#7 has merge conflicts")
        );
    });
    assert!(
        !api.calls()
            .contains(&"merge_pull_request_stack".to_string())
    );
}
