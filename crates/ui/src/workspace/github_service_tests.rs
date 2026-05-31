use std::sync::Arc;

use chrono::{Duration, Utc};
use gpui::{
    AppContext, Context, Entity, IntoElement, Render, TestAppContext, VisualTestContext, Window,
    div,
};
use gpui_component::{Root, Theme, ThemeMode};
use harbor_domain::{
    ChecksSummary, DiffFile, FileStatus, MergeState, PullRequest, PullRequestReview,
    PullRequestReviewState, RepoId, ReviewDecision, ReviewThreadState, WorkflowConclusion,
    WorkflowRun, WorkflowStatus,
};
use harbor_github::{
    ConditionalFetch, GitHubError, PullRequestEnrichment, PullRequestPage, PullRequestPageCursor,
};
use harbor_sync::{SyncState, SyncTarget};

use crate::{
    actions::{PanelTab, PullRequestAction, WorkflowAction},
    test_fixtures::{diff_file, pull_request, review_thread, test_time},
    workspace::{
        AppView, GitHubAuthSource, GitHubAuthStatus, PullRequestInboxCacheKey,
        PullRequestInboxMode,
        github_service::{GitHubAuthApi, test_support::FakeGitHubApi},
    },
};

#[test]
fn fake_github_api_records_auth_source_calls() {
    let api = FakeGitHubApi::default();

    api.configure_token("oauth-token".to_string(), GitHubAuthSource::OAuth)
        .expect("fake oauth token auth should succeed");
    api.configure_gh_cli()
        .expect("fake github cli auth should succeed");
    api.clear_auth().expect("fake auth clear should succeed");

    assert_eq!(
        api.calls(),
        vec!["configure_oauth_token", "configure_gh_cli", "clear_auth",]
    );
}

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
async fn loads_diff_review_threads_and_defers_other_panel_fetches(cx: &mut TestAppContext) {
    let api = Arc::new(FakeGitHubApi::default());
    let pull_request = pull_request();
    let thread = review_thread(ReviewThreadState::Unresolved);
    api.push_pull_request_detail(Ok(pull_request.clone()));
    api.push_files(Ok(vec![test_diff_file()]));
    api.push_current_user(Ok("octocat".to_string()));
    api.push_reviews(Ok(Vec::new()));
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
            "list_review_threads"
        ]
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

#[gpui::test]
async fn cached_detail_restore_preserves_diff_position_without_refetch(cx: &mut TestAppContext) {
    let api = Arc::new(FakeGitHubApi::default());
    let (view_entity, cx) = init_workspace_service_test(cx, api.clone());

    view_entity.update(cx, |view, cx| {
        view.pull_requests = vec![pull_request()];
        view.selection_state.reset_pull_request_index();
        view.detail_state.files = vec![
            diff_file("src/a.rs", FileStatus::Modified),
            diff_file("src/b.rs", FileStatus::Modified),
        ];
        view.detail_state.diffs = vec![None, None];
        mark_detail_sections_loaded(view);
        view.selection_state.set_diff_position(1, 4);
        view.active_tab = PanelTab::Diff;
        view.cache_current_pull_request_detail_snapshot();

        view.detail_state.files = vec![diff_file("src/other.rs", FileStatus::Modified)];
        view.detail_state.diffs = vec![None];
        view.selection_state.set_diff_position(0, 0);
        view.active_tab = PanelTab::Review;

        assert!(view.restore_selected_pull_request_detail_snapshot(cx));
        assert_eq!(
            view.detail_state
                .files
                .iter()
                .map(|file| file.path.as_str())
                .collect::<Vec<_>>(),
            vec!["src/a.rs", "src/b.rs"]
        );
        assert_eq!(view.active_file_index(), 1);
        assert_eq!(view.active_hunk_index(), 4);
        assert_eq!(view.active_tab, PanelTab::Diff);
        assert_eq!(view.status, "Showing cached PR #7 details");
    });
    cx.run_until_parked();

    assert!(api.calls().is_empty());
}

#[gpui::test]
async fn cached_inbox_restore_bounds_stale_selection_without_refetch(cx: &mut TestAppContext) {
    let api = Arc::new(FakeGitHubApi::default());
    let pull_request = pull_request();
    let (view_entity, cx) = init_workspace_service_test(cx, api.clone());

    view_entity.update(cx, |view, cx| {
        view.repository_state
            .select_repository(pull_request.repo.clone());
        view.pull_request_inbox.set_mode(PullRequestInboxMode::Open);
        view.pull_requests = vec![pull_request.clone()];
        view.detail_state.files = vec![test_diff_file()];
        view.detail_state.diffs = vec![None];
        mark_detail_sections_loaded(view);
        view.selection_state.set_pull_request_index(9);
        view.selection_state.set_diff_position(7, 2);

        let key = view
            .current_pull_request_inbox_key()
            .expect("configured repository should produce inbox cache key");
        view.cache_current_pull_request_inbox_snapshot();
        assert_eq!(view.pull_request_inbox.snapshot_count(&key), Some(1));

        view.pull_requests.clear();
        view.detail_state.files.clear();
        view.detail_state.diffs.clear();
        view.selection_state.set_pull_request_index(3);
        view.selection_state.set_diff_position(3, 0);

        assert!(view.restore_pull_request_inbox_snapshot(key, cx));
        assert_eq!(view.pull_requests.len(), 1);
        assert_eq!(view.selected_pull_request_index(), 0);
        assert_eq!(view.selected_pull_request_number(), Some(7));
        assert_eq!(view.active_file_index(), 0);
        assert_eq!(view.active_hunk_index(), 2);
        assert_eq!(
            view.status,
            "Showing cached open pull requests from acme/app"
        );
    });
    cx.run_until_parked();

    assert!(api.calls().is_empty());
}

#[gpui::test]
async fn repository_load_restores_in_memory_snapshot_before_refresh(cx: &mut TestAppContext) {
    let api = Arc::new(FakeGitHubApi::default());
    let cached_pull_request = pull_request();
    let mut refreshed_pull_request = cached_pull_request.clone();
    refreshed_pull_request.title = "Updated pull request".to_string();
    api.push_light_pull_requests(Ok(ConditionalFetch::Modified {
        value: vec![refreshed_pull_request.clone()],
        validator: None,
    }));
    let (view_entity, cx) = init_workspace_service_test(cx, api.clone());

    view_entity.update(cx, |view, cx| {
        view.repository_state
            .select_repository(cached_pull_request.repo.clone());
        view.pull_request_inbox.set_mode(PullRequestInboxMode::Open);
        view.pull_requests = vec![cached_pull_request.clone()];
        mark_detail_sections_loaded(view);
        view.cache_current_pull_request_inbox_snapshot();

        view.repository_state
            .select_repository(RepoId::new("acme", "other"));
        view.pull_request_inbox
            .set_mode(PullRequestInboxMode::Closed);
        view.pull_requests.clear();

        view.load_repository_pull_requests_from_cache(
            cached_pull_request.repo.clone(),
            PullRequestInboxMode::Open,
            cx,
        );

        assert_eq!(view.pull_requests, vec![cached_pull_request.clone()]);
        assert!(view.pull_request_inbox.is_loading());
        assert_eq!(
            view.status,
            "Showing cached open pull requests from acme/app"
        );
    });
    cx.run_until_parked();

    view_entity.read_with(cx, |view, _| {
        assert_eq!(view.pull_requests, vec![refreshed_pull_request.clone()]);
        assert_eq!(view.status, "Loaded 1 open pull requests from acme/app");
        assert!(!view.pull_request_inbox.is_loading());
    });
    assert_eq!(api.calls(), vec!["list_repository_pull_requests_light"]);
}

#[gpui::test]
async fn signed_out_state_clears_visible_github_content(cx: &mut TestAppContext) {
    let api = Arc::new(FakeGitHubApi::default());
    let pull_request = pull_request();
    let (view_entity, cx) = init_workspace_service_test(cx, api);

    view_entity.update(cx, |view, _| {
        view.auth_status = GitHubAuthStatus::SignedOut;
        view.repository_state
            .select_repository(pull_request.repo.clone());
        view.pull_requests = vec![pull_request];
        view.detail_state.files = vec![test_diff_file()];
        view.detail_state.workflow_runs = vec![workflow_run()];
        view.review_state.current_user_login = Some("octocat".to_string());
        view.pull_request_inbox.start_loading();

        view.show_github_sign_in_required();

        assert!(view.pull_requests.is_empty());
        assert!(view.current_repository().is_none());
        assert!(view.detail_state.files.is_empty());
        assert!(view.detail_state.workflow_runs.is_empty());
        assert_eq!(view.review_state.current_user_login, None);
        assert!(!view.pull_request_inbox.is_loading());
        assert_eq!(
            view.status,
            "Choose a GitHub sign-in method to load repositories"
        );
    });
}

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

#[gpui::test]
async fn refresh_review_data_keeps_reviews_when_threads_fail(cx: &mut TestAppContext) {
    let api = Arc::new(FakeGitHubApi::default());
    let pull_request = pull_request();
    let review = pull_request_review("review-1", PullRequestReviewState::Approved);
    api.push_current_user(Ok("octocat".to_string()));
    api.push_reviews(Ok(vec![review.clone()]));
    api.push_review_threads(Err(github_error("threads failed")));
    let (view_entity, cx) = init_workspace_service_test(cx, api);

    view_entity.update(cx, |view, cx| {
        view.pull_requests = vec![pull_request];
        view.selection_state.reset_pull_request_index();
        view.load_selected_review_data(cx);
    });
    cx.run_until_parked();

    view_entity.read_with(cx, |view, _| {
        assert_eq!(view.review_state.pull_request_reviews, vec![review]);
        assert!(view.review_state.review_threads.is_empty());
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

#[gpui::test]
async fn refresh_review_data_keeps_threads_when_reviews_fail(cx: &mut TestAppContext) {
    let api = Arc::new(FakeGitHubApi::default());
    let pull_request = pull_request();
    let thread = review_thread(ReviewThreadState::Unresolved);
    api.push_current_user(Ok("octocat".to_string()));
    api.push_reviews(Err(github_error("reviews failed")));
    api.push_review_threads(Ok(vec![thread.clone()]));
    let (view_entity, cx) = init_workspace_service_test(cx, api);

    view_entity.update(cx, |view, cx| {
        view.pull_requests = vec![pull_request];
        view.selection_state.reset_pull_request_index();
        view.load_selected_review_data(cx);
    });
    cx.run_until_parked();

    view_entity.read_with(cx, |view, _| {
        assert!(view.review_state.pull_request_reviews.is_empty());
        assert_eq!(view.review_state.review_threads, vec![thread]);
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

#[gpui::test]
async fn workflow_action_reports_success_and_failure_from_service(cx: &mut TestAppContext) {
    let success_api = Arc::new(FakeGitHubApi::default());
    success_api.push_dispatch_workflow(Ok(()));
    let (success_view, cx) = init_workspace_service_test(cx, success_api.clone());

    success_view.update(cx, |view, cx| {
        view.pull_requests = vec![pull_request()];
        view.selection_state.reset_pull_request_index();
        view.detail_state.workflow_runs = vec![workflow_run()];
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
        view.detail_state.workflow_runs = vec![workflow_run()];
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

fn init_workspace_service_test(
    cx: &mut TestAppContext,
    api: Arc<FakeGitHubApi>,
) -> (Entity<AppView>, &mut VisualTestContext) {
    cx.update(|cx| {
        gpui_component::init(cx);
        Theme::change(ThemeMode::Dark, None, cx);
    });

    let mut view_entity = None;
    let (_, cx) = cx.add_window_view(|window, cx| {
        let view = cx.new(|cx| AppView::new_with_github_api(api, window, cx));
        view_entity = Some(view);
        Root::new(cx.new(|_| EmptyHarness), window, cx)
    });

    (
        view_entity.expect("workspace service test AppView should be created"),
        cx,
    )
}

fn enqueue_successful_detail_load(api: &FakeGitHubApi, pull_request: &PullRequest) {
    api.push_pull_request_detail(Ok(pull_request.clone()));
    api.push_files(Ok(vec![test_diff_file()]));
    api.push_check_runs(Ok(Vec::new()));
    api.push_workflow_runs(Ok(Vec::new()));
    api.push_current_user(Ok("octocat".to_string()));
    api.push_reviews(Ok(Vec::new()));
    api.push_review_threads(Ok(Vec::new()));
}

fn mark_detail_sections_loaded(view: &mut AppView) {
    view.detail_state.apply_details_success();
    view.detail_state.apply_files_success();
    view.detail_state.apply_checks_success();
    view.detail_state.apply_workflows_success();
    view.review_state.apply_reviews_success();
}

fn test_diff_file() -> DiffFile {
    let mut file = diff_file("src/lib.rs", FileStatus::Modified);
    file.patch = Some("@@ -1 +1 @@\n-old\n+new\n".to_string());
    file
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

fn workflow_run() -> WorkflowRun {
    WorkflowRun {
        id: 42,
        workflow_id: Some(9),
        name: "build".to_string(),
        workflow_name: Some("CI".to_string()),
        status: WorkflowStatus::Completed,
        conclusion: Some(WorkflowConclusion::Failure),
        head_branch: "feature".to_string(),
        head_sha: "abc123".to_string(),
        event: "pull_request".to_string(),
        url: "https://api.github.com/repos/acme/app/actions/runs/42".to_string(),
        html_url: "https://github.com/acme/app/actions/runs/42".to_string(),
        created_at: test_time(),
        updated_at: test_time(),
    }
}

fn github_error(message: &str) -> GitHubError {
    GitHubError::Transport(message.to_string())
}

fn enrichment(pull_request: &PullRequest) -> PullRequestEnrichment {
    PullRequestEnrichment {
        node_id: pull_request.node_id.clone(),
        review_decision: pull_request.review_decision,
        merge_state: pull_request.merge_state,
        checks_summary: Default::default(),
    }
}

struct EmptyHarness;

impl Render for EmptyHarness {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
    }
}
