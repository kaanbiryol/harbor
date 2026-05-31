use std::sync::Arc;

use chrono::{Duration, Utc};
use gpui::TestAppContext;
use harbor_github::ConditionalFetch;
use harbor_sync::{SyncState, SyncTarget};

use crate::{
    test_fixtures::pull_request,
    workspace::{PullRequestInboxMode, github_service::test_support::FakeGitHubApi},
};

use super::init_workspace_service_test;

#[gpui::test]
async fn focus_catch_up_uses_light_inbox_refresh_only(cx: &mut TestAppContext) {
    let api = Arc::new(FakeGitHubApi::default());
    let pull_request = pull_request();
    api.push_light_pull_requests(Ok(ConditionalFetch::Modified {
        value: vec![pull_request.clone()],
        validator: None,
    }));
    let (view_entity, cx) = init_workspace_service_test(cx, api.clone());

    view_entity.update(cx, |view, cx| {
        view.repository_state
            .select_repository(pull_request.repo.clone());
        view.pull_requests = vec![pull_request];
        view.selection_state.reset_pull_request_index();
        view.sync_runtime.set_sync_state(
            SyncTarget::ActiveInboxLight,
            SyncState {
                last_successful_fetch_at: Some(Utc::now() - Duration::seconds(301)),
                ..Default::default()
            },
        );
        view.catch_up_active_inbox_after_focus(cx);
    });
    cx.run_until_parked();

    assert_eq!(api.calls(), vec!["list_repository_pull_requests_light"]);
}

#[gpui::test]
async fn focus_catch_up_before_threshold_does_not_refresh(cx: &mut TestAppContext) {
    let api = Arc::new(FakeGitHubApi::default());
    let pull_request = pull_request();
    let (view_entity, cx) = init_workspace_service_test(cx, api.clone());

    view_entity.update(cx, |view, cx| {
        view.repository_state
            .select_repository(pull_request.repo.clone());
        view.sync_runtime.set_sync_state(
            SyncTarget::ActiveInboxLight,
            SyncState {
                last_successful_fetch_at: Some(Utc::now()),
                ..Default::default()
            },
        );
        view.catch_up_active_inbox_after_focus(cx);
    });
    cx.run_until_parked();

    assert!(api.calls().is_empty());
}

#[gpui::test]
async fn focus_catch_up_does_not_run_needs_review_before_shared_cadence(cx: &mut TestAppContext) {
    let api = Arc::new(FakeGitHubApi::default());
    let pull_request = pull_request();
    let (view_entity, cx) = init_workspace_service_test(cx, api.clone());

    view_entity.update(cx, |view, cx| {
        view.repository_state
            .select_repository(pull_request.repo.clone());
        view.pull_request_inbox
            .set_mode(PullRequestInboxMode::NeedsReview);
        view.sync_runtime.set_sync_state(
            SyncTarget::ActiveInbox,
            SyncState {
                last_successful_fetch_at: Some(Utc::now() - Duration::seconds(31)),
                ..Default::default()
            },
        );
        view.catch_up_active_inbox_after_focus(cx);
    });
    cx.run_until_parked();

    assert!(api.calls().is_empty());
}

#[gpui::test]
async fn active_inbox_stale_marks_current_mode_target(cx: &mut TestAppContext) {
    let api = Arc::new(FakeGitHubApi::default());
    let (view_entity, cx) = init_workspace_service_test(cx, api);

    view_entity.update(cx, |view, _| {
        view.pull_request_inbox.set_mode(PullRequestInboxMode::Open);
        view.mark_active_inbox_stale();
        assert!(
            view.sync_runtime
                .sync_state(SyncTarget::ActiveInboxLight)
                .is_some_and(|state| state.stale)
        );
        assert!(
            !view
                .sync_runtime
                .sync_state(SyncTarget::ActiveInbox)
                .is_some_and(|state| state.stale)
        );

        view.pull_request_inbox
            .set_mode(PullRequestInboxMode::NeedsReview);
        view.mark_active_inbox_stale();
        assert!(
            view.sync_runtime
                .sync_state(SyncTarget::ActiveInbox)
                .is_some_and(|state| state.stale)
        );
    });
}
