use gpui::{
    App, Context, FocusHandle, Focusable, IntoElement, Render, Window, div, prelude::*, px, rgb,
    uniform_list,
};
use gpui_component::{Disableable, Sizable, button::Button};
use harbor_domain::{PullRequest, RepoId};

use crate::actions::*;
use crate::panels::{
    merge_blocker, render_actions_panel, render_changed_file_row, render_checks_panel,
    render_diff_panel, render_logs_panel, render_merge_state, render_pull_request_row,
    render_review_decision, render_review_panel, review_action_blocker,
};
use crate::workspace::AppView;

impl Focusable for AppView {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for AppView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if !self.did_focus {
            window.focus(&self.focus_handle, cx);
            self.did_focus = true;
        }

        let selected_pr = self.selected_pull_request().cloned();

        div()
            .key_context(KEY_CONTEXT)
            .track_focus(&self.focus_handle(cx))
            .on_action(cx.listener(Self::select_next))
            .on_action(cx.listener(Self::select_previous))
            .on_action(cx.listener(Self::open_selected))
            .on_action(cx.listener(Self::cycle_panel_tab))
            .on_action(cx.listener(Self::toggle_command_palette))
            .on_action(cx.listener(Self::close_panel))
            .on_action(cx.listener(Self::refresh_selected))
            .on_action(cx.listener(Self::checkout_pr))
            .on_action(cx.listener(Self::open_in_browser))
            .on_action(cx.listener(Self::approve_pr))
            .on_action(cx.listener(Self::request_changes))
            .on_action(cx.listener(Self::merge_pr))
            .on_action(cx.listener(Self::open_logs))
            .on_action(cx.listener(Self::trigger_build))
            .on_action(cx.listener(Self::rerun_failed))
            .on_action(cx.listener(Self::filter_current_list))
            .on_action(cx.listener(Self::select_next_file))
            .on_action(cx.listener(Self::select_previous_file))
            .on_action(cx.listener(Self::select_next_hunk))
            .on_action(cx.listener(Self::select_previous_hunk))
            .on_action(cx.listener(Self::copy_active_file_path))
            .on_action(cx.listener(Self::open_active_file_on_github))
            .size_full()
            .flex()
            .flex_col()
            .bg(rgb(0x101214))
            .text_color(rgb(0xe6e8eb))
            .child(self.render_header())
            .when(self.command_palette_open, |element| {
                element.child(self.render_command_palette())
            })
            .child(
                div()
                    .flex()
                    .flex_1()
                    .min_h_0()
                    .min_w_0()
                    .overflow_hidden()
                    .gap_2()
                    .p_2()
                    .child(self.render_inbox(cx))
                    .child(self.render_details(selected_pr.as_ref(), cx))
                    .child(self.render_panel(selected_pr.as_ref(), cx)),
            )
            .child(
                div()
                    .px_3()
                    .py_2()
                    .text_xs()
                    .text_color(rgb(0x9aa4b2))
                    .border_1()
                    .border_color(rgb(0x242a31))
                    .child(self.status.clone()),
            )
    }
}

impl AppView {
    fn render_header(&self) -> impl IntoElement {
        div()
            .flex()
            .items_center()
            .justify_between()
            .px_4()
            .py_3()
            .border_1()
            .border_color(rgb(0x242a31))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_3()
                    .child(div().text_lg().child("Harbor"))
                    .child(
                        div()
                            .text_xs()
                            .text_color(rgb(0x9aa4b2))
                            .child("native GitHub pull request cockpit"),
                    ),
            )
            .child(
                div()
                    .flex()
                    .gap_2()
                    .child(
                        Button::new("repo-switcher")
                            .label("cmd+p")
                            .small()
                            .outline(),
                    )
                    .child(Button::new("command-palette").label("cmd+k").small()),
            )
    }

    fn render_command_palette(&self) -> impl IntoElement {
        div()
            .mx_2()
            .mt_2()
            .p_3()
            .rounded_md()
            .border_1()
            .border_color(rgb(0x3a424c))
            .bg(rgb(0x171b20))
            .child(
                div()
                    .pb_2()
                    .text_sm()
                    .text_color(rgb(0xf1f5f9))
                    .child("Command palette placeholder"),
            )
            .children(COMMANDS.iter().map(|command| {
                div()
                    .flex()
                    .justify_between()
                    .py_1()
                    .text_sm()
                    .child(command.title)
                    .child(
                        div()
                            .text_xs()
                            .text_color(rgb(0x9aa4b2))
                            .child(command.shortcut),
                    )
            }))
    }

    fn render_inbox(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let show_list =
            !self.is_loading_prs && self.load_error.is_none() && !self.pull_requests.is_empty();

        div()
            .w(px(320.))
            .flex()
            .flex_col()
            .min_h_0()
            .rounded_md()
            .border_1()
            .border_color(rgb(0x242a31))
            .bg(rgb(0x15191e))
            .overflow_hidden()
            .child(
                div()
                    .px_3()
                    .py_2()
                    .text_sm()
                    .text_color(rgb(0xf1f5f9))
                    .child("Pull request inbox")
                    .child(
                        div().pt_1().text_xs().text_color(rgb(0x9aa4b2)).child(
                            self.configured_repo
                                .as_ref()
                                .map(RepoId::full_name)
                                .unwrap_or_else(|| "fake data".to_string()),
                        ),
                    ),
            )
            .when(self.is_loading_prs, |element| {
                element.child(
                    div()
                        .flex_1()
                        .px_3()
                        .py_3()
                        .text_sm()
                        .text_color(rgb(0x9aa4b2))
                        .child("Loading open pull requests..."),
                )
            })
            .when(
                !self.is_loading_prs && self.load_error.is_some(),
                |element| {
                    element.child(
                        div()
                            .flex_1()
                            .px_3()
                            .py_3()
                            .text_sm()
                            .text_color(rgb(0xf87171))
                            .child(self.load_error.clone().unwrap_or_default()),
                    )
                },
            )
            .when(
                !self.is_loading_prs && self.load_error.is_none() && self.pull_requests.is_empty(),
                |element| {
                    element.child(
                        div()
                            .flex_1()
                            .px_3()
                            .py_3()
                            .text_sm()
                            .text_color(rgb(0x9aa4b2))
                            .child("No open pull requests"),
                    )
                },
            )
            .when(show_list, |element| {
                element.child(
                    uniform_list(
                        "pull-request-inbox-list",
                        self.pull_requests.len(),
                        cx.processor(|view, range: std::ops::Range<usize>, _window, cx| {
                            let mut rows = Vec::with_capacity(range.len());

                            for index in range {
                                let Some(pr) = view.pull_requests.get(index) else {
                                    continue;
                                };
                                rows.push(render_pull_request_row(
                                    index,
                                    pr,
                                    index == view.selected_pr,
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
            })
    }

    fn render_details(&self, pr: Option<&PullRequest>, cx: &mut Context<Self>) -> impl IntoElement {
        let Some(pr) = pr else {
            return div()
                .w(px(360.))
                .flex()
                .flex_col()
                .min_h_0()
                .rounded_md()
                .border_1()
                .border_color(rgb(0x242a31))
                .bg(rgb(0x15191e))
                .overflow_hidden()
                .p_3()
                .text_sm()
                .text_color(rgb(0x9aa4b2))
                .child("Select a pull request to see details")
                .into_any_element();
        };

        let review_action_disabled = self.configured_repo.is_none()
            || self.is_running_pr_action
            || review_action_blocker(pr).is_some();
        let merge_action_disabled = self.configured_repo.is_none()
            || self.is_running_pr_action
            || merge_blocker(pr).is_some();

        div()
            .w(px(360.))
            .flex()
            .flex_col()
            .min_h_0()
            .rounded_md()
            .border_1()
            .border_color(rgb(0x242a31))
            .bg(rgb(0x15191e))
            .overflow_hidden()
            .child(
                div()
                    .p_3()
                    .border_1()
                    .border_color(rgb(0x242a31))
                    .child(
                        div()
                            .text_sm()
                            .child(format!("#{} {}", pr.number, pr.title)),
                    )
                    .child(
                        div()
                            .pt_1()
                            .text_xs()
                            .text_color(rgb(0x9aa4b2))
                            .child(format!("{} / {}", pr.repo.full_name(), pr.head_sha)),
                    )
                    .when(self.is_loading_details, |element| {
                        element.child(
                            div()
                                .pt_2()
                                .text_xs()
                                .text_color(rgb(0x9aa4b2))
                                .child("Loading latest PR details..."),
                        )
                    })
                    .when_some(self.details_error.clone(), |element, error| {
                        element.child(
                            div()
                                .pt_2()
                                .text_xs()
                                .text_color(rgb(0xf87171))
                                .child(error),
                        )
                    })
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
                                    .text_color(rgb(0xfbbf24))
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
                                    .outline()
                                    .loading(self.is_running_pr_action)
                                    .disabled(review_action_disabled)
                                    .on_click(cx.listener(|view, _, _, cx| {
                                        view.run_pull_request_action(
                                            PullRequestAction::Approve,
                                            cx,
                                        );
                                    })),
                            )
                            .child(
                                Button::new("request-pr-changes")
                                    .label("changes")
                                    .small()
                                    .outline()
                                    .loading(self.is_running_pr_action)
                                    .disabled(review_action_disabled)
                                    .on_click(cx.listener(|view, _, _, cx| {
                                        view.run_pull_request_action(
                                            PullRequestAction::RequestChanges,
                                            cx,
                                        );
                                    })),
                            )
                            .child(
                                Button::new("merge-pr")
                                    .label("merge")
                                    .small()
                                    .outline()
                                    .loading(self.is_running_pr_action)
                                    .disabled(merge_action_disabled)
                                    .on_click(cx.listener(|view, _, _, cx| {
                                        view.run_pull_request_action(PullRequestAction::Merge, cx);
                                    })),
                            ),
                    )
                    .when_some(self.pr_action_error.clone(), |element, error| {
                        element.child(
                            div()
                                .pt_2()
                                .text_xs()
                                .text_color(rgb(0xf87171))
                                .child(error),
                        )
                    }),
            )
            .child(
                div()
                    .px_3()
                    .py_2()
                    .text_xs()
                    .text_color(rgb(0x9aa4b2))
                    .child("Changed files"),
            )
            .when(self.is_loading_files, |element| {
                element.child(
                    div()
                        .flex_1()
                        .px_3()
                        .py_3()
                        .text_sm()
                        .text_color(rgb(0x9aa4b2))
                        .child("Loading changed files..."),
                )
            })
            .when_some(self.files_error.clone(), |element, error| {
                element.child(
                    div()
                        .flex_1()
                        .px_3()
                        .py_3()
                        .text_sm()
                        .text_color(rgb(0xf87171))
                        .child(error),
                )
            })
            .when(
                !self.is_loading_files && self.files_error.is_none() && self.files.is_empty(),
                |element| {
                    element.child(
                        div()
                            .flex_1()
                            .px_3()
                            .py_3()
                            .text_sm()
                            .text_color(rgb(0x9aa4b2))
                            .child("No changed files"),
                    )
                },
            )
            .when(
                !self.is_loading_files && self.files_error.is_none() && !self.files.is_empty(),
                |element| {
                    element.child(
                        uniform_list(
                            "changed-files-list",
                            self.files.len(),
                            cx.processor(|view, range: std::ops::Range<usize>, _window, cx| {
                                let mut rows = Vec::with_capacity(range.len());

                                for index in range {
                                    let Some(file) = view.files.get(index) else {
                                        continue;
                                    };
                                    rows.push(render_changed_file_row(
                                        index,
                                        file,
                                        index == view.active_file,
                                        cx,
                                    ));
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

    fn render_panel(&self, pr: Option<&PullRequest>, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex_1()
            .flex()
            .flex_col()
            .min_h_0()
            .min_w_0()
            .rounded_md()
            .border_1()
            .border_color(rgb(0x242a31))
            .bg(rgb(0x15191e))
            .overflow_hidden()
            .child(
                div()
                    .flex()
                    .gap_2()
                    .p_2()
                    .border_1()
                    .border_color(rgb(0x242a31))
                    .children(PanelTab::ALL.iter().map(|tab| {
                        let active = *tab == self.active_tab;
                        div()
                            .px_3()
                            .py_1()
                            .rounded_sm()
                            .text_sm()
                            .when(active, |element| element.bg(rgb(0x243244)))
                            .child(tab.label())
                    })),
            )
            .child(
                div()
                    .id("panel-content-scroll")
                    .flex_1()
                    .flex()
                    .flex_col()
                    .min_h_0()
                    .min_w_0()
                    .p_3()
                    .text_sm()
                    .child(match self.active_tab {
                        PanelTab::Diff => render_diff_panel(
                            self.active_file(),
                            self.active_diff(),
                            self.is_loading_files,
                            self.files_error.as_deref(),
                            self.diff_list_scroll.clone(),
                            cx,
                        )
                        .into_any_element(),
                        PanelTab::Review => render_review_panel(
                            &self.pull_request_reviews,
                            &self.review_threads,
                            self.is_loading_reviews,
                            self.reviews_error.as_deref(),
                            self.review_list_scroll.clone(),
                            cx,
                        )
                        .into_any_element(),
                        PanelTab::Checks => render_checks_panel(
                            pr.map(|pr| pr.checks_summary).unwrap_or_default(),
                            &self.check_runs,
                            self.is_loading_checks,
                            self.checks_error.as_deref(),
                        )
                        .into_any_element(),
                        PanelTab::Actions => render_actions_panel(
                            pr,
                            &self.workflow_runs,
                            self.is_loading_workflows,
                            self.workflows_error.as_deref(),
                            self.action_error.as_deref(),
                            self.is_running_action,
                            cx,
                        )
                        .into_any_element(),
                        PanelTab::Logs => render_logs_panel(
                            self.selected_workflow_run_for_logs(),
                            &self.workflow_jobs,
                            self.log_chunk.as_ref(),
                            self.is_loading_logs,
                            self.logs_error.as_deref(),
                            self.log_list_scroll.clone(),
                            cx,
                        )
                        .into_any_element(),
                    }),
            )
    }
}
