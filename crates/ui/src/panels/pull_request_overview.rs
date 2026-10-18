use crate::gpui_component::{
    Disableable, Sizable, StyledExt,
    button::{Button, ButtonVariants},
    skeleton::Skeleton,
    spinner::Spinner,
};
use gpui::{AnyElement, Context, IntoElement, div, list, prelude::*, px};
use harbor_domain::PullRequest;

use crate::{icons::Octicon, panels::render_empty_state, visual::color, workspace::AppView};

const OVERVIEW_SIDEBAR_WIDTH: f32 = 280.0;

mod activity;
mod description;
mod events;
mod metadata_picker;
mod model;
mod readiness;
mod sidebar;
mod timeline;

use events::*;
use model::*;
#[cfg(test)]
use readiness::{merge_readiness, pull_request_readiness};
use timeline::*;

impl AppView {
    fn overview_description_items(&self) -> OverviewDescriptionItems {
        if self.pull_request_description_editing {
            return OverviewDescriptionItems::Editor;
        }

        match self.overview_state.description_block_count() {
            0 => OverviewDescriptionItems::Empty,
            block_count => OverviewDescriptionItems::Blocks(block_count),
        }
    }

    pub(crate) fn render_pull_request_overview_panel(
        &mut self,
        pr: Option<&PullRequest>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let Some(pr) = pr else {
            if self.pull_request_workspace_loading() {
                let message = if self.pull_request_inbox.is_loading() {
                    "Loading pull requests…"
                } else {
                    "Opening workspace…"
                };
                return div()
                    .debug_selector(|| "pull-request-workspace-loading".to_string())
                    .flex_1()
                    .min_h_0()
                    .w_full()
                    .flex()
                    .flex_col()
                    .items_center()
                    .justify_center()
                    .gap_3()
                    .child(Spinner::new().large())
                    .child(
                        div()
                            .text_sm()
                            .text_color(color::text_secondary())
                            .child(message),
                    )
                    .into_any_element();
            }

            let has_repository = self.repository_state.has_configured_repo();
            let (icon, title, description) = if !has_repository {
                (
                    Octicon::FileDirectory,
                    "No repository selected",
                    "Choose a repository from the title bar to load its pull requests.",
                )
            } else if let Some(error) = self.pull_request_inbox.load_error() {
                (Octicon::Alert, "Could not load pull requests", error)
            } else if self.pull_requests.is_empty() {
                (
                    Octicon::GitPullRequest,
                    self.pull_request_inbox.mode().empty_message(),
                    "Refresh to check again, or choose another repository from the title bar.",
                )
            } else {
                (
                    Octicon::GitPullRequest,
                    "No pull request selected",
                    "Select a pull request from the list to view its overview, changes, reviews, and checks.",
                )
            };

            return render_empty_state(icon, title, description)
                .debug_selector(|| "pull-request-empty-state".to_string())
                .when(has_repository, |element| {
                    element.child(
                        div().pt_2().child(
                            Button::new("refresh-empty-pull-request-inbox")
                                .icon(Octicon::Sync)
                                .label("refresh")
                                .small()
                                .outline()
                                .loading(self.pull_request_inbox.is_loading())
                                .disabled(self.pull_request_inbox.is_loading())
                                .on_click(cx.listener(|view, _, _, cx| {
                                    view.reload_pull_request_inbox(cx);
                                })),
                        ),
                    )
                })
                .into_any_element();
        };
        let Some(detail_key) = self.selected_pull_request_detail_key() else {
            return render_overview_activity_loading();
        };
        let initial_data_finished = self.detail_state.details_ready()
            && self.detail_state.commits_finished()
            && self.review_state.reviews_finished();
        self.overview_state
            .prepare_pull_request(detail_key.clone(), initial_data_finished);
        let metadata_ready = self.overview_state.is_ready() || self.detail_state.details_ready();
        if metadata_ready && !self.overview_state.description_ready() {
            self.prepare_pull_request_description(pr, detail_key, cx);
        }
        let initial_load_finished =
            initial_data_finished && self.overview_state.description_ready();
        if !self.overview_state.is_ready() && initial_load_finished {
            self.overview_state.mark_ready();
        }
        let overview_ready = self.overview_state.is_ready();
        let activity_loading =
            self.review_state.reviews_loading() || self.detail_state.commits_loading();
        let panel_items = overview_panel_items(
            self.overview_description_items(),
            self.detail_state.commits(),
            self.review_state.pull_request_reviews(),
            self.review_state.pull_request_comments(),
            self.review_state.review_threads(),
            self.review_state
                .reviews_error()
                .or_else(|| self.detail_state.commits_error()),
        );
        let panel_item_keys = panel_items.iter().map(OverviewPanelItem::key).collect();
        sync_overview_list_items(
            &self.overview_state.list_state,
            &mut self.overview_state.list_item_keys,
            panel_item_keys,
        );
        let panel_items_for_render = panel_items.clone();
        let activity_body = if overview_ready {
            list(
                self.overview_state.list_state.clone(),
                cx.processor(move |view, index: usize, _window, cx| {
                    let Some(item) = panel_items_for_render.get(index) else {
                        return div().into_any_element();
                    };

                    match item {
                        OverviewPanelItem::DescriptionHeader => view.render_description_header(cx),
                        OverviewPanelItem::DescriptionBlock { index: block_index } => {
                            let is_last =
                                *block_index + 1 == view.overview_state.description_block_count();
                            view.render_description_block(*block_index, is_last)
                        }
                        OverviewPanelItem::DescriptionEmpty => view.render_empty_description(),
                        OverviewPanelItem::DescriptionEditor => view.render_description_editor(cx),
                        OverviewPanelItem::ActivityHeader => {
                            render_overview_activity_header(activity_loading).into_any_element()
                        }
                        OverviewPanelItem::Commit { sha } => view
                            .detail_state
                            .commits()
                            .iter()
                            .find(|commit| commit.sha == *sha)
                            .cloned()
                            .map(|commit| render_overview_commit_event(&commit, index, cx))
                            .unwrap_or_else(|| div().into_any_element()),
                        OverviewPanelItem::Comment { id } => view
                            .review_state
                            .pull_request_comments()
                            .iter()
                            .find(|comment| comment.id == *id)
                            .cloned()
                            .map(|comment| {
                                let markdown = view.render_overview_markdown(
                                    format!("overview-comment-body-{}", comment.id),
                                    &comment.body,
                                    cx,
                                );
                                render_overview_comment_event(&comment, index, markdown)
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
                                            format!("overview-review-body-{}", review.id),
                                            body,
                                            cx,
                                        )
                                    });
                                render_overview_review_event(&review, index, markdown)
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
                                    view.overview_state.thread_expansion_override(&thread.id),
                                );
                                view.render_overview_thread_event(&thread, index, expanded, cx)
                            })
                            .unwrap_or_else(|| div().into_any_element()),
                        OverviewPanelItem::Message(message) => render_timeline_message(message),
                        OverviewPanelItem::Composer => view
                            .selected_pull_request()
                            .cloned()
                            .map(|pr| view.render_overview_comment_composer(&pr, cx))
                            .unwrap_or_else(|| div().into_any_element()),
                    }
                }),
            )
            .size_full()
            .into_any_element()
        } else {
            div()
                .size_full()
                .flex()
                .flex_col()
                .child(
                    div()
                        .flex_none()
                        .mb_3()
                        .child(render_overview_skeleton_card(
                            "pull-request-overview-description-loading",
                            56.0,
                        )),
                )
                .child(render_overview_activity_loading())
                .into_any_element()
        };
        let sidebar = if overview_ready {
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
                .child(self.render_labels_card(pr, cx))
                .into_any_element()
        } else {
            render_overview_sidebar_loading()
        };

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
                                div()
                                    .debug_selector(|| "pull-request-overview-activity".to_string())
                                    .size_full()
                                    .child(activity_body),
                            ),
                    )
                    .child(sidebar),
            )
            .into_any_element()
    }
}

fn render_overview_sidebar_loading() -> AnyElement {
    div()
        .debug_selector(|| "pull-request-overview-sidebar-loading".to_string())
        .w(px(OVERVIEW_SIDEBAR_WIDTH))
        .h_full()
        .min_h_0()
        .flex_none()
        .flex()
        .flex_col()
        .gap_3()
        .overflow_hidden()
        .child(render_overview_skeleton_card(
            "pull-request-overview-status-loading",
            210.0,
        ))
        .child(render_overview_skeleton_card(
            "pull-request-overview-people-loading",
            88.0,
        ))
        .child(render_overview_skeleton_card(
            "pull-request-overview-labels-loading",
            44.0,
        ))
        .into_any_element()
}

fn render_overview_skeleton_card(selector: &'static str, body_height: f32) -> AnyElement {
    div()
        .debug_selector(move || selector.to_string())
        .w_full()
        .rounded_sm()
        .border_1()
        .border_color(color::border())
        .bg(color::content_background())
        .p_3()
        .flex()
        .flex_col()
        .gap_3()
        .child(Skeleton::new().w_1_3().h(px(14.0)).rounded_sm())
        .child(
            Skeleton::new()
                .secondary()
                .w_full()
                .h(px(body_height))
                .rounded_sm(),
        )
        .into_any_element()
}

fn render_overview_activity_loading() -> AnyElement {
    div()
        .debug_selector(|| "pull-request-overview-activity-loading".to_string())
        .w_full()
        .flex_none()
        .flex()
        .flex_col()
        .gap_2()
        .child(render_overview_activity_skeleton_row(
            "pull-request-overview-activity-loading-row-1",
            false,
        ))
        .child(render_overview_activity_skeleton_row(
            "pull-request-overview-activity-loading-row-2",
            true,
        ))
        .into_any_element()
}

fn render_overview_activity_skeleton_row(selector: &'static str, compact: bool) -> AnyElement {
    let body = Skeleton::new().secondary().h(px(10.0)).rounded_sm();
    let body = if compact { body.w_1_2() } else { body.w_2_3() };

    div()
        .debug_selector(move || selector.to_string())
        .w_full()
        .h(px(52.0))
        .rounded_sm()
        .border_1()
        .border_color(color::border_subtle())
        .bg(color::content_background())
        .p_3()
        .flex()
        .items_center()
        .gap_3()
        .child(Skeleton::new().size(px(24.0)).rounded_full())
        .child(
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .flex_col()
                .gap_2()
                .child(Skeleton::new().w_1_3().h(px(10.0)).rounded_sm())
                .child(body),
        )
        .into_any_element()
}

fn render_overview_activity_header(loading: bool) -> impl IntoElement {
    div()
        .debug_selector(|| "pull-request-overview-activity-header".to_string())
        .w_full()
        .h(px(28.0))
        .pb_2()
        .flex()
        .items_center()
        .justify_between()
        .child(
            div()
                .text_sm()
                .font_medium()
                .text_color(color::text_primary())
                .child("Activity"),
        )
        .child(
            div()
                .size(px(16.0))
                .flex()
                .items_center()
                .justify_center()
                .when(loading, |element| element.child(Spinner::new().small())),
        )
}

#[cfg(test)]
mod tests;
