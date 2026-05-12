use gpui::{Context, IntoElement, div, prelude::*, px, uniform_list};
use gpui_component::{
    Disableable, Sizable,
    button::{Button, ButtonVariants},
};
use harbor_domain::PullRequest;

use crate::{
    actions::PullRequestAction,
    panels::{
        merge_blocker, render_changed_file_row, render_changed_folder_row, render_merge_state,
        render_review_decision, review_action_blocker,
    },
    visual::color,
    workspace::{AppView, ChangedFileTreeRow},
};

impl AppView {
    pub(super) fn render_details(
        &self,
        pr: Option<&PullRequest>,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let Some(pr) = pr else {
            return div()
                .w(px(360.))
                .flex()
                .flex_col()
                .min_h_0()
                .border_1()
                .border_color(color::border())
                .bg(color::panel_background())
                .overflow_hidden()
                .p_3()
                .text_sm()
                .text_color(color::text_muted())
                .child("Select a pull request to see details")
                .into_any_element();
        };

        let pull_request_action_running = self.action_runtime.pull_request_action_running();
        let review_action_blocker = review_action_blocker(pr);
        let merge_blocker = merge_blocker(pr);
        let review_action_disabled = pull_request_action_running || review_action_blocker.is_some();
        let merge_action_disabled = pull_request_action_running || merge_blocker.is_some();
        let approve_tooltip = review_action_blocker
            .clone()
            .unwrap_or_else(|| "Approve pull request".to_string());
        let changes_tooltip = review_action_blocker
            .clone()
            .unwrap_or_else(|| "Request changes".to_string());
        let merge_tooltip = merge_blocker
            .clone()
            .unwrap_or_else(|| "Merge pull request".to_string());
        let pull_request_url = pr.url.clone();
        let pull_request_number = pr.number;

        div()
            .w(px(360.))
            .flex()
            .flex_col()
            .min_h_0()
            .border_1()
            .border_color(color::border())
            .bg(color::panel_background())
            .overflow_hidden()
            .child(
                div()
                    .p_3()
                    .border_1()
                    .border_color(color::border())
                    .child(
                        div()
                            .id(("pull-request-title-link", pr.number))
                            .text_sm()
                            .text_color(color::accent())
                            .cursor_pointer()
                            .hover(|element| element.text_color(color::accent_hover()))
                            .on_click(cx.listener(move |view, _, _, cx| {
                                cx.open_url(&pull_request_url);
                                view.status =
                                    format!("Opened PR #{pull_request_number} in browser");
                                cx.notify();
                            }))
                            .child(format!("#{} {}", pr.number, pr.title)),
                    )
                    .child(
                        div()
                            .pt_1()
                            .text_xs()
                            .text_color(color::text_muted())
                            .child(format!("{} / {}", pr.repo.full_name(), pr.head_sha)),
                    )
                    .when(self.detail_state.details_loading(), |element| {
                        element.child(
                            div()
                                .pt_2()
                                .text_xs()
                                .text_color(color::text_muted())
                                .child("Loading latest PR details..."),
                        )
                    })
                    .when_some(
                        self.detail_state.details_error().map(str::to_string),
                        |element, error| {
                            element.child(
                                div()
                                    .pt_2()
                                    .text_xs()
                                    .text_color(color::danger())
                                    .child(error),
                            )
                        },
                    )
                    .child(
                        div()
                            .pt_2()
                            .flex()
                            .gap_2()
                            .child(render_review_decision(pr.review_decision))
                            .child(render_merge_state(pr.merge_state))
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(color::warning())
                                    .child(format!("{} unresolved", pr.unresolved_threads)),
                            ),
                    )
                    .child(
                        div()
                            .pt_3()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(
                                Button::new("approve-pr")
                                    .label("approve")
                                    .small()
                                    .primary()
                                    .tooltip(approve_tooltip.clone())
                                    .loading(pull_request_action_running)
                                    .disabled(review_action_disabled)
                                    .on_click(cx.listener(|view, _, window, cx| {
                                        view.run_pull_request_action(
                                            PullRequestAction::Approve,
                                            window,
                                            cx,
                                        );
                                    })),
                            )
                            .child(
                                Button::new("request-pr-changes")
                                    .label("changes")
                                    .small()
                                    .outline()
                                    .tooltip(changes_tooltip.clone())
                                    .loading(pull_request_action_running)
                                    .disabled(review_action_disabled)
                                    .on_click(cx.listener(|view, _, window, cx| {
                                        view.run_pull_request_action(
                                            PullRequestAction::RequestChanges,
                                            window,
                                            cx,
                                        );
                                    })),
                            )
                            .child({
                                let button = Button::new("merge-pr")
                                    .label("merge")
                                    .small()
                                    .tooltip(merge_tooltip.clone());
                                let button = if merge_action_disabled {
                                    button.outline()
                                } else {
                                    button.primary()
                                };

                                button
                                    .loading(pull_request_action_running)
                                    .disabled(merge_action_disabled)
                                    .on_click(cx.listener(|view, _, window, cx| {
                                        view.run_pull_request_action(
                                            PullRequestAction::Merge,
                                            window,
                                            cx,
                                        );
                                    }))
                            }),
                    )
                    .when_some(
                        self.review_state.pending_review_cloned(),
                        |element, pending_review| {
                            element.child(self.render_pending_review_bar(pending_review, cx))
                        },
                    )
                    .when_some(
                        self.action_runtime
                            .pull_request_action_error()
                            .map(str::to_string),
                        |element, error| {
                            element.child(
                                div()
                                    .pt_2()
                                    .text_xs()
                                    .text_color(color::danger())
                                    .child(error),
                            )
                        },
                    ),
            )
            .child(self.render_changed_files_header(cx))
            .when(self.detail_state.files_loading(), |element| {
                element.child(
                    div()
                        .flex_1()
                        .px_3()
                        .py_3()
                        .text_sm()
                        .text_color(color::text_muted())
                        .child("Loading changed files..."),
                )
            })
            .when_some(
                self.detail_state.files_error().map(str::to_string),
                |element, error| {
                    element.child(
                        div()
                            .flex_1()
                            .px_3()
                            .py_3()
                            .text_sm()
                            .text_color(color::danger())
                            .child(error),
                    )
                },
            )
            .when(
                !self.detail_state.files_loading()
                    && self.detail_state.files_error().is_none()
                    && self.detail_state.files.is_empty(),
                |element| {
                    element.child(
                        div()
                            .flex_1()
                            .px_3()
                            .py_3()
                            .text_sm()
                            .text_color(color::text_muted())
                            .child("No changed files"),
                    )
                },
            )
            .when(
                !self.detail_state.files_loading()
                    && self.detail_state.files_error().is_none()
                    && !self.detail_state.files.is_empty()
                    && self.changed_file_tree_rows(cx).is_empty(),
                |element| {
                    element.child(
                        div()
                            .flex_1()
                            .px_3()
                            .py_3()
                            .text_sm()
                            .text_color(color::text_muted())
                            .child("No files match filter"),
                    )
                },
            )
            .when(
                !self.detail_state.files_loading()
                    && self.detail_state.files_error().is_none()
                    && !self.detail_state.files.is_empty()
                    && !self.changed_file_tree_rows(cx).is_empty(),
                |element| {
                    let row_count = self.changed_file_tree_rows(cx).len();

                    element.child(
                        uniform_list(
                            "changed-files-list",
                            row_count,
                            cx.processor(|view, range: std::ops::Range<usize>, _window, cx| {
                                let tree_rows = view.changed_file_tree_rows(cx);
                                let mut rows = Vec::with_capacity(range.len());

                                for row_index in range {
                                    let Some(row) = tree_rows.get(row_index) else {
                                        continue;
                                    };
                                    match row {
                                        ChangedFileTreeRow::Folder(folder_row) => {
                                            rows.push(render_changed_folder_row(folder_row, cx));
                                        }
                                        ChangedFileTreeRow::File(file_row) => {
                                            let Some(file) =
                                                view.detail_state.files.get(file_row.file_index)
                                            else {
                                                continue;
                                            };
                                            rows.push(render_changed_file_row(
                                                file_row,
                                                file,
                                                file_row.file_index == view.active_file_index(),
                                                view.reviewed_file_paths.contains(&file.path),
                                                cx,
                                            ));
                                        }
                                    }
                                }

                                rows
                            }),
                        )
                        .track_scroll(&self.file_list_scroll)
                        .flex_1()
                        .min_h_0()
                        .w_full(),
                    )
                },
            )
            .into_any_element()
    }
}
