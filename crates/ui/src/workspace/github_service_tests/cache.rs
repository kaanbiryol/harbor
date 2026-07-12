use std::sync::Arc;

use gpui::TestAppContext;
use harbor_domain::{FileStatus, RepoId};
use harbor_github::ConditionalFetch;

use crate::{
    actions::PanelTab,
    diff::parse_files,
    panels::DiffListItem,
    test_fixtures::{diff_file, patched_diff_file, pull_request},
    workspace::{AppView, PullRequestInboxMode, github_service::test_support::FakeGitHubApi},
};

use super::init_workspace_service_test;

#[gpui::test]
async fn cached_detail_restore_preserves_diff_position_without_refetch(cx: &mut TestAppContext) {
    let api = Arc::new(FakeGitHubApi::default());
    let (view_entity, cx) = init_workspace_service_test(cx, api.clone());

    view_entity.update(cx, |view, cx| {
        view.pull_requests = vec![pull_request()];
        view.selection_state.reset_pull_request_index();
        view.detail_state.replace_diff_files(
            vec![
                diff_file("src/a.rs", FileStatus::Modified),
                diff_file("src/b.rs", FileStatus::Modified),
            ],
            vec![None, None],
        );
        mark_detail_sections_loaded(view);
        view.selection_state.set_diff_position(1, 4);
        view.active_tab = PanelTab::Diff;
        view.cache_current_pull_request_detail_snapshot();

        view.detail_state.replace_diff_files(
            vec![diff_file("src/other.rs", FileStatus::Modified)],
            vec![None],
        );
        view.selection_state.set_diff_position(0, 0);
        view.active_tab = PanelTab::Review;

        assert!(view.restore_selected_pull_request_detail_snapshot(cx));
        assert_eq!(
            view.detail_state
                .files()
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
async fn cached_detail_restore_rebuilds_diff_list_items(cx: &mut TestAppContext) {
    let api = Arc::new(FakeGitHubApi::default());
    let (view_entity, cx) = init_workspace_service_test(cx, api.clone());

    view_entity.update(cx, |view, cx| {
        view.pull_requests = vec![pull_request()];
        view.selection_state.reset_pull_request_index();
        let files = vec![patched_file("src/a.rs"), patched_file("src/b.rs")];
        view.detail_state
            .replace_diff_files(files.clone(), parse_files(&files));
        view.changed_files_state
            .reviewed_file_paths
            .insert("src/a.rs".to_string());
        mark_detail_sections_loaded(view);
        view.active_tab = PanelTab::Diff;
        view.cache_current_pull_request_detail_snapshot();

        let stale_files = vec![patched_file("src/other.rs")];
        view.detail_state
            .replace_diff_files(stale_files.clone(), parse_files(&stale_files));
        view.changed_files_state.reviewed_file_paths.clear();
        view.sync_diff_list_items(cx);
        assert_eq!(file_headers(&view.diff_list_items), vec![0]);

        assert!(view.restore_selected_pull_request_detail_snapshot(cx));
        assert_eq!(file_headers(&view.diff_list_items), vec![0, 1]);
        assert!(
            !view
                .diff_list_items
                .iter()
                .any(|item| matches!(item, DiffListItem::Line { file_index: 0, .. }))
        );
        assert!(
            view.diff_list_items
                .iter()
                .any(|item| matches!(item, DiffListItem::Line { file_index: 1, .. }))
        );
    });
    cx.run_until_parked();

    assert!(api.calls().is_empty());
}

#[gpui::test]
async fn cached_detail_restore_preserves_diff_section_overrides(cx: &mut TestAppContext) {
    let api = Arc::new(FakeGitHubApi::default());
    let (view_entity, cx) = init_workspace_service_test(cx, api.clone());

    view_entity.update(cx, |view, cx| {
        view.pull_requests = vec![pull_request()];
        view.selection_state.reset_pull_request_index();
        let files = vec![patched_file("src/a.rs"), patched_file("src/b.rs")];
        view.detail_state
            .replace_diff_files(files.clone(), parse_files(&files));
        view.changed_files_state
            .reviewed_file_paths
            .insert("src/a.rs".to_string());
        view.changed_files_state
            .expanded_diff_file_paths
            .insert("src/a.rs".to_string());
        view.changed_files_state
            .collapsed_diff_file_paths
            .insert("src/b.rs".to_string());
        mark_detail_sections_loaded(view);
        view.cache_current_pull_request_detail_snapshot();

        view.changed_files_state.reviewed_file_paths.clear();
        view.changed_files_state.expanded_diff_file_paths.clear();
        view.changed_files_state.collapsed_diff_file_paths.clear();
        view.sync_diff_list_items(cx);

        assert!(view.restore_selected_pull_request_detail_snapshot(cx));
        assert!(
            view.changed_files_state
                .reviewed_file_paths
                .contains("src/a.rs")
        );
        assert!(
            view.changed_files_state
                .expanded_diff_file_paths
                .contains("src/a.rs")
        );
        assert!(
            view.changed_files_state
                .collapsed_diff_file_paths
                .contains("src/b.rs")
        );
        assert!(
            view.diff_list_items
                .iter()
                .any(|item| matches!(item, DiffListItem::Line { file_index: 0, .. }))
        );
        assert!(
            !view
                .diff_list_items
                .iter()
                .any(|item| matches!(item, DiffListItem::Line { file_index: 1, .. }))
        );
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
        view.detail_state
            .replace_diff_files(vec![patched_diff_file()], vec![None]);
        mark_detail_sections_loaded(view);
        view.selection_state.set_pull_request_index(9);
        view.selection_state.set_diff_position(7, 2);

        let key = view
            .current_pull_request_inbox_key()
            .expect("configured repository should produce inbox cache key");
        view.cache_current_pull_request_inbox_snapshot();
        assert_eq!(view.pull_request_inbox.snapshot_count(&key), Some(1));

        view.pull_requests.clear();
        view.detail_state.clear_diff_files();
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
async fn cached_inbox_restore_rebuilds_diff_list_items(cx: &mut TestAppContext) {
    let api = Arc::new(FakeGitHubApi::default());
    let pull_request = pull_request();
    let (view_entity, cx) = init_workspace_service_test(cx, api.clone());

    view_entity.update(cx, |view, cx| {
        view.repository_state
            .select_repository(pull_request.repo.clone());
        view.pull_request_inbox.set_mode(PullRequestInboxMode::Open);
        view.pull_requests = vec![pull_request.clone()];
        let files = vec![patched_file("src/a.rs"), patched_file("src/b.rs")];
        view.detail_state
            .replace_diff_files(files.clone(), parse_files(&files));
        view.changed_files_state
            .reviewed_file_paths
            .insert("src/a.rs".to_string());
        mark_detail_sections_loaded(view);

        let key = view
            .current_pull_request_inbox_key()
            .expect("configured repository should produce inbox cache key");
        view.cache_current_pull_request_inbox_snapshot();

        view.pull_requests.clear();
        view.detail_state.clear_diff_files();
        view.changed_files_state.reviewed_file_paths.clear();
        view.sync_diff_list_items(cx);
        assert!(view.diff_list_items.is_empty());

        assert!(view.restore_pull_request_inbox_snapshot(key, cx));
        assert_eq!(file_headers(&view.diff_list_items), vec![0, 1]);
        assert!(
            !view
                .diff_list_items
                .iter()
                .any(|item| matches!(item, DiffListItem::Line { file_index: 0, .. }))
        );
        assert!(
            view.diff_list_items
                .iter()
                .any(|item| matches!(item, DiffListItem::Line { file_index: 1, .. }))
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

fn mark_detail_sections_loaded(view: &mut AppView) {
    view.detail_state.apply_details_success();
    view.detail_state.apply_files_success();
    view.detail_state.apply_checks_success();
    view.detail_state.apply_workflows_success();
    view.review_state.apply_reviews_success();
}

fn patched_file(path: &str) -> harbor_domain::DiffFile {
    let mut file = patched_diff_file();
    file.path = path.to_string();
    file
}

fn file_headers(items: &[DiffListItem]) -> Vec<usize> {
    items
        .iter()
        .filter_map(|item| match item {
            DiffListItem::FileHeader { file_index, .. } => Some(*file_index),
            DiffListItem::Line { .. }
            | DiffListItem::ReviewComposer { .. }
            | DiffListItem::ReviewThread { .. }
            | DiffListItem::DiffUnavailable { .. } => None,
        })
        .collect()
}
