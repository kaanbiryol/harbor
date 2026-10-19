use std::sync::Arc;

use gpui::{AppContext, Modifiers, MouseButton, TestAppContext, VisualTestContext, point, size};
use harbor_domain::{PullRequestCommit, RepoId};

use super::*;
use crate::{
    gpui_component::Root,
    test_fixtures::{pull_request, test_time},
    workspace::{
        GitHubAuthSource, GitHubAuthStatus,
        test_support::{FakeGitHubApi, init_test_theme, open_workspace_window},
    },
};

#[gpui::test]
async fn pull_request_header_spans_details_and_panel(cx: &mut TestAppContext) {
    init_test_theme(cx);

    let window = cx.open_window(size(px(1012.), px(700.)), |window, cx| {
        let view = cx
            .new(|cx| AppView::new_with_github_api(Arc::new(FakeGitHubApi::default()), window, cx));
        view.update(cx, |view, cx| {
            let mut pull_request = pull_request();
            pull_request.title =
                "Keep a deliberately long pull request title compact across narrow workspace layouts"
                    .to_string();
            pull_request.head_ref =
                "test/verify-long-pull-request-header-layout-across-narrow-workspaces".to_string();
            view.pull_requests = vec![pull_request];
            view.active_tab = PanelTab::Diff;
            view.pull_request_inbox.set_visible(false);
            cx.notify();
        });
        Root::new(view, window, cx)
    });
    let mut visual_context = VisualTestContext::from_window(window.into(), cx);
    visual_context.run_until_parked();
    let cx = &mut visual_context;

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
    let metadata = cx
        .debug_bounds("pull-request-header-metadata")
        .expect("pull request metadata should render");

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
    assert!(
        tabs.origin.y <= metadata.origin.y + metadata.size.height + px(2.0),
        "tabs should stay close to the pull request metadata"
    );
}

#[gpui::test]
async fn panel_tab_height_stays_stable_when_counts_load(cx: &mut TestAppContext) {
    let (view_entity, cx) = open_workspace_window(cx, |window, cx| {
        let view = cx
            .new(|cx| AppView::new_with_github_api(Arc::new(FakeGitHubApi::default()), window, cx));
        view.update(cx, |view, cx| {
            view.pull_requests = vec![pull_request()];
            view.active_tab = PanelTab::Overview;
            cx.notify();
        });
        view
    });

    cx.refresh().expect("test window should refresh");
    let initial_height = cx
        .debug_bounds("pull-request-panel-tabs")
        .expect("pull request tabs should render")
        .size
        .height;

    view_entity.update(cx, |view, cx| {
        view.pull_requests[0].unresolved_threads = 1;
        view.detail_state.replace_commits(vec![PullRequestCommit {
            sha: "abc123".to_string(),
            message: "Add stable tab count layout".to_string(),
            author: "octocat".to_string(),
            author_avatar_url: None,
            authored_at: Some(test_time()),
        }]);
        view.detail_state.apply_commits_success();
        cx.notify();
    });
    cx.refresh()
        .expect("test window should refresh after counts load");
    let loaded_height = cx
        .debug_bounds("pull-request-panel-tabs")
        .expect("pull request tabs should remain rendered")
        .size
        .height;

    assert_eq!(initial_height, loaded_height);
}

#[gpui::test]
async fn pull_request_columns_resize_horizontally(cx: &mut TestAppContext) {
    let (_view, cx) = open_workspace_window(cx, |window, cx| {
        let view = cx
            .new(|cx| AppView::new_with_github_api(Arc::new(FakeGitHubApi::default()), window, cx));
        view.update(cx, |view, cx| {
            view.pull_requests = vec![pull_request()];
            view.active_tab = PanelTab::Diff;
            cx.notify();
        });
        view
    });

    cx.refresh().expect("test window should refresh");

    let inbox_before = cx
        .debug_bounds("pull-request-inbox")
        .expect("pull request inbox should render");
    let inbox_splitter = point(inbox_before.right(), inbox_before.center().y);
    let wider_inbox_splitter = point(inbox_splitter.x + px(80.0), inbox_splitter.y);
    cx.simulate_mouse_down(inbox_splitter, MouseButton::Left, Modifiers::none());
    cx.simulate_mouse_move(
        point(inbox_splitter.x + px(10.0), inbox_splitter.y),
        MouseButton::Left,
        Modifiers::none(),
    );
    cx.simulate_mouse_move(wider_inbox_splitter, MouseButton::Left, Modifiers::none());
    cx.simulate_mouse_up(wider_inbox_splitter, MouseButton::Left, Modifiers::none());
    cx.refresh()
        .expect("test window should refresh after resizing the inbox");

    let inbox_after = cx
        .debug_bounds("pull-request-inbox")
        .expect("pull request inbox should remain rendered");
    assert!(
        inbox_after.size.width > inbox_before.size.width + px(60.0),
        "dragging the inbox splitter should increase its width"
    );

    let changed_files_before = cx
        .debug_bounds("changed-files-sidebar")
        .expect("changed files should render");
    let changed_files_splitter = point(
        changed_files_before.right(),
        changed_files_before.center().y,
    );
    let wider_changed_files_splitter = point(
        changed_files_splitter.x + px(80.0),
        changed_files_splitter.y,
    );
    cx.simulate_mouse_down(changed_files_splitter, MouseButton::Left, Modifiers::none());
    cx.simulate_mouse_move(
        point(
            changed_files_splitter.x + px(10.0),
            changed_files_splitter.y,
        ),
        MouseButton::Left,
        Modifiers::none(),
    );
    cx.simulate_mouse_move(
        wider_changed_files_splitter,
        MouseButton::Left,
        Modifiers::none(),
    );
    cx.simulate_mouse_up(
        wider_changed_files_splitter,
        MouseButton::Left,
        Modifiers::none(),
    );
    cx.refresh()
        .expect("test window should refresh after resizing changed files");

    let changed_files_after = cx
        .debug_bounds("changed-files-sidebar")
        .expect("changed files should remain rendered");
    assert!(
        changed_files_after.size.width > changed_files_before.size.width + px(60.0),
        "dragging the changed-files splitter should increase its width"
    );
}

#[gpui::test]
async fn non_diff_panel_uses_full_workspace_width(cx: &mut TestAppContext) {
    let (_view, cx) = open_workspace_window(cx, |window, cx| {
        let view = cx
            .new(|cx| AppView::new_with_github_api(Arc::new(FakeGitHubApi::default()), window, cx));
        view.update(cx, |view, cx| {
            view.pull_requests = vec![pull_request()];
            view.active_tab = PanelTab::Checks;
            cx.notify();
        });
        view
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
    let (_view, cx) = open_workspace_window(cx, |window, cx| {
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
        view
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
async fn native_stack_popover_renders_with_virtualized_rows(cx: &mut TestAppContext) {
    use harbor_domain::{MergeQueueState, PullRequestStack, PullRequestStackMembership};
    let api = Arc::new(FakeGitHubApi::default());
    let (_entity, cx) = open_workspace_window(cx, |window, cx| {
        let view = cx.new(|cx| AppView::new_with_github_api(api, window, cx));
        view.update(cx, |view, cx| {
            let members = (1..=100)
                .map(|number| {
                    let mut member = pull_request();
                    member.number = number;
                    member.stack = Some(PullRequestStackMembership {
                        number: 101,
                        position: number as usize,
                        size: 100,
                        base_ref: "main".into(),
                    });
                    member.merge_capabilities.queue_state = MergeQueueState::Enabled;
                    member.merge_capabilities.viewer_can_queue = true;
                    member
                })
                .collect::<Vec<_>>();
            view.pull_requests = vec![members.last().unwrap().clone()];
            view.detail_state.stack = Some(Arc::new(PullRequestStack {
                number: 101,
                base_ref: "main".into(),
                pull_requests: members,
            }));
            view.detail_state.stack_load.succeed();
            view.detail_state.stack_expanded = true;
            view.active_tab = PanelTab::Overview;
            cx.notify();
        });
        view
    });
    cx.refresh().expect("stack popover should render");
    let content = cx
        .debug_bounds("pull-request-stack-content")
        .expect("native stack popover should be visible");
    assert!(
        content.size.height < px(800.),
        "100 stack members should render in a bounded virtual list"
    );
    assert!(content.size.width <= px(600.));
}

#[gpui::test]
async fn stack_rows_keep_titles_and_statuses_inside_the_popover(cx: &mut TestAppContext) {
    use harbor_domain::{MergeQueueState, PullRequestStack, PullRequestStackMembership};
    let (_entity, cx) = open_workspace_window(cx, |window, cx| {
        let view = cx
            .new(|cx| AppView::new_with_github_api(Arc::new(FakeGitHubApi::default()), window, cx));
        view.update(cx, |view, cx| {
            let members = (1..=2).map(|number| {
                let mut member = pull_request();
                member.number = number;
                member.title = if number == 2 { "Replace ImmediateClock".into() } else { "Fix the Share map first-fix test flaking on loaded CI runners and retain determinism across all of the supported configurations".into() };
                member.head_ref = "fix/long-branch-name-that-must-truncate-without-moving-the-readiness-label".into();
                member.stack = Some(PullRequestStackMembership { number: 3, position: number as usize, size: 2, base_ref: "main".into() });
                member.merge_capabilities.queue_state = MergeQueueState::Enabled;
                member.checks_summary.failed = 1;
                member
            }).collect::<Vec<_>>();
            view.pull_requests = vec![members[1].clone()];
            view.detail_state.stack = Some(Arc::new(PullRequestStack { number: 3, base_ref: "main".into(), pull_requests: members }));
            view.detail_state.stack_load.succeed();
            view.detail_state.stack_expanded = true;
            view.pull_request_inbox.set_visible(false);
            view.active_tab = PanelTab::Overview;
            cx.notify();
        });
        view
    });
    for viewport_width in [900., 420.] {
        cx.simulate_resize(size(px(viewport_width), px(700.)));
        cx.refresh().expect("stack popover should render");
        let content = cx.debug_bounds("pull-request-stack-content").unwrap();
        let first = cx.debug_bounds("stack-pr-row-2").unwrap();
        let second = cx.debug_bounds("stack-pr-row-1").unwrap();
        let first_status = cx.debug_bounds("stack-pr-status-2").unwrap();
        let second_status = cx.debug_bounds("stack-pr-status-1").unwrap();
        let long_title = cx.debug_bounds("stack-pr-title-1").unwrap();
        assert_eq!(
            first.size.width, second.size.width,
            "stack row widths must not depend on their titles"
        );
        assert!(
            first.size.width >= content.size.width - px(20.),
            "selection must span the popover width: {first:?}, {content:?}"
        );
        assert_eq!(
            first_status.right(),
            second_status.right(),
            "readiness labels must align"
        );
        assert!(
            second_status.right() <= content.right() - px(8.),
            "readiness must stay inside the popover"
        );
        assert!(
            long_title.right() < second_status.origin.x,
            "long titles must truncate before readiness labels"
        );
        assert!(
            content.origin.x >= px(0.) && content.right() <= px(viewport_width),
            "stack popover must fit inside the window: {content:?}"
        );
    }
}
