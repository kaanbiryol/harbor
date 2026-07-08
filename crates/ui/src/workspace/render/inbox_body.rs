use gpui::{AnyElement, Context, IntoElement, div, prelude::*, uniform_list};

use crate::{
    panels::render_pull_request_row,
    visual::color,
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
        let empty_message = if self.repository_state.has_configured_repo() {
            current_mode.empty_message()
        } else {
            "Choose a repository from the header"
        };
        let show_page_footer = rows_available
            && (self.pull_request_inbox.has_next_page()
                || self.pull_request_inbox.is_loading_more()
                || self.pull_request_inbox.load_more_error().is_some());
        let pull_request_list_item_count =
            visible_pull_request_indices.len() + usize::from(show_page_footer);
        let show_list = rows_available && pull_request_list_item_count > 0;
        let mut body = Vec::new();

        match body_state {
            PullRequestInboxBodyState::LoadingEmpty => {}
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
                body.push(
                    div()
                        .flex_1()
                        .px_3()
                        .py_3()
                        .text_sm()
                        .text_color(color::text_muted())
                        .child(empty_message)
                        .into_any_element(),
                );
            }
            PullRequestInboxBodyState::Rows => {}
        }

        if show_filtered_empty {
            body.push(
                div()
                    .flex_1()
                    .px_3()
                    .py_3()
                    .text_sm()
                    .text_color(color::text_muted())
                    .child("No loaded pull requests match filters")
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
                            cx.processor(|view, range: std::ops::Range<usize>, _window, cx| {
                                let visible_indices = view.visible_pull_request_indices();
                                let visible_count = visible_indices.len();
                                let prefetch_indices = range
                                    .clone()
                                    .filter_map(|row_index| visible_indices.get(row_index).copied())
                                    .collect::<Vec<_>>();
                                view.prefetch_visible_pull_request_row_enrichments(
                                    prefetch_indices,
                                    cx,
                                );
                                let mut rows = Vec::with_capacity(range.len());

                                for row_index in range {
                                    if row_index == visible_count {
                                        rows.push(view.render_pull_request_inbox_page_footer(cx));
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
                            }),
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
