use std::sync::Arc;

use crate::gpui_component::{Root, Theme, ThemeMode};
use gpui::{
    AppContext, ListAlignment, ListOffset, ListState, Modifiers, ScrollDelta, ScrollWheelEvent,
    TestAppContext, point, px,
};
use harbor_domain::{
    Label, MergeState, PullRequestComment, PullRequestCommit, PullRequestReview,
    PullRequestReviewState, ReviewDecision, ReviewThreadState,
};

use super::{
    OverviewDescriptionItems, OverviewPanelItem, merge_readiness, overview_panel_items,
    overview_thread_expanded, overview_thread_item_index, pull_request_readiness,
    sync_overview_list_items,
};
use crate::actions::PanelTab;
use crate::test_fixtures::{pull_request, review_thread, test_time};
use crate::visual::Tone;
use crate::workspace::{
    AppView,
    test_support::{FakeGitHubApi, open_workspace_window},
};

#[test]
fn clean_merge_state_still_requires_approval_and_resolved_conversations() {
    let mut pull_request = pull_request();
    pull_request.merge_state = Some(MergeState::Clean);
    pull_request.review_decision = None;
    pull_request.unresolved_threads = 5;

    assert_eq!(merge_readiness(&pull_request).0, "Blocked");
    assert_eq!(pull_request_readiness(&pull_request).0, "Review required");

    pull_request.review_decision = Some(ReviewDecision::Approved);
    assert_eq!(merge_readiness(&pull_request).0, "Blocked");
    assert_eq!(
        pull_request_readiness(&pull_request),
        (
            "Conversations open",
            "Resolve threads to merge.",
            Tone::Warning,
        )
    );

    pull_request.unresolved_threads = 0;
    assert_eq!(merge_readiness(&pull_request).0, "Ready");
}

#[test]
fn unresolved_threads_start_expanded_and_completed_threads_start_collapsed() {
    assert!(overview_thread_expanded(
        ReviewThreadState::Unresolved,
        None
    ));
    assert!(!overview_thread_expanded(ReviewThreadState::Resolved, None));
    assert!(!overview_thread_expanded(ReviewThreadState::Outdated, None));
    assert!(overview_thread_expanded(
        ReviewThreadState::Resolved,
        Some(true)
    ));
    assert!(!overview_thread_expanded(
        ReviewThreadState::Unresolved,
        Some(false)
    ));
}

#[test]
fn finds_thread_index_in_virtual_overview_items() {
    let thread = review_thread(ReviewThreadState::Unresolved);
    let items = overview_panel_items(
        OverviewDescriptionItems::Empty,
        &[],
        &[],
        &[],
        &[thread],
        None,
    );

    assert_eq!(overview_thread_item_index(&items, "thread-1"), Some(3));
    assert_eq!(overview_thread_item_index(&items, "missing"), None);
}

#[test]
fn preserves_scroll_anchor_when_timeline_item_is_inserted_above() {
    let list_state = ListState::new(3, ListAlignment::Top, px(160.0));
    list_state.scroll_to(ListOffset {
        item_ix: 1,
        offset_in_item: px(18.0),
    });
    let mut previous_keys = vec![
        "comment:1".to_string(),
        "thread:1".to_string(),
        "composer".to_string(),
    ];
    let next_keys = vec![
        "review:1".to_string(),
        "comment:1".to_string(),
        "thread:1".to_string(),
        "composer".to_string(),
    ];

    sync_overview_list_items(&list_state, &mut previous_keys, next_keys.clone());

    assert_eq!(previous_keys, next_keys);
    assert_eq!(list_state.item_count(), 4);
    assert_eq!(list_state.logical_scroll_top().item_ix, 2);
    assert_eq!(list_state.logical_scroll_top().offset_in_item, px(18.0));
}

#[test]
fn overview_rows_include_description_and_activity_in_one_list() {
    let keys = overview_panel_items(OverviewDescriptionItems::Empty, &[], &[], &[], &[], None)
        .iter()
        .map(OverviewPanelItem::key)
        .collect::<Vec<_>>();

    assert_eq!(
        keys,
        vec![
            "description:header",
            "description:empty",
            "activity:header",
            "activity:empty",
            "composer"
        ]
    );
}

#[test]
fn description_markdown_blocks_are_individual_overview_rows() {
    let keys = overview_panel_items(
        OverviewDescriptionItems::Blocks(3),
        &[],
        &[],
        &[],
        &[],
        None,
    )
    .iter()
    .map(OverviewPanelItem::key)
    .collect::<Vec<_>>();

    assert_eq!(
        keys,
        vec![
            "description:header",
            "description:block:0",
            "description:block:1",
            "description:block:2",
            "activity:header",
            "activity:empty",
            "composer"
        ]
    );
}

#[gpui::test]
async fn overview_markdown_state_survives_virtual_row_recreation(cx: &mut TestAppContext) {
    cx.update(|cx| {
        crate::gpui_component::init(cx);
        Theme::change(ThemeMode::Dark, None, cx);
    });

    let (_, _) = cx.add_window_view(|window, cx| {
        let view = cx.new(|cx| AppView::new_without_startup_tasks(window, cx));
        view.update(cx, |view, cx| {
            let first =
                view.ensure_overview_markdown_state("comment:1".to_string(), "first body", cx);
            let reused =
                view.ensure_overview_markdown_state("comment:1".to_string(), "first body", cx);
            let updated =
                view.ensure_overview_markdown_state("comment:1".to_string(), "updated body", cx);

            assert_eq!(first.entity_id(), reused.entity_id());
            assert_eq!(first.entity_id(), updated.entity_id());
            assert_eq!(
                view.overview_state.markdown_state_source("comment:1"),
                Some("updated body")
            );
        });
        Root::new(view, window, cx)
    });
}

#[gpui::test]
async fn overview_uses_skeletons_until_initial_data_finishes(cx: &mut TestAppContext) {
    let (_view, cx) = open_workspace_window(cx, |window, cx| {
        let view = cx
            .new(|cx| AppView::new_with_github_api(Arc::new(FakeGitHubApi::default()), window, cx));
        view.update(cx, |view, cx| {
            view.pull_requests = vec![pull_request()];
            view.detail_state.apply_details_success();
            view.active_tab = PanelTab::Overview;
            cx.notify();
        });
        view
    });

    cx.refresh().expect("test window should refresh");
    let description_loading = cx
        .debug_bounds("pull-request-overview-description-loading")
        .expect("description skeleton should render while overview loads");
    let activity_loading = cx
        .debug_bounds("pull-request-overview-activity-loading")
        .expect("activity loader should render");
    assert!(activity_loading.size.height > px(28.0));
    assert!(activity_loading.size.height <= px(112.0));
    assert!(
        activity_loading.origin.y >= description_loading.origin.y + description_loading.size.height
    );
    assert!(
        activity_loading.origin.y
            <= description_loading.origin.y + description_loading.size.height + px(16.0)
    );
    assert!(
        cx.debug_bounds("pull-request-overview-activity-loading-row-1")
            .is_some()
    );
    assert!(
        cx.debug_bounds("pull-request-overview-activity-header")
            .is_none()
    );
    assert!(cx.debug_bounds("pull-request-overview-panel").is_some());
    assert!(
        cx.debug_bounds("pull-request-overview-description")
            .is_none()
    );
    assert!(
        cx.debug_bounds("pull-request-overview-sidebar-loading")
            .is_some()
    );
    assert!(cx.debug_bounds("pull-request-overview-sidebar").is_none());
    assert!(
        cx.debug_bounds("pull-request-overview-comment-input")
            .is_none()
    );
}

#[gpui::test]
async fn overview_reveals_content_together_after_pull_request_switch(cx: &mut TestAppContext) {
    let (view_entity, cx) = open_workspace_window(cx, |window, cx| {
        let view = cx
            .new(|cx| AppView::new_with_github_api(Arc::new(FakeGitHubApi::default()), window, cx));
        view.update(cx, |view, cx| {
            let mut first_pull_request = pull_request();
            first_pull_request.body = Some("First description".to_string());
            let mut second_pull_request = pull_request();
            second_pull_request.number = 8;
            second_pull_request.head_sha = "def456".to_string();
            view.pull_requests = vec![first_pull_request, second_pull_request];
            view.detail_state.apply_details_success();
            view.detail_state.apply_commits_success();
            view.review_state.apply_reviews_success();
            view.active_tab = PanelTab::Overview;
            cx.notify();
        });
        view
    });

    cx.refresh().expect("test window should refresh");
    cx.run_until_parked();
    cx.refresh()
        .expect("test window should refresh after description parsing");
    assert!(
        cx.debug_bounds("pull-request-overview-description")
            .is_some()
    );
    assert!(
        cx.debug_bounds("pull-request-overview-activity-header")
            .is_some()
    );

    view_entity.update(cx, |view, cx| {
        view.cache_current_pull_request_detail_snapshot();
        view.overview_state.cache_current_content();
        view.selection_state.set_pull_request_index(1);
        view.clear_selected_pull_request_detail_state();
        view.detail_state.start_details_load();
        view.detail_state.apply_commits_success();
        view.review_state.apply_reviews_success();
        cx.notify();
    });

    cx.refresh()
        .expect("test window should refresh after switching pull requests");
    assert!(
        cx.debug_bounds("pull-request-overview-description")
            .is_none()
    );
    assert!(
        cx.debug_bounds("pull-request-overview-description-loading")
            .is_some()
    );
    assert!(cx.debug_bounds("pull-request-overview-activity").is_some());
    assert!(
        cx.debug_bounds("pull-request-overview-activity-loading-row-1")
            .is_some()
    );
    assert!(
        cx.debug_bounds("pull-request-overview-sidebar-loading")
            .is_some()
    );
    assert!(cx.debug_bounds("pull-request-merge-readiness").is_none());

    view_entity.update(cx, |view, cx| {
        view.pull_requests[1].body = Some(
            "## Second description\n\nThis content arrives with pull request metadata.".to_string(),
        );
        view.detail_state.apply_details_success();
        cx.notify();
    });

    cx.refresh()
        .expect("test window should refresh after metadata loads");
    assert!(
        cx.debug_bounds("pull-request-overview-description-loading")
            .is_some()
    );
    assert!(
        cx.debug_bounds("pull-request-overview-activity-header")
            .is_none()
    );

    cx.run_until_parked();
    cx.refresh()
        .expect("test window should refresh after description parsing");
    assert!(
        cx.debug_bounds("pull-request-overview-description")
            .is_some()
    );
    assert!(
        cx.debug_bounds("pull-request-overview-description-loading")
            .is_none()
    );
    assert!(
        cx.debug_bounds("pull-request-overview-activity-header")
            .is_some()
    );
    let description = cx
        .debug_bounds("pull-request-overview-description")
        .expect("parsed description should render before activity");
    let activity_header = cx
        .debug_bounds("pull-request-overview-activity-header")
        .expect("activity should render after description parsing");
    assert!(
        activity_header.origin.y >= description.origin.y + description.size.height,
        "activity should start below the fully laid out description"
    );
    assert!(
        cx.debug_bounds("pull-request-overview-sidebar-loading")
            .is_none()
    );
    assert!(cx.debug_bounds("pull-request-merge-readiness").is_some());

    view_entity.update(cx, |view, cx| {
        view.select_pull_request(0, cx);
    });
    cx.refresh()
        .expect("cached overview should render without another parsing turn");
    assert!(
        cx.debug_bounds("pull-request-overview-description-loading")
            .is_none()
    );
    assert!(
        cx.debug_bounds("pull-request-overview-description")
            .is_some()
    );
    assert!(
        cx.debug_bounds("pull-request-overview-activity-header")
            .is_some()
    );

    view_entity.update(cx, |view, cx| {
        view.select_pull_request(1, cx);
    });
    cx.refresh()
        .expect("second cached overview should render without another parsing turn");
    assert!(
        cx.debug_bounds("pull-request-overview-description-loading")
            .is_none()
    );
    assert!(
        cx.debug_bounds("pull-request-overview-description")
            .is_some()
    );
    assert!(
        cx.debug_bounds("pull-request-overview-activity-header")
            .is_some()
    );
}

#[gpui::test]
async fn long_overview_descriptions_share_the_virtualized_overview_scroll(cx: &mut TestAppContext) {
    let (view_entity, cx) = open_workspace_window(cx, |window, cx| {
        let view = cx
            .new(|cx| AppView::new_with_github_api(Arc::new(FakeGitHubApi::default()), window, cx));
        view.update(cx, |view, cx| {
            let mut pull_request = pull_request();
            pull_request.body = Some("description paragraph\n\n".repeat(400));
            view.pull_requests = vec![pull_request];
            view.detail_state.apply_details_success();
            view.detail_state.apply_commits_success();
            view.review_state.apply_reviews_success();
            view.active_tab = PanelTab::Overview;
            cx.notify();
        });
        view
    });

    cx.refresh().expect("test window should refresh");
    cx.run_until_parked();
    cx.refresh()
        .expect("test window should refresh after description parsing");

    assert!(
        cx.debug_bounds("pull-request-overview-description-block-0")
            .is_some(),
        "the first visible markdown block should render"
    );
    assert!(
        cx.debug_bounds("pull-request-overview-description-block-100")
            .is_none(),
        "offscreen markdown blocks should not render"
    );
    assert!(
        cx.debug_bounds("pull-request-overview-activity-header")
            .is_none(),
        "activity should follow the full description in the same list"
    );

    view_entity.update(cx, |view, cx| {
        assert!(view.overview_state.description_block_count() > 100);
        view.overview_state.list_state.scroll_to(gpui::ListOffset {
            item_ix: 101,
            offset_in_item: px(0.0),
        });
        cx.notify();
    });
    cx.refresh()
        .expect("test window should refresh after scrolling the description");

    assert!(
        cx.debug_bounds("pull-request-overview-description-block-100")
            .is_some(),
        "scrolling the overview should reveal later markdown blocks"
    );
    assert!(
        cx.debug_bounds("pull-request-overview-description")
            .is_none(),
        "the description header should leave the viewport with the shared scroll"
    );
}

#[gpui::test]
async fn overview_panel_renders_description_and_editable_metadata(cx: &mut TestAppContext) {
    let (view_entity, cx) = open_workspace_window(cx, |window, cx| {
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
            view.detail_state.apply_details_success();
            view.detail_state.replace_commits(vec![PullRequestCommit {
                sha: "abc123".to_string(),
                message: "Add commit count".to_string(),
                author: "octocat".to_string(),
                author_avatar_url: None,
                authored_at: Some(test_time()),
            }]);
            view.detail_state.apply_commits_success();
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
                        author_avatar_url: None,
                        state: PullRequestReviewState::Approved,
                        body: None,
                        submitted_at: Some(test_time()),
                    },
                    PullRequestReview {
                        id: "review-empty".to_string(),
                        node_id: Some("review-node-empty".to_string()),
                        author: "reviewer".to_string(),
                        author_avatar_url: None,
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
            view.review_state.apply_reviews_success();
            view.active_tab = PanelTab::Overview;
            cx.notify();
        });
        view
    });

    cx.refresh().expect("test window should refresh");
    cx.run_until_parked();
    cx.refresh()
        .expect("test window should refresh after description parsing");
    assert!(cx.debug_bounds("pull-request-overview-panel").is_some());
    let description = cx
        .debug_bounds("pull-request-overview-description")
        .expect("overview description should render");
    assert!(
        cx.debug_bounds("pull-request-overview-activity-header")
            .is_some()
    );
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
    view_entity.read_with(cx, |view, _| {
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
    assert!(cx.debug_bounds("commits-tab-count").is_some());
    let author_chip = cx
        .debug_bounds("pull-request-person-octocat")
        .expect("pull request author chip should render");
    assert!(author_chip.size.width > px(40.));
    assert!(cx.debug_bounds("pull-request-person-reviewer").is_some());
    assert!(
        cx.debug_bounds("pull-request-review-status-reviewer")
            .is_some()
    );
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
async fn overview_description_and_activity_scroll_together(cx: &mut TestAppContext) {
    let (_view, cx) = open_workspace_window(cx, |window, cx| {
        let view = cx
            .new(|cx| AppView::new_with_github_api(Arc::new(FakeGitHubApi::default()), window, cx));
        view.update(cx, |view, cx| {
            view.pull_requests = vec![pull_request()];
            view.detail_state.apply_details_success();
            view.detail_state.replace_commits(
                (0..24)
                    .map(|index| PullRequestCommit {
                        sha: format!("commit-{index}"),
                        message: format!("Timeline commit {index}"),
                        author: "octocat".to_string(),
                        author_avatar_url: None,
                        authored_at: Some(test_time()),
                    })
                    .collect(),
            );
            view.detail_state.apply_commits_success();
            view.review_state.apply_reviews_success();
            view.active_tab = PanelTab::Overview;
            cx.notify();
        });
        view
    });

    cx.refresh().expect("test window should refresh");
    let timeline = cx
        .debug_bounds("pull-request-overview-timeline")
        .expect("overview timeline should render");
    let description_before_scroll = cx
        .debug_bounds("pull-request-overview-description")
        .expect("overview description should render");

    cx.simulate_event(ScrollWheelEvent {
        position: timeline.center(),
        delta: ScrollDelta::Pixels(point(px(0.0), px(-400.0))),
        ..Default::default()
    });
    cx.refresh()
        .expect("test window should refresh after scrolling the timeline");

    let description_after_scroll = cx.debug_bounds("pull-request-overview-description");
    assert!(
        description_after_scroll.is_none_or(|description| {
            description.origin.y < description_before_scroll.origin.y
        }),
        "scrolling activity should also move the description"
    );
}

#[gpui::test]
async fn overview_sidebar_scrolls_independently(cx: &mut TestAppContext) {
    let (_view, cx) = open_workspace_window(cx, |window, cx| {
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
            view.detail_state.apply_details_success();
            view.detail_state.apply_commits_success();
            view.review_state.apply_reviews_success();
            view.active_tab = PanelTab::Overview;
            cx.notify();
        });
        view
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
