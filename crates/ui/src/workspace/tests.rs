use std::sync::Arc;

use super::*;
use crate::gpui_component::{Root, Theme, ThemeMode};
use crate::{
    actions::ToggleRepositorySwitcher,
    test_fixtures::{pull_request, review_thread},
    workspace::test_support::FakeGitHubApi,
};
use gpui::{AppContext, ScrollDelta, ScrollWheelEvent, TestAppContext, point, px};
use harbor_domain::ReviewThreadState;

#[gpui::test]
fn review_thread_index_tracks_edits_removal_and_restored_snapshots(cx: &mut TestAppContext) {
    cx.update(crate::gpui_component::init);
    let (view, cx) = cx.add_window_view(AppView::new_without_startup_tasks);

    view.update(cx, |view, _| {
        let state = &mut view.review_state;
        let first = review_thread(ReviewThreadState::Unresolved);
        let mut second = first.clone();
        second.id = "thread-2".to_string();
        second.comments[0].id = "comment-2".to_string();
        state.replace_loaded_review_threads(vec![first, second]);
        let snapshot = state.shared_review_threads();
        let original_index = state.review_thread_index();
        assert!(Arc::ptr_eq(&original_index, &state.review_thread_index()));
        assert_eq!(indexed_review_threads(state, 12), [0, 1]);

        state
            .review_comment_mut("comment-1")
            .expect("first comment exists")
            .position
            .as_mut()
            .expect("fixture comment has a position")
            .line = Some(20);
        assert!(!Arc::ptr_eq(&original_index, &state.review_thread_index()));
        assert_eq!(indexed_review_threads(state, 12), [1]);
        assert_eq!(indexed_review_threads(state, 20), [0]);

        state.remove_review_comment("comment-1");
        assert_eq!(indexed_review_threads(state, 12), [0]);
        assert!(indexed_review_threads(state, 20).is_empty());

        let optimistic = state.insert_optimistic_review_thread(
            harbor_domain::ReviewCommentRange {
                path: "src/lib.rs".to_string(),
                line: 20,
                side: harbor_domain::ReviewSide::Right,
                start_line: None,
                start_side: None,
            },
            "New comment".to_string(),
        );
        assert_eq!(indexed_review_threads(state, 20), [1]);
        state.remove_review_comment(&optimistic.comment_id);
        assert!(indexed_review_threads(state, 20).is_empty());

        state.restore_review_snapshot(
            Arc::new(Vec::new()),
            Arc::new(Vec::new()),
            snapshot,
            None,
            None,
            true,
        );
        assert_eq!(indexed_review_threads(state, 12), [0, 1]);
        assert!(indexed_review_threads(state, 20).is_empty());

        state.replace_loaded_review_threads(vec![review_thread(ReviewThreadState::Resolved)]);
        assert_eq!(indexed_review_threads(state, 12), [0]);
        state.clear_review_data();
        assert!(indexed_review_threads(state, 12).is_empty());
    });
}

fn indexed_review_threads(state: &ReviewRuntimeState, line: u32) -> Vec<usize> {
    let file = crate::test_fixtures::patched_diff_file();
    let line = harbor_domain::diff::DiffLine::<()> {
        kind: harbor_domain::diff::DiffLineKind::Added,
        old_line: None,
        new_line: Some(line),
        text: String::new(),
        syntax_highlights: Vec::new(),
        changed_range: None,
    };
    let mut indices = Vec::new();
    state
        .review_thread_index()
        .for_each_thread_for_line(&file, &line, |index| indices.push(index));
    indices
}

#[gpui::test]
#[ignore = "manual CPU benchmark: script/test -p harbor-ui benchmark_scroll_review_index -- --ignored --nocapture"]
fn benchmark_scroll_review_index(cx: &mut TestAppContext) {
    use std::{hint::black_box, time::Instant};

    cx.update(crate::gpui_component::init);
    let (view, cx) = cx.add_window_view(AppView::new_without_startup_tasks);
    view.update(cx, |view, _| {
        const REBUILD_ITERATIONS: u32 = 200;
        const CACHED_ITERATIONS: u32 = 200_000;
        let state = &mut view.review_state;
        for thread_count in [1_000, 10_000] {
            let threads = (1..=thread_count)
                .map(|line| harbor_domain::ReviewThread {
                    comments_complete: true,
                    id: format!("thread-{line}"),
                    path: "src/lib.rs".to_string(),
                    range: Some(harbor_domain::ReviewCommentRange {
                        path: "src/lib.rs".to_string(),
                        line,
                        side: harbor_domain::ReviewSide::Right,
                        start_line: None,
                        start_side: None,
                    }),
                    state: ReviewThreadState::Unresolved,
                    comments: Vec::new(),
                })
                .collect();
            state.replace_loaded_review_threads(threads);
            black_box(state.review_thread_index());

            let started_at = Instant::now();
            for _ in 0..REBUILD_ITERATIONS {
                black_box(Arc::new(
                    harbor_domain::diff_reviews::ReviewThreadIndex::new(black_box(
                        state.review_threads(),
                    )),
                ));
            }
            let rebuild_time = started_at.elapsed() / REBUILD_ITERATIONS;

            let started_at = Instant::now();
            for _ in 0..CACHED_ITERATIONS {
                black_box(black_box(&*state).review_thread_index());
            }
            let reuse_time = started_at.elapsed() / CACHED_ITERATIONS;
            println!(
                "{thread_count} threads: rebuild {rebuild_time:?}, cached {reuse_time:?} per access"
            );
        }
    });
}

#[test]
fn defaults_pull_request_inbox_to_open_mode() {
    assert_eq!(PullRequestInboxMode::default(), PullRequestInboxMode::Open);
    assert_eq!(PullRequestInboxMode::Open.label(), "Open");
    assert_eq!(PullRequestInboxMode::Closed.label(), "Closed");
    assert_eq!(PullRequestInboxMode::NeedsReview.label(), "Needs review");
    assert_eq!(
        PullRequestInboxMode::Closed.empty_message(),
        "No closed pull requests"
    );
}

#[gpui::test]
async fn initial_panel_loads_use_skeleton_rows(cx: &mut TestAppContext) {
    cx.update(|cx| {
        crate::gpui_component::init(cx);
        Theme::change(ThemeMode::Dark, None, cx);
    });

    let mut view_entity = None;
    let (_, cx) = cx.add_window_view(|window, cx| {
        let view = cx.new(|cx| AppView::new_without_startup_tasks(window, cx));
        view_entity = Some(view.clone());
        view.update(cx, |view, cx| {
            let repository = RepoId::new("harbor", "harbor-fixtures");
            view.auth_status = GitHubAuthStatus::SignedIn {
                login: Some("octocat".to_string()),
                source: GitHubAuthSource::GhCli,
            };
            view.repository_state.select_repository(repository.clone());
            view.repository_actions_state
                .reset_for_repository(repository);
            view.repository_actions_state.start_workflows_load();
            view.repository_actions_state.start_runs_load();
            view.pull_requests = vec![pull_request()];
            view.detail_state.start_files_load();
            view.detail_state.start_checks_load();
            view.detail_state.start_commits_load();
            view.detail_state.start_workflows_load();
            view.ensure_workflow_log_view(cx)
                .unwrap()
                .update(cx, |logs, cx| {
                    logs.state.start_loading();
                    cx.notify();
                });
            view.review_state.start_reviews_load();
            view.active_tab = PanelTab::Diff;
            cx.notify();
        });
        Root::new(view, window, cx)
    });
    let view_entity = view_entity.expect("test AppView should be created");

    cx.refresh().expect("diff loading state should render");
    assert!(cx.debug_bounds("diff-loading-skeleton").is_some());
    assert!(cx.debug_bounds("changed-files-loading-skeleton").is_some());

    for (tab, selector) in [
        (PanelTab::Checks, "checks-loading-skeleton"),
        (PanelTab::Commits, "commits-loading-skeleton"),
        (PanelTab::Logs, "logs-loading-skeleton"),
    ] {
        view_entity.update(cx, |view, cx| {
            view.active_tab = tab;
            cx.notify();
        });
        cx.refresh().expect("panel loading state should render");
        assert!(
            cx.debug_bounds(selector).is_some(),
            "{selector} should render on the {tab:?} tab"
        );
    }

    view_entity.update(cx, |view, cx| {
        view.active_tab = PanelTab::Actions;
        cx.notify();
    });
    cx.refresh().expect("Actions loading state should render");
    assert!(
        cx.debug_bounds("workflow-sidebar-loading-skeleton")
            .is_some()
    );
    assert!(cx.debug_bounds("workflow-runs-loading-skeleton").is_some());
    assert!(
        cx.debug_bounds("selected-pr-workflow-actions-loading-skeleton")
            .is_some()
    );
}

#[gpui::test]
async fn repository_switcher_starts_closed(cx: &mut TestAppContext) {
    cx.update(|cx| {
        crate::gpui_component::init(cx);
        Theme::change(ThemeMode::Dark, None, cx);
    });

    let mut view_entity = None;
    let (_, cx) = cx.add_window_view(|window, cx| {
        let view = cx.new(|cx| AppView::new_without_startup_tasks(window, cx));
        view_entity = Some(view.clone());
        Root::new(view, window, cx)
    });

    view_entity
        .expect("test AppView should be created")
        .read_with(cx, |view, _| {
            assert!(!view.repository_state.repository_switcher_open);
        });
}

#[gpui::test]
async fn app_view_starts_before_storage_is_ready(cx: &mut TestAppContext) {
    cx.update(|cx| {
        crate::gpui_component::init(cx);
        Theme::change(ThemeMode::Dark, None, cx);
    });

    let mut view_entity = None;
    let (_, cx) = cx.add_window_view(|window, cx| {
        let view = cx.new(|cx| AppView::new(Arc::new(FakeGitHubApi::default()), None, window, cx));
        view_entity = Some(view.clone());
        Root::new(view, window, cx)
    });

    view_entity
        .expect("test AppView should be created")
        .read_with(cx, |view, _| {
            assert!(view.repository_state.store().is_none());
            assert!(view.repository_state.error().is_none());
            assert!(view.repository_state.is_loading());
            assert_eq!(view.status, "Initializing local storage...");
        });

    cx.refresh()
        .expect("workspace should render before storage is ready");
    assert!(cx.debug_bounds("pull-request-inbox-loading").is_some());
    assert!(cx.debug_bounds("pull-request-workspace-loading").is_some());
    assert!(cx.debug_bounds("pull-request-empty-state").is_none());
}

#[gpui::test]
async fn repository_switcher_does_not_open_while_auth_gate_is_visible(cx: &mut TestAppContext) {
    cx.update(|cx| {
        crate::gpui_component::init(cx);
        Theme::change(ThemeMode::Dark, None, cx);
    });

    let mut view_entity = None;
    let (_, cx) = cx.add_window_view(|window, cx| {
        let view = cx.new(|cx| AppView::new_without_startup_tasks(window, cx));
        view.update(cx, |view, cx| {
            view.auth_status = GitHubAuthStatus::SignedOut;
            view.toggle_repository_switcher(&ToggleRepositorySwitcher, window, cx);
            assert!(!view.repository_state.repository_switcher_open);
        });
        view_entity = Some(view.clone());
        Root::new(view, window, cx)
    });

    view_entity
        .expect("test AppView should be created")
        .read_with(cx, |view, _| {
            assert!(!view.repository_state.repository_switcher_open);
        });
}

#[gpui::test]
async fn workflow_output_scrolls_both_axes_and_stays_visible_during_refresh(
    cx: &mut TestAppContext,
) {
    cx.update(|cx| {
        crate::gpui_component::init(cx);
        Theme::change(ThemeMode::Dark, None, cx);
    });
    let mut view_entity = None;
    let (_, cx) = cx.add_window_view(|window, cx| {
        let view = cx.new(|cx| AppView::new_without_startup_tasks(window, cx));
        view_entity = Some(view.clone());
        view.update(cx, |view, cx| {
            view.auth_status = GitHubAuthStatus::SignedIn {
                login: Some("octocat".into()),
                source: GitHubAuthSource::GhCli,
            };
            view.pull_requests = vec![pull_request()];
            view.detail_state
                .replace_workflow_runs(vec![crate::test_fixtures::workflow_run()]);
            let output = format!(
                "short\n{}",
                format!("{}\n", "wide output ".repeat(100)).repeat(500)
            );
            view.detail_state.log_chunk =
                Some(Arc::new(harbor_logs::parse_workflow_log(42, &output)));
            view.active_tab = PanelTab::Logs;
            view.ensure_workflow_log_view(cx);
        });
        Root::new(view, window, cx)
    });
    let view = view_entity.unwrap();
    cx.run_until_parked();
    cx.refresh().unwrap();
    let viewport = cx
        .debug_bounds("workflow-log-viewport")
        .expect("log viewport is visible");
    assert!(viewport.size.height > px(0.));
    view.read_with(cx, |view, cx| {
        let scroll = &view
            .workflow_logs
            .as_ref()
            .unwrap()
            .read(cx)
            .state
            .list_scroll
            .0
            .borrow()
            .base_handle;
        assert!(
            scroll.max_offset().x > px(0.),
            "long lines must contribute to scroll width"
        );
        assert!(
            scroll.max_offset().y > px(0.),
            "output must have a bounded viewport"
        );
    });
    cx.simulate_event(ScrollWheelEvent {
        position: viewport.center(),
        delta: ScrollDelta::Pixels(point(px(-400.), px(0.))),
        ..Default::default()
    });
    cx.refresh().unwrap();
    cx.simulate_event(ScrollWheelEvent {
        position: viewport.center(),
        delta: ScrollDelta::Pixels(point(px(0.), px(-200.))),
        ..Default::default()
    });
    cx.refresh().unwrap();
    let offset = view.read_with(cx, |view, cx| {
        view.workflow_logs
            .as_ref()
            .unwrap()
            .read(cx)
            .state
            .list_scroll
            .0
            .borrow()
            .base_handle
            .offset()
    });
    assert!(offset.x < px(0.), "horizontal wheel input scrolls output");
    assert!(offset.y < px(0.), "vertical wheel input scrolls output");
    view.update(cx, |view, cx| {
        view.ensure_workflow_log_view(cx)
            .unwrap()
            .update(cx, |logs, cx| {
                logs.state.start_loading();
                cx.notify();
            });
        cx.notify();
    });
    cx.refresh().unwrap();
    assert_eq!(cx.debug_bounds("workflow-log-viewport"), Some(viewport));
    view.read_with(cx, |view, cx| {
        assert_eq!(
            view.workflow_logs
                .as_ref()
                .unwrap()
                .read(cx)
                .state
                .list_scroll
                .0
                .borrow()
                .base_handle
                .offset(),
            offset
        );
        assert_eq!(
            view.workflow_logs
                .as_ref()
                .unwrap()
                .read(cx)
                .state
                .visible_lines
                .len(),
            501
        );
    });
    assert!(cx.debug_bounds("logs-loading-skeleton").is_none());
}
