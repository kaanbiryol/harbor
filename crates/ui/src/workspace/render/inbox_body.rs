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
        let body_state = pull_request_inbox_body_state(
            self.pull_request_inbox.is_loading(),
            load_error.is_some(),
            !self.pull_requests.is_empty(),
        );
        let show_list = matches!(
            body_state,
            PullRequestInboxBodyState::ErrorRows | PullRequestInboxBodyState::Rows
        );
        let empty_message = if self.repository_state.has_configured_repo() {
            current_mode.empty_message()
        } else {
            "Choose a repository from the header"
        };
        let show_page_footer = show_list
            && (self.pull_request_inbox.has_next_page()
                || self.pull_request_inbox.is_loading_more()
                || self.pull_request_inbox.load_more_error().is_some());
        let pull_request_list_item_count = self.pull_requests.len() + usize::from(show_page_footer);
        let mut body = Vec::new();

        match body_state {
            PullRequestInboxBodyState::LoadingEmpty => {
                body.push(
                    div()
                        .flex_1()
                        .px_3()
                        .py_3()
                        .text_sm()
                        .text_color(color::text_muted())
                        .child(format!("Loading {}...", current_mode.status_label()))
                        .into_any_element(),
                );
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
                                view.prefetch_visible_pull_request_row_enrichments(
                                    range.clone(),
                                    cx,
                                );
                                let mut rows = Vec::with_capacity(range.len());

                                for index in range {
                                    if index == view.pull_requests.len() {
                                        rows.push(view.render_pull_request_inbox_page_footer(cx));
                                        continue;
                                    }

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
