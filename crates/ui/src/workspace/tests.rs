use std::sync::Arc;

use super::*;
use crate::{
    actions::{CloseSettings, OpenSettings, ToggleRepositorySwitcher},
    test_fixtures::pull_request,
    workspace::github_service::test_support::FakeGitHubApi,
};
use gpui::{AppContext, TestAppContext, px};
use gpui_component::{Root, Theme, ThemeMode};
use harbor_domain::Label;

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

    let (_, cx) = cx.add_window_view(|window, cx| {
        let view = cx
            .new(|cx| AppView::new_with_github_api(Arc::new(FakeGitHubApi::default()), window, cx));
        view.update(cx, |view, cx| {
            let mut pull_request = pull_request();
            pull_request.body = Some("## summary\n\n- Adds a focused fixture".to_string());
            pull_request.unresolved_threads = 5;
            pull_request.labels = vec![Label {
                name: "needs-review".to_string(),
                color: Some("34d399".to_string()),
            }];
            view.pull_requests = vec![pull_request];
            view.active_tab = PanelTab::Overview;
            cx.notify();
        });
        Root::new(view, window, cx)
    });

    cx.refresh().expect("test window should refresh");
    assert!(cx.debug_bounds("pull-request-overview-panel").is_some());
    assert!(
        cx.debug_bounds("pull-request-overview-description")
            .is_some()
    );
    assert!(cx.debug_bounds("pull-request-overview-sidebar").is_some());
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
