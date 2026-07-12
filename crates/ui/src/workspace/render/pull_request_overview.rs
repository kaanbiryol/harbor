use gpui::{AnyElement, Context, IntoElement, div, list, prelude::*, px};
use gpui_component::{
    Disableable, Sizable, StyledExt,
    button::{Button, ButtonVariants},
    input::Input,
};
use harbor_domain::PullRequest;

use crate::{panels::overview_markdown_body, visual::color, workspace::AppView};

const OVERVIEW_SIDEBAR_WIDTH: f32 = 280.0;

#[path = "pull_request_overview/activity.rs"]
mod activity;
#[path = "pull_request_overview/description.rs"]
mod description;
#[path = "pull_request_overview/events.rs"]
mod events;
#[path = "pull_request_overview/model.rs"]
mod model;
#[path = "pull_request_overview/readiness.rs"]
mod readiness;
#[path = "pull_request_overview/sidebar.rs"]
mod sidebar;
#[path = "pull_request_overview/timeline.rs"]
mod timeline;

use events::*;
use model::*;
use sidebar::*;
use timeline::*;

impl AppView {
    pub(super) fn render_pull_request_overview_panel(
        &mut self,
        pr: Option<&PullRequest>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let Some(pr) = pr else {
            return div()
                .text_sm()
                .text_color(color::text_muted())
                .child("Select a pull request to see its overview")
                .into_any_element();
        };
        let panel_items = overview_panel_items(
            self.review_state.pull_request_reviews(),
            self.review_state.pull_request_comments(),
            self.review_state.review_threads(),
            self.review_state.reviews_loading(),
            self.review_state.reviews_error(),
        );
        let panel_item_keys = panel_items.iter().map(OverviewPanelItem::key).collect();
        sync_overview_list_items(
            &self.overview_state.list_state,
            &mut self.overview_state.list_item_keys,
            panel_item_keys,
        );
        let panel_items_for_render = panel_items.clone();

        div()
            .debug_selector(|| "pull-request-overview-panel".to_string())
            .image_cache(gpui::retain_all("pull-request-overview-avatar-cache"))
            .flex_1()
            .min_h_0()
            .min_w_0()
            .overflow_hidden()
            .child(
                div()
                    .h_full()
                    .min_h_0()
                    .flex()
                    .items_stretch()
                    .gap_3()
                    .w_full()
                    .min_w_0()
                    .child(
                        div()
                            .debug_selector(|| "pull-request-overview-timeline".to_string())
                            .flex_1()
                            .min_h_0()
                            .min_w_0()
                            .child(
                                list(
                                    self.overview_state.list_state.clone(),
                                    cx.processor(move |view, index: usize, _window, cx| {
                                        let Some(item) = panel_items_for_render.get(index) else {
                                            return div().into_any_element();
                                        };

                                        match item {
                                            OverviewPanelItem::Description => view
                                                .selected_pull_request()
                                                .cloned()
                                                .map(|pr| {
                                                    div()
                                                        .w_full()
                                                        .pb_3()
                                                        .child(
                                                            view.render_description_card(&pr, cx),
                                                        )
                                                        .into_any_element()
                                                })
                                                .unwrap_or_else(|| div().into_any_element()),
                                            OverviewPanelItem::Comment { id } => view
                                                .review_state
                                                .pull_request_comments()
                                                .iter()
                                                .find(|comment| comment.id == *id)
                                                .cloned()
                                                .map(|comment| {
                                                    let markdown = view.render_overview_markdown(
                                                        format!(
                                                            "overview-comment-body-{}",
                                                            comment.id
                                                        ),
                                                        &comment.body,
                                                        cx,
                                                    );
                                                    render_overview_comment_event(
                                                        &comment, index, markdown,
                                                    )
                                                })
                                                .unwrap_or_else(|| div().into_any_element()),
                                            OverviewPanelItem::Review { id } => view
                                                .review_state
                                                .pull_request_reviews()
                                                .iter()
                                                .find(|review| review.id == *id)
                                                .cloned()
                                                .map(|review| {
                                                    let markdown = review
                                                        .body
                                                        .as_deref()
                                                        .map(str::trim)
                                                        .filter(|body| !body.is_empty())
                                                        .map(|body| {
                                                            view.render_overview_markdown(
                                                                format!(
                                                                    "overview-review-body-{}",
                                                                    review.id
                                                                ),
                                                                body,
                                                                cx,
                                                            )
                                                        });
                                                    render_overview_review_event(
                                                        &review, index, markdown,
                                                    )
                                                })
                                                .unwrap_or_else(|| div().into_any_element()),
                                            OverviewPanelItem::Thread { id } => view
                                                .review_state
                                                .review_threads()
                                                .iter()
                                                .find(|thread| thread.id == *id)
                                                .cloned()
                                                .map(|thread| {
                                                    let expanded = overview_thread_expanded(
                                                        thread.state,
                                                        view.overview_state
                                                            .thread_expansion_overrides
                                                            .get(&thread.id)
                                                            .copied(),
                                                    );
                                                    view.render_overview_thread_event(
                                                        &thread, index, expanded, cx,
                                                    )
                                                })
                                                .unwrap_or_else(|| div().into_any_element()),
                                            OverviewPanelItem::Message(message) => {
                                                render_timeline_message(message)
                                            }
                                            OverviewPanelItem::Composer => view
                                                .selected_pull_request()
                                                .cloned()
                                                .map(|pr| {
                                                    view.render_overview_comment_composer(&pr, cx)
                                                })
                                                .unwrap_or_else(|| div().into_any_element()),
                                        }
                                    }),
                                )
                                .size_full(),
                            ),
                    )
                    .child(
                        div()
                            .id("pull-request-overview-sidebar-scroll")
                            .debug_selector(|| "pull-request-overview-sidebar".to_string())
                            .w(px(OVERVIEW_SIDEBAR_WIDTH))
                            .h_full()
                            .min_h_0()
                            .flex_none()
                            .flex()
                            .flex_col()
                            .gap_3()
                            .overflow_y_scroll()
                            .child(self.render_merge_readiness_card(pr, cx))
                            .child(self.render_people_card(pr, cx))
                            .child(self.render_labels_card(pr, cx)),
                    ),
            )
            .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use chrono::Duration;
    use gpui::{AppContext, ListAlignment, ListOffset, ListState, TestAppContext, px};
    use gpui_component::{Root, Theme, ThemeMode};
    use harbor_domain::{
        MergeState, PullRequestComment, PullRequestReview, PullRequestReviewState, ReviewDecision,
        ReviewThreadState,
    };

    use super::{
        OverviewTimelineItem, merge_readiness, overview_panel_items, overview_review_visible,
        overview_thread_expanded, overview_thread_item_index, overview_timeline_items,
        parse_label_color, pull_request_readiness, sync_overview_list_items,
    };
    use crate::test_fixtures::{pull_request, review_thread, test_time};
    use crate::visual::Tone;
    use crate::workspace::AppView;

    #[test]
    fn parses_github_label_colors() {
        assert!(parse_label_color("34d399").is_some());
        assert!(parse_label_color("#34d399").is_some());
        assert!(parse_label_color("bad").is_none());
        assert!(parse_label_color("zzzzzz").is_none());
    }

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
    fn orders_timeline_activity_and_hides_pending_reviews() {
        let time = test_time();
        let comments = vec![PullRequestComment {
            id: "comment".to_string(),
            author: "octocat".to_string(),
            author_avatar_url: None,
            body: "comment".to_string(),
            created_at: time + Duration::minutes(2),
            updated_at: None,
        }];
        let reviews = vec![
            PullRequestReview {
                id: "submitted".to_string(),
                node_id: None,
                author: "reviewer".to_string(),
                state: PullRequestReviewState::Approved,
                body: None,
                submitted_at: Some(time + Duration::minutes(1)),
            },
            PullRequestReview {
                id: "pending".to_string(),
                node_id: None,
                author: "reviewer".to_string(),
                state: PullRequestReviewState::Pending,
                body: None,
                submitted_at: Some(time),
            },
            PullRequestReview {
                id: "empty-commented".to_string(),
                node_id: None,
                author: "reviewer".to_string(),
                state: PullRequestReviewState::Commented,
                body: None,
                submitted_at: Some(time),
            },
        ];
        let mut thread = review_thread(ReviewThreadState::Unresolved);
        thread.comments[0].created_at = time + Duration::minutes(3);
        let threads = vec![thread];

        let items = overview_timeline_items(&reviews, &comments, &threads);

        assert_eq!(items.len(), 3);
        assert!(matches!(items[0], OverviewTimelineItem::Review(_)));
        assert!(matches!(items[1], OverviewTimelineItem::Comment(_)));
        assert!(matches!(items[2], OverviewTimelineItem::Thread(_)));
    }

    #[test]
    fn keeps_only_meaningful_review_events() {
        let review = |state, body: Option<&str>| PullRequestReview {
            id: "review".to_string(),
            node_id: None,
            author: "reviewer".to_string(),
            state,
            body: body.map(str::to_string),
            submitted_at: Some(test_time()),
        };

        assert!(!overview_review_visible(&review(
            PullRequestReviewState::Pending,
            None
        )));
        assert!(!overview_review_visible(&review(
            PullRequestReviewState::Commented,
            None
        )));
        assert!(!overview_review_visible(&review(
            PullRequestReviewState::Commented,
            Some("  \n")
        )));
        assert!(overview_review_visible(&review(
            PullRequestReviewState::Commented,
            Some("Review summary")
        )));
        assert!(overview_review_visible(&review(
            PullRequestReviewState::Approved,
            None
        )));
        assert!(overview_review_visible(&review(
            PullRequestReviewState::ChangesRequested,
            None
        )));
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
        let items = overview_panel_items(&[], &[], &[thread], false, None);

        assert_eq!(overview_thread_item_index(&items, "thread-1"), Some(1));
        assert_eq!(overview_thread_item_index(&items, "missing"), None);
    }

    #[test]
    fn preserves_scroll_anchor_when_timeline_item_is_inserted_above() {
        let list_state = ListState::new(4, ListAlignment::Top, px(160.0));
        list_state.scroll_to(ListOffset {
            item_ix: 2,
            offset_in_item: px(18.0),
        });
        let mut previous_keys = vec![
            "description".to_string(),
            "comment:1".to_string(),
            "thread:1".to_string(),
            "composer".to_string(),
        ];
        let next_keys = vec![
            "description".to_string(),
            "review:1".to_string(),
            "comment:1".to_string(),
            "thread:1".to_string(),
            "composer".to_string(),
        ];

        sync_overview_list_items(&list_state, &mut previous_keys, next_keys.clone());

        assert_eq!(previous_keys, next_keys);
        assert_eq!(list_state.item_count(), 5);
        assert_eq!(list_state.logical_scroll_top().item_ix, 3);
        assert_eq!(list_state.logical_scroll_top().offset_in_item, px(18.0));
    }

    #[test]
    fn keeps_overview_at_top_when_loading_placeholder_is_replaced() {
        let list_state = ListState::new(3, ListAlignment::Top, px(160.0));
        let mut previous_keys = vec![
            "description".to_string(),
            "message:loading".to_string(),
            "composer".to_string(),
        ];
        let next_keys = vec![
            "description".to_string(),
            "comment:1".to_string(),
            "review:1".to_string(),
            "thread:1".to_string(),
            "composer".to_string(),
        ];

        sync_overview_list_items(&list_state, &mut previous_keys, next_keys);

        assert_eq!(list_state.logical_scroll_top().item_ix, 0);
        assert_eq!(list_state.logical_scroll_top().offset_in_item, px(0.0));
    }

    #[gpui::test]
    async fn overview_markdown_state_survives_virtual_row_recreation(cx: &mut TestAppContext) {
        cx.update(|cx| {
            gpui_component::init(cx);
            Theme::change(ThemeMode::Dark, None, cx);
        });

        let (_, _) = cx.add_window_view(|window, cx| {
            let view = cx.new(|cx| AppView::new_without_startup_tasks(window, cx));
            view.update(cx, |view, cx| {
                let first =
                    view.ensure_overview_markdown_state("comment:1".to_string(), "first body", cx);
                let reused =
                    view.ensure_overview_markdown_state("comment:1".to_string(), "first body", cx);
                let updated = view.ensure_overview_markdown_state(
                    "comment:1".to_string(),
                    "updated body",
                    cx,
                );

                assert_eq!(first.entity_id(), reused.entity_id());
                assert_eq!(first.entity_id(), updated.entity_id());
                assert_eq!(
                    view.overview_state.markdown_states["comment:1"].source,
                    "updated body"
                );
            });
            Root::new(view, window, cx)
        });
    }
}
