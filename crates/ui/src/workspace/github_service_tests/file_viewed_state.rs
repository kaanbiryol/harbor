use std::sync::Arc;

use gpui::TestAppContext;
use harbor_domain::FileViewedState;

use crate::{
    diff::parse_files,
    panels::DiffListItem,
    test_fixtures::{patched_diff_file, pull_request},
    workspace::github_service::test_support::FakeGitHubApi,
};

use super::{github_error, init_workspace_service_test};

#[gpui::test]
async fn marking_changed_file_reviewed_syncs_to_github(cx: &mut TestAppContext) {
    let api = Arc::new(FakeGitHubApi::default());
    api.push_mark_file_viewed(Ok(()));
    let (view_entity, cx) = init_workspace_service_test(cx, api.clone());

    view_entity.update(cx, |view, cx| {
        view.pull_requests = vec![pull_request()];
        view.selection_state.reset_pull_request_index();
        let files = vec![patched_diff_file()];
        view.detail_state
            .replace_diff_files(files.clone(), parse_files(&files));

        view.toggle_changed_file_reviewed(0, cx);

        assert!(view.reviewed_file_paths().contains("src/lib.rs"));
        assert_eq!(
            view.detail_state.files()[0].viewed_state,
            FileViewedState::Viewed
        );
    });
    cx.run_until_parked();

    assert_eq!(api.calls(), vec!["mark_pull_request_file_viewed"]);
}

#[gpui::test]
async fn unmarking_changed_file_reviewed_syncs_to_github(cx: &mut TestAppContext) {
    let api = Arc::new(FakeGitHubApi::default());
    api.push_unmark_file_viewed(Ok(()));
    let (view_entity, cx) = init_workspace_service_test(cx, api.clone());

    view_entity.update(cx, |view, cx| {
        view.pull_requests = vec![pull_request()];
        view.selection_state.reset_pull_request_index();
        let mut file = patched_diff_file();
        file.viewed_state = FileViewedState::Viewed;
        let files = vec![file];
        view.detail_state
            .replace_diff_files(files.clone(), parse_files(&files));
        view.sync_reviewed_file_paths_from_files();

        view.toggle_changed_file_reviewed(0, cx);

        assert!(!view.reviewed_file_paths().contains("src/lib.rs"));
        assert_eq!(
            view.detail_state.files()[0].viewed_state,
            FileViewedState::Unviewed
        );
    });
    cx.run_until_parked();

    assert_eq!(api.calls(), vec!["unmark_pull_request_file_viewed"]);
}

#[gpui::test]
async fn failed_mark_changed_file_reviewed_rolls_back(cx: &mut TestAppContext) {
    let api = Arc::new(FakeGitHubApi::default());
    api.push_mark_file_viewed(Err(github_error("viewed state update failed")));
    let (view_entity, cx) = init_workspace_service_test(cx, api.clone());

    view_entity.update(cx, |view, cx| {
        view.pull_requests = vec![pull_request()];
        view.selection_state.reset_pull_request_index();
        let files = vec![patched_diff_file()];
        view.detail_state
            .replace_diff_files(files.clone(), parse_files(&files));

        view.toggle_changed_file_reviewed(0, cx);
        assert!(view.reviewed_file_paths().contains("src/lib.rs"));
    });
    cx.run_until_parked();

    view_entity.read_with(cx, |view, _| {
        assert!(!view.reviewed_file_paths().contains("src/lib.rs"));
        assert_eq!(
            view.detail_state.files()[0].viewed_state,
            FileViewedState::Unviewed
        );
        assert!(
            view.status
                .contains("Failed to mark src/lib.rs as reviewed")
        );
    });
    assert_eq!(api.calls(), vec!["mark_pull_request_file_viewed"]);
}

#[gpui::test]
async fn toggling_diff_file_section_does_not_select_file_or_sync_reviewed_state(
    cx: &mut TestAppContext,
) {
    let api = Arc::new(FakeGitHubApi::default());
    let (view_entity, cx) = init_workspace_service_test(cx, api.clone());

    view_entity.update(cx, |view, cx| {
        let mut first_file = patched_diff_file();
        first_file.path = "src/a.rs".to_string();
        let mut second_file = patched_diff_file();
        second_file.path = "src/b.rs".to_string();
        let files = vec![first_file, second_file];
        view.detail_state
            .replace_diff_files(files.clone(), parse_files(&files));
        view.selection_state.set_diff_position(1, 0);
        view.sync_diff_list_items(cx);

        view.toggle_diff_file_section(0, cx);

        assert_eq!(view.active_file_index(), 1);
        assert!(view.collapsed_diff_file_paths.contains("src/a.rs"));
        assert!(!view.reviewed_file_paths().contains("src/a.rs"));
        assert!(
            !view
                .diff_list_items
                .iter()
                .any(|item| matches!(item, DiffListItem::Line { file_index: 0, .. }))
        );
        assert_eq!(view.status, "Collapsed src/a.rs");
    });
    cx.run_until_parked();

    assert!(api.calls().is_empty());
}
