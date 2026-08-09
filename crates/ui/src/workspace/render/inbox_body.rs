use gpui::{AnyElement, Context, IntoElement, div, prelude::*, px, uniform_list};
use gpui_component::skeleton::Skeleton;

use crate::{
    icons::Octicon,
    panels::{render_empty_state, render_pull_request_row},
    visual::{color, layout},
    workspace::{AppView, PullRequestInboxMode},
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PullRequestInboxBodyState {
    LoadingEmpty,
    ErrorEmpty,
    ErrorRows,
    Empty,
    Rows,
}

fn pull_request_inbox_body_state(
    is_loading: bool,
    has_load_error: bool,
    has_pull_requests: bool,
) -> PullRequestInboxBodyState {
    match (is_loading, has_load_error, has_pull_requests) {
        (true, _, true) => PullRequestInboxBodyState::Rows,
        (true, _, false) => PullRequestInboxBodyState::LoadingEmpty,
        (false, true, true) => PullRequestInboxBodyState::ErrorRows,
        (false, true, false) => PullRequestInboxBodyState::ErrorEmpty,
        (false, false, true) => PullRequestInboxBodyState::Rows,
        (false, false, false) => PullRequestInboxBodyState::Empty,
    }
}

impl AppView {
    pub(super) fn render_pull_request_inbox_body(
        &self,
        current_mode: PullRequestInboxMode,
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        let load_error = self.pull_request_inbox.load_error().map(str::to_string);
        let visible_pull_request_indices = self.visible_pull_request_indices();
        let has_active_filters = self.has_active_pull_request_filters();
        let body_state = pull_request_inbox_body_state(
            self.pull_request_inbox.is_loading(),
            load_error.is_some(),
            !self.pull_requests.is_empty(),
        );
        let rows_available = matches!(
            body_state,
            PullRequestInboxBodyState::ErrorRows | PullRequestInboxBodyState::Rows
        );
        let show_filtered_empty =
            rows_available && has_active_filters && visible_pull_request_indices.is_empty();
        let has_repository = self.repository_state.has_configured_repo();
        let show_page_footer = rows_available
            && (self.pull_request_inbox.has_next_page()
                || self.pull_request_inbox.is_loading_more()
                || self.pull_request_inbox.load_more_error().is_some());
        let pull_request_list_item_count =
            visible_pull_request_indices.len() + usize::from(show_page_footer);
        let show_list = rows_available && pull_request_list_item_count > 0;
        let visible_pull_request_indices_for_render = visible_pull_request_indices.clone();
        let mut body = Vec::new();

        match body_state {
            PullRequestInboxBodyState::LoadingEmpty => {
                body.push(render_pull_request_inbox_loading());
            }
            PullRequestInboxBodyState::ErrorRows => {
                body.push(
                    div()
                        .id("pull-request-inbox-refresh-error")
                        .px_3()
                        .py_2()
                        .border_b_1()
                        .border_color(color::border())
                        .text_xs()
                        .text_color(color::danger())
                        .child(format!(
                            "Refresh failed: {}",
                            load_error.clone().unwrap_or_default()
                        ))
                        .into_any_element(),
                );
            }
            PullRequestInboxBodyState::ErrorEmpty => {
                body.push(
                    div()
                        .flex_1()
                        .px_3()
                        .py_3()
                        .text_sm()
                        .text_color(color::danger())
                        .child(load_error.clone().unwrap_or_default())
                        .into_any_element(),
                );
            }
            PullRequestInboxBodyState::Empty => {
                body.push(render_pull_request_inbox_empty_state(
                    current_mode,
                    has_repository,
                ));
            }
            PullRequestInboxBodyState::Rows => {}
        }

        if show_filtered_empty {
            body.push(
                render_empty_state(
                    Octicon::Search,
                    "No matching pull requests",
                    "Adjust or clear the active filters to see more results.",
                )
                .debug_selector(|| "pull-request-inbox-filtered-empty-state".to_string())
                .into_any_element(),
            );
        }

        if show_list {
            body.push(
                div()
                    .id("pull-request-inbox-list")
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_h_0()
                    .w_full()
                    .child(
                        uniform_list(
                            "pull-request-inbox-rows",
                            pull_request_list_item_count,
                            cx.processor(
                                move |view, range: std::ops::Range<usize>, _window, cx| {
                                    let visible_indices = &visible_pull_request_indices_for_render;
                                    let visible_count = visible_indices.len();
                                    let prefetch_indices = range
                                        .clone()
                                        .filter_map(|row_index| {
                                            visible_indices.get(row_index).copied()
                                        })
                                        .collect::<Vec<_>>();
                                    view.prefetch_visible_pull_request_row_enrichments(
                                        prefetch_indices,
                                        cx,
                                    );
                                    let mut rows = Vec::with_capacity(range.len());

                                    for row_index in range {
                                        if row_index == visible_count {
                                            rows.push(
                                                view.render_pull_request_inbox_page_footer(cx),
                                            );
                                            continue;
                                        }

                                        let Some(index) = visible_indices.get(row_index).copied()
                                        else {
                                            continue;
                                        };
                                        let Some(pr) = view.pull_requests.get(index) else {
                                            continue;
                                        };
                                        rows.push(render_pull_request_row(
                                            index,
                                            pr,
                                            index == view.selected_pull_request_index(),
                                            cx,
                                        ));
                                    }

                                    rows
                                },
                            ),
                        )
                        .track_scroll(&self.pr_list_scroll)
                        .flex_1()
                        .min_h_0()
                        .w_full(),
                    )
                    .into_any_element(),
            );
        }

        body
    }
}

fn render_pull_request_inbox_empty_state(
    mode: PullRequestInboxMode,
    has_repository: bool,
) -> AnyElement {
    let (icon, title, description) = if !has_repository {
        (
            Octicon::FileDirectory,
            "No repository selected",
            "Choose a repository from the title bar to load pull requests.",
        )
    } else {
        match mode {
            PullRequestInboxMode::Open => (
                Octicon::GitPullRequest,
                "No open pull requests",
                "This repository does not have any open pull requests.",
            ),
            PullRequestInboxMode::Closed => (
                Octicon::CheckCircle,
                "No closed pull requests",
                "Closed pull requests will appear here.",
            ),
            PullRequestInboxMode::NeedsReview => (
                Octicon::CheckCircle,
                "Nothing to review",
                "You're all caught up. No pull requests currently need your review.",
            ),
        }
    };

    render_empty_state(icon, title, description)
        .debug_selector(|| "pull-request-inbox-empty-state".to_string())
        .into_any_element()
}

fn render_pull_request_inbox_loading() -> AnyElement {
    div()
        .debug_selector(|| "pull-request-inbox-loading".to_string())
        .flex_1()
        .min_h_0()
        .w_full()
        .overflow_hidden()
        .children((0..12).map(render_pull_request_inbox_skeleton_row))
        .into_any_element()
}

fn render_pull_request_inbox_skeleton_row(index: usize) -> AnyElement {
    let metadata_widths = [112.0, 148.0, 124.0, 136.0];

    div()
        .debug_selector(move || format!("pull-request-inbox-loading-row-{index}"))
        .h(px(layout::PULL_REQUEST_ROW_HEIGHT))
        .w_full()
        .min_w_0()
        .flex()
        .overflow_hidden()
        .border_1()
        .border_color(color::border_subtle())
        .child(
            div()
                .h_full()
                .w(px(2.0))
                .flex_none()
                .bg(color::border_subtle()),
        )
        .child(
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .flex_col()
                .justify_center()
                .gap_2()
                .overflow_hidden()
                .px_3()
                .py_2()
                .child(
                    div()
                        .w_full()
                        .min_w_0()
                        .flex()
                        .items_center()
                        .gap_2()
                        .child(
                            pull_request_inbox_skeleton()
                                .w(px(32.0))
                                .h(px(10.0))
                                .rounded_sm(),
                        )
                        .child(
                            pull_request_inbox_skeleton()
                                .flex_1()
                                .h(px(12.0))
                                .rounded_sm(),
                        )
                        .when(index % 3 != 2, |element| {
                            element.child(pull_request_inbox_skeleton().size(px(24.0)).rounded_xs())
                        }),
                )
                .child(
                    pull_request_inbox_skeleton()
                        .w(px(metadata_widths[index % metadata_widths.len()]))
                        .h(px(9.0))
                        .rounded_sm(),
                ),
        )
        .into_any_element()
}

fn pull_request_inbox_skeleton() -> Skeleton {
    Skeleton::new().bg(color::border_strong())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_existing_pull_request_rows_visible_while_refreshing() {
        assert_eq!(
            pull_request_inbox_body_state(true, false, true),
            PullRequestInboxBodyState::Rows
        );
        assert_eq!(
            pull_request_inbox_body_state(false, true, true),
            PullRequestInboxBodyState::ErrorRows
        );
        assert_eq!(
            pull_request_inbox_body_state(true, false, false),
            PullRequestInboxBodyState::LoadingEmpty
        );
        assert_eq!(
            pull_request_inbox_body_state(false, true, false),
            PullRequestInboxBodyState::ErrorEmpty
        );
    }
}
