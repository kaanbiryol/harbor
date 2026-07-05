use super::*;
use crate::actions::{CloseSettings, OpenSettings, ToggleRepositorySwitcher};
use gpui::{AppContext, TestAppContext, px};
use gpui_component::{Root, Theme, ThemeMode};

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
async fn fullscreen_repository_title_starts_at_left_edge(cx: &mut TestAppContext) {
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
    assert_eq!(
        bounds.origin.x,
        px(0.),
        "fullscreen repository title should start at the left edge"
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
