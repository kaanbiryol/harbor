use gpui::{
    App, Context, FocusHandle, Focusable, IntoElement, Render, Window, div, prelude::*, rgb,
};
use harbor_domain::PullRequest;

use crate::actions::*;
use crate::panels::{
    render_actions_panel, render_checks_panel, render_diff_panel, render_logs_panel,
    render_review_panel,
};
use crate::workspace::AppView;

#[path = "render/changed_files.rs"]
mod changed_files;
#[path = "render/details.rs"]
mod details;
#[path = "render/header.rs"]
pub(crate) mod header;
#[path = "render/inbox.rs"]
mod inbox;
#[path = "render/pending_review.rs"]
mod pending_review;

impl Focusable for AppView {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

pub(super) fn render_switcher_section_label(label: &'static str) -> impl IntoElement {
    div()
        .px_2()
        .py_1()
        .text_xs()
        .text_color(rgb(0x7d8794))
        .child(label)
}

impl Render for AppView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if !self.did_focus {
            if self.repository_switcher_open {
                self.repository_search_input
                    .update(cx, |input, cx| input.focus(window, cx));
            } else {
                window.focus(&self.focus_handle, cx);
            }
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
            .on_action(cx.listener(Self::toggle_pull_request_inbox))
            .on_action(cx.listener(Self::toggle_repository_switcher))
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
            .on_action(cx.listener(Self::choose_local_checkout))
            .on_action(cx.listener(Self::open_with_vs_code))
            .on_action(cx.listener(Self::open_with_cursor))
            .on_action(cx.listener(Self::open_with_zed))
            .on_action(cx.listener(Self::open_with_finder))
            .on_action(cx.listener(Self::open_with_terminal))
            .on_action(cx.listener(Self::open_with_ghostty))
            .on_action(cx.listener(Self::open_with_warp))
            .on_action(cx.listener(Self::open_with_xcode))
            .size_full()
            .flex()
            .flex_col()
            .bg(rgb(0x101214))
            .text_color(rgb(0xe6e8eb))
            .child(self.render_title_bar(cx))
            .child(
                div()
                    .flex()
                    .flex_1()
                    .min_h_0()
                    .min_w_0()
                    .overflow_hidden()
                    .gap_2()
                    .p_2()
                    .when(self.pull_request_inbox_visible, |element| {
                        element.child(self.render_inbox(cx))
                    })
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
    fn render_panel(&self, pr: Option<&PullRequest>, cx: &mut Context<Self>) -> impl IntoElement {
        let view = cx.entity().clone();

        div()
            .flex_1()
            .flex()
            .flex_col()
            .min_h_0()
            .min_w_0()
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
                    .children(
                        PanelTab::ALL
                            .iter()
                            .copied()
                            .enumerate()
                            .map(|(index, tab)| {
                                let active = tab == self.active_tab;
                                let view = view.clone();

                                div()
                                    .id(("panel-tab", index))
                                    .px_3()
                                    .py_1()
                                    .text_sm()
                                    .cursor_pointer()
                                    .when(active, |element| element.bg(rgb(0x243244)))
                                    .hover(move |element| {
                                        if active {
                                            element
                                        } else {
                                            element.bg(rgb(0x1d2530))
                                        }
                                    })
                                    .on_click(move |_, _, cx| {
                                        view.update(cx, |view, cx| {
                                            view.select_panel_tab(tab, cx);
                                        });
                                    })
                                    .child(tab.label())
                            }),
                    ),
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
                        PanelTab::Diff => {
                            let visible_file_indices = self.visible_file_indices(cx);
                            render_diff_panel(
                                &self.files,
                                &self.diffs,
                                &visible_file_indices,
                                &self.reviewed_file_paths,
                                &self.review_threads,
                                self.review_composer.as_ref(),
                                self.review_comment_error.as_deref(),
                                self.review_thread_reply_thread_id.as_deref(),
                                self.review_comment_edit_comment_id.as_deref(),
                                self.is_loading_files,
                                self.files_error.as_deref(),
                                self.diff_list_scroll.clone(),
                                cx,
                            )
                            .into_any_element()
                        }
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
