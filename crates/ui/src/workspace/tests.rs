use std::sync::Arc;

use super::*;
use crate::{
    actions::{CloseSettings, OpenSettings, ToggleRepositorySwitcher},
    test_fixtures::{pull_request, review_thread, test_time},
    workspace::github_service::test_support::FakeGitHubApi,
};
use gpui::{AppContext, Modifiers, ScrollDelta, ScrollWheelEvent, TestAppContext, point, px};
use gpui_component::{Root, Theme, ThemeMode};
use harbor_domain::{
    Label, PullRequestComment, PullRequestReview, PullRequestReviewState, ReviewThreadState,
};

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
async fn repository_switcher_starts_closed(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_component::init(cx);
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
async fn overview_panel_renders_description_and_editable_metadata(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_component::init(cx);
        Theme::change(ThemeMode::Dark, None, cx);
    });

    let mut view_entity = None;
    let (_, cx) = cx.add_window_view(|window, cx| {
        let view = cx
            .new(|cx| AppView::new_with_github_api(Arc::new(FakeGitHubApi::default()), window, cx));
        view_entity = Some(view.clone());
        view.update(cx, |view, cx| {
            let mut pull_request = pull_request();
            pull_request.body = Some("## summary\n\n- Adds a focused fixture".to_string());
            pull_request.unresolved_threads = 5;
            pull_request.labels = vec![Label {
                name: "needs-review".to_string(),
                color: Some("34d399".to_string()),
            }];
            view.pull_requests = vec![pull_request];
            let unresolved_thread = review_thread(ReviewThreadState::Unresolved);
            let mut resolved_thread = review_thread(ReviewThreadState::Resolved);
            resolved_thread.id = "thread-resolved".to_string();
            resolved_thread.path = "src/resolved.rs".to_string();
            resolved_thread.comments[0].id = "comment-resolved".to_string();
            view.review_state.apply_loaded_review_data(
                vec![
                    PullRequestReview {
                        id: "review-1".to_string(),
                        node_id: Some("review-node-1".to_string()),
                        author: "reviewer".to_string(),
                        state: PullRequestReviewState::Approved,
                        body: None,
                        submitted_at: Some(test_time()),
                    },
                    PullRequestReview {
                        id: "review-empty".to_string(),
                        node_id: Some("review-node-empty".to_string()),
                        author: "reviewer".to_string(),
                        state: PullRequestReviewState::Commented,
                        body: None,
                        submitted_at: Some(test_time()),
                    },
                ],
                vec![PullRequestComment {
                    id: "comment-1".to_string(),
                    author: "reviewer".to_string(),
                    author_avatar_url: None,
                    body: "Looks good".to_string(),
                    created_at: test_time(),
                    updated_at: None,
                }],
                vec![unresolved_thread, resolved_thread],
                Some("octocat".to_string()),
                None,
            );
            view.active_tab = PanelTab::Overview;
            cx.notify();
        });
        Root::new(view, window, cx)
    });

    cx.refresh().expect("test window should refresh");
    assert!(cx.debug_bounds("pull-request-overview-panel").is_some());
    let description = cx
        .debug_bounds("pull-request-overview-description")
        .expect("overview description should render");
    assert!(cx.debug_bounds("pull-request-overview-sidebar").is_some());
    assert!(cx.debug_bounds("pull-request-overview-timeline").is_some());
    assert!(cx.debug_bounds("overview-review-review-1").is_some());
    assert!(cx.debug_bounds("overview-review-review-empty").is_none());
    assert!(
        cx.debug_bounds("overview-thread-comment-thread-1-0")
            .is_some()
    );
    assert!(
        cx.debug_bounds("overview-thread-comment-thread-resolved-0")
            .is_none()
    );
    let unresolved_thread = cx
        .debug_bounds("overview-thread-card-thread-1")
        .expect("unresolved thread should render");
    assert!(
        unresolved_thread.origin.x + unresolved_thread.size.width
            >= description.origin.x + description.size.width - px(1.0),
        "timeline thread should fill the available content width"
    );
    assert!(cx.debug_bounds("overview-thread-diff-thread-1").is_some());
    let reply_field = cx
        .debug_bounds("overview-reply-field-thread-1")
        .expect("overview reply field should render");
    let toggle_thread = cx
        .debug_bounds("overview-toggle-thread-thread-1")
        .expect("overview resolve action should render");
    assert!(
        reply_field.origin.x >= unresolved_thread.origin.x
            && reply_field.origin.x + reply_field.size.width
                <= unresolved_thread.origin.x + unresolved_thread.size.width,
        "reply field should stay inside the thread card"
    );
    assert!(
        toggle_thread.origin.x + toggle_thread.size.width
            <= unresolved_thread.origin.x + unresolved_thread.size.width,
        "resolve action should stay inside the thread card"
    );
    let thread_node = cx
        .debug_bounds("overview-thread-node-thread-1")
        .expect("overview thread timeline node should render");
    let thread_header = cx
        .debug_bounds("overview-thread-toggle-thread-1")
        .expect("overview thread header should render");
    assert!(
        (thread_node.center().y - thread_header.center().y).abs() <= px(1.0),
        "timeline node should be vertically centered with the file header"
    );
    assert!(cx.debug_bounds("add-reaction-trigger-comment-1").is_some());
    cx.simulate_click(reply_field.center(), Modifiers::none());
    cx.refresh()
        .expect("test window should refresh after opening reply");
    assert!(
        cx.debug_bounds("overview-submit-thread-reply-thread-1")
            .is_some()
    );
    let cancel_reply = cx
        .debug_bounds("overview-cancel-thread-reply-thread-1")
        .expect("overview reply cancel button should render");
    cx.simulate_click(cancel_reply.center(), Modifiers::none());
    cx.refresh()
        .expect("test window should refresh after cancelling reply");
    assert!(cx.debug_bounds("overview-reply-field-thread-1").is_some());
    let unresolved_thread = cx
        .debug_bounds("overview-thread-toggle-thread-1")
        .expect("unresolved thread should remain visible");
    cx.simulate_click(unresolved_thread.center(), Modifiers::none());
    cx.refresh()
        .expect("test window should refresh after collapsing");
    assert!(
        cx.debug_bounds("overview-thread-comment-thread-1-0")
            .is_none()
    );
    let resolved_thread = cx
        .debug_bounds("overview-thread-toggle-thread-resolved")
        .expect("resolved thread summary should render");
    cx.simulate_click(resolved_thread.center(), Modifiers::none());
    cx.refresh()
        .expect("test window should refresh after toggle");
    assert!(
        cx.debug_bounds("overview-thread-comment-thread-resolved-0")
            .is_some()
    );
    view_entity
        .as_ref()
        .expect("test AppView should be created")
        .read_with(cx, |view, _| {
            assert_eq!(view.active_tab, PanelTab::Overview);
        });
    assert!(
        cx.debug_bounds("pull-request-overview-comment-composer")
            .is_some()
    );
    let comment_input = cx
        .debug_bounds("pull-request-overview-comment-input")
        .expect("overview comment input should render");
    assert!(comment_input.size.height > px(60.));
    assert!(comment_input.size.height < px(160.));
    assert!(cx.debug_bounds("pull-request-merge-readiness").is_some());
    assert!(
        cx.debug_bounds("pull-request-unresolved-conversations")
            .is_some()
    );
    assert!(cx.debug_bounds("review-tab-unresolved-count").is_some());
    let author_chip = cx
        .debug_bounds("pull-request-person-octocat")
        .expect("pull request author chip should render");
    assert!(author_chip.size.width > px(40.));
    let label_chip = cx
        .debug_bounds("pull-request-label-needs-review")
        .expect("pull request label chip should render");
    assert!(label_chip.size.width > px(40.));
    assert!(cx.debug_bounds("add-reviewer-control").is_some());
    assert!(cx.debug_bounds("add-assignee-control").is_some());
    assert!(cx.debug_bounds("add-label-control").is_some());
    assert!(cx.debug_bounds("changed-files-sidebar").is_none());
}

#[gpui::test]
async fn overview_sidebar_scrolls_independently(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_component::init(cx);
        Theme::change(ThemeMode::Dark, None, cx);
    });

    let (_, cx) = cx.add_window_view(|window, cx| {
        let view = cx
            .new(|cx| AppView::new_with_github_api(Arc::new(FakeGitHubApi::default()), window, cx));
        view.update(cx, |view, cx| {
            let mut pull_request = pull_request();
            pull_request.unresolved_threads = 5;
            pull_request.labels = (0..24)
                .map(|index| Label {
                    name: format!("sidebar-overflow-{index}"),
                    color: None,
                })
                .collect();
            view.pull_requests = vec![pull_request];
            view.active_tab = PanelTab::Overview;
            cx.notify();
        });
        Root::new(view, window, cx)
    });

    cx.refresh().expect("test window should refresh");
    let sidebar = cx
        .debug_bounds("pull-request-overview-sidebar")
        .expect("overview sidebar should render");
    let people_before_scroll = cx
        .debug_bounds("pull-request-people-card")
        .expect("people card should render");
    let description_before_scroll = cx
        .debug_bounds("pull-request-overview-description")
        .expect("overview description should render");

    cx.simulate_event(ScrollWheelEvent {
        position: sidebar.center(),
        delta: ScrollDelta::Pixels(point(px(0.0), px(-400.0))),
        ..Default::default()
    });
    cx.refresh()
        .expect("test window should refresh after scrolling the sidebar");

    let people_after_scroll = cx
        .debug_bounds("pull-request-people-card")
        .expect("people card should remain rendered after scrolling");
    let description_after_scroll = cx
        .debug_bounds("pull-request-overview-description")
        .expect("overview description should remain rendered after scrolling");
    assert!(
        people_after_scroll.origin.y < people_before_scroll.origin.y,
        "scrolling over the overview sidebar should move its cards"
    );
    assert_eq!(
        description_after_scroll.origin.y, description_before_scroll.origin.y,
        "scrolling over the overview sidebar should not move the timeline"
    );
}

#[gpui::test]
async fn pull_request_header_spans_details_and_panel(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_component::init(cx);
        Theme::change(ThemeMode::Dark, None, cx);
    });

    let (_, cx) = cx.add_window_view(|window, cx| {
        let view = cx
            .new(|cx| AppView::new_with_github_api(Arc::new(FakeGitHubApi::default()), window, cx));
        view.update(cx, |view, cx| {
            view.pull_requests = vec![pull_request()];
            view.active_tab = PanelTab::Diff;
            cx.notify();
        });
        Root::new(view, window, cx)
    });

    cx.refresh().expect("test window should refresh");

    let header = cx
        .debug_bounds("pull-request-workspace-header")
        .expect("pull request header should render");
    let details = cx
        .debug_bounds("changed-files-sidebar")
        .expect("changed files should render on the diff tab");
    let changed_files_title = cx
        .debug_bounds("changed-files-title")
        .expect("changed files title should render");
    let panel = cx
        .debug_bounds("pull-request-panel")
        .expect("pull request panel should render");
    let tabs = cx
        .debug_bounds("pull-request-panel-tabs")
        .expect("pull request tabs should render");

    assert_eq!(header.origin.x, details.origin.x);
    assert!(header.size.width > details.size.width);
    assert!(changed_files_title.size.width > px(80.));
    assert!(
        header.origin.x + header.size.width >= panel.origin.x + panel.size.width,
        "header should reach across the active panel"
    );
    assert!(
        header.size.height < px(130.),
        "pull request header should stay compact, got {:?}",
        header.size.height
    );
    assert!(
        cx.debug_bounds("pull-request-header-metadata").is_some(),
        "metadata and statuses should share the compact secondary row"
    );
    assert!(
        tabs.origin.y + tabs.size.height <= panel.origin.y,
        "tabs should render in the header above the active panel"
    );
}

#[gpui::test]
async fn non_diff_panel_uses_full_workspace_width(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_component::init(cx);
        Theme::change(ThemeMode::Dark, None, cx);
    });

    let (_, cx) = cx.add_window_view(|window, cx| {
        let view = cx
            .new(|cx| AppView::new_with_github_api(Arc::new(FakeGitHubApi::default()), window, cx));
        view.update(cx, |view, cx| {
            view.pull_requests = vec![pull_request()];
            view.active_tab = PanelTab::Review;
            cx.notify();
        });
        Root::new(view, window, cx)
    });

    cx.refresh().expect("test window should refresh");

    let header = cx
        .debug_bounds("pull-request-workspace-header")
        .expect("pull request header should render");
    let panel = cx
        .debug_bounds("pull-request-panel")
        .expect("pull request panel should render");

    assert!(cx.debug_bounds("changed-files-sidebar").is_none());
    assert_eq!(header.origin.x, panel.origin.x);
    assert_eq!(header.size.width, panel.size.width);
}

#[gpui::test]
async fn fullscreen_repository_title_keeps_left_inset(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_component::init(cx);
        Theme::change(ThemeMode::Dark, None, cx);
    });

    let (_, cx) = cx.add_window_view(|window, cx| {
        window.toggle_fullscreen();
        let view = cx.new(|cx| AppView::new_without_startup_tasks(window, cx));
        view.update(cx, |view, cx| {
            view.auth_status = GitHubAuthStatus::SignedIn {
                login: Some("octocat".to_string()),
                source: GitHubAuthSource::GhCli,
            };
            view.repository_state
                .select_repository(RepoId::new("harbor", "harbor-fixtures"));
            cx.notify();
        });
        Root::new(view, window, cx)
    });

    cx.refresh().expect("test window should refresh");
    cx.run_until_parked();

    let bounds = cx
        .debug_bounds("repository-switcher-label")
        .expect("repository label should render");
    assert!(
        bounds.origin.x >= px(8.),
        "fullscreen repository title should keep a visible left inset"
    );
}

#[gpui::test]
async fn repository_switcher_does_not_open_while_auth_gate_is_visible(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_component::init(cx);
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
async fn dismissing_github_auth_popover_keeps_pending_sign_in(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_component::init(cx);
        Theme::change(ThemeMode::Dark, None, cx);
    });

    let mut view_entity = None;
    let (_, cx) = cx.add_window_view(|window, cx| {
        let view = cx.new(|cx| AppView::new_without_startup_tasks(window, cx));
        view.update(cx, |view, cx| {
            view.auth_status = GitHubAuthStatus::SigningIn {
                user_code: "8F0B-1F01".to_string(),
                verification_uri: "https://github.com/login/device".to_string(),
            };
            view.github_auth_popover_open = true;
            view.tasks
                .set_auth_task(cx.spawn(async move |_view, _cx| {}));

            view.dismiss_github_auth_popover(cx);

            assert_eq!(
                view.auth_status,
                GitHubAuthStatus::SigningIn {
                    user_code: "8F0B-1F01".to_string(),
                    verification_uri: "https://github.com/login/device".to_string(),
                }
            );
            assert!(!view.github_auth_popover_open());
            assert!(view.tasks.has_auth_task());
            assert_eq!(view.status, "Waiting for GitHub authorization");
        });
        view_entity = Some(view.clone());
        Root::new(view, window, cx)
    });

    view_entity
        .expect("test AppView should be created")
        .read_with(cx, |view, _| {
            assert!(matches!(
                view.auth_status,
                GitHubAuthStatus::SigningIn { .. }
            ));
        });
}

#[gpui::test]
async fn github_account_settings_open_and_close(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_component::init(cx);
        Theme::change(ThemeMode::Dark, None, cx);
    });

    let mut view_entity = None;
    let (_, cx) = cx.add_window_view(|window, cx| {
        let view = cx.new(|cx| AppView::new_without_startup_tasks(window, cx));
        view.update(cx, |view, cx| {
            view.github_auth_popover_open = true;
            view.open_settings(&OpenSettings, window, cx);

            assert!(view.settings_open());
            assert_eq!(view.settings_section(), SettingsSection::GitHub);
            assert!(!view.github_auth_popover_open);

            view.close_settings(&CloseSettings, window, cx);
            assert!(!view.settings_open());
        });
        view_entity = Some(view.clone());
        Root::new(view, window, cx)
    });

    view_entity
        .expect("test AppView should be created")
        .read_with(cx, |view, _| {
            assert!(!view.settings_open());
        });
}

#[gpui::test]
async fn pending_auth_switch_preserves_current_source(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_component::init(cx);
        Theme::change(ThemeMode::Dark, None, cx);
    });

    let mut view_entity = None;
    let (_, cx) = cx.add_window_view(|window, cx| {
        let view = cx.new(|cx| AppView::new_without_startup_tasks(window, cx));
        view.update(cx, |view, _| {
            view.auth_status = GitHubAuthStatus::SignedIn {
                login: Some("octocat".to_string()),
                source: GitHubAuthSource::GhCli,
            };
            view.auth_switch_status = Some(AuthSwitchStatus::StartingOAuth);

            assert_eq!(
                view.current_github_auth_source(),
                Some(GitHubAuthSource::GhCli)
            );
            assert_eq!(
                view.auth_switch_status(),
                Some(&AuthSwitchStatus::StartingOAuth)
            );
        });
        view_entity = Some(view.clone());
        Root::new(view, window, cx)
    });

    view_entity
        .expect("test AppView should be created")
        .read_with(cx, |view, _| {
            assert_eq!(
                view.current_github_auth_source(),
                Some(GitHubAuthSource::GhCli)
            );
        });
}
