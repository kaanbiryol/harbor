use gpui::{App, Context, FocusHandle, Focusable, IntoElement, Render, Window, div, prelude::*};
use gpui_component::Root;

use crate::actions::*;
use crate::visual::{color, font};
use crate::workspace::AppView;

#[path = "render/auth_gate.rs"]
mod auth_gate;
#[path = "render/auth_gate_signed_out.rs"]
mod auth_gate_signed_out;
#[path = "render/auth_preview.rs"]
mod auth_preview;
#[path = "render/changed_file_filter_rows.rs"]
mod changed_file_filter_rows;
#[path = "render/changed_file_filters.rs"]
mod changed_file_filters;
#[path = "render/changed_files.rs"]
mod changed_files;
#[path = "render/details.rs"]
mod details;
#[path = "render/header.rs"]
pub(crate) mod header;
#[path = "render/inbox.rs"]
mod inbox;
#[path = "render/inbox_body.rs"]
mod inbox_body;
#[path = "render/inbox_filters.rs"]
mod inbox_filters;
#[path = "render/inbox_header.rs"]
mod inbox_header;
#[path = "render/inbox_page_footer.rs"]
mod inbox_page_footer;
#[path = "render/inbox_search.rs"]
mod inbox_search;
#[path = "render/inbox_search_rows.rs"]
mod inbox_search_rows;
#[path = "render/panel.rs"]
mod panel;
#[path = "render/pending_review.rs"]
mod pending_review;
#[path = "render/pull_request_details_header.rs"]
mod pull_request_details_header;
#[path = "render/rate_limits.rs"]
mod rate_limits;
#[path = "render/review_action_comment_dialog.rs"]
mod review_action_comment_dialog;
#[path = "render/settings.rs"]
mod settings;
#[path = "render/settings_account.rs"]
mod settings_account;
#[path = "render/settings_auth_methods.rs"]
mod settings_auth_methods;
#[path = "render/settings_auth_status.rs"]
mod settings_auth_status;
#[path = "render/status_bar.rs"]
mod status_bar;

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
        .text_color(color::text_muted())
        .child(label)
}

impl Render for AppView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if !self.sync_runtime.did_focus() {
            if self.repository_state.repository_switcher_open {
                self.repository_state
                    .repository_search_input
                    .update(cx, |input, cx| input.focus(window, cx));
            } else {
                window.focus(&self.focus_handle, cx);
            }
            self.sync_runtime.mark_focused_once();
        }

        let selected_pr = self.selected_pull_request().cloned();
        let show_auth_gate = self.github_auth_gate_visible();
        let content = if show_auth_gate {
            self.render_github_auth_gate(cx).into_any_element()
        } else {
            div()
                .flex()
                .flex_1()
                .min_h_0()
                .min_w_0()
                .overflow_hidden()
                .gap_2()
                .p_2()
                .when(self.pull_request_inbox.is_visible(), |element| {
                    element.child(self.render_inbox(cx))
                })
                .child(self.render_details(selected_pr.as_ref(), cx))
                .child(self.render_panel(selected_pr.as_ref(), cx))
                .into_any_element()
        };

        div()
            .key_context(KEY_CONTEXT)
            .track_focus(&self.focus_handle(cx))
            .on_action(cx.listener(Self::select_next))
            .on_action(cx.listener(Self::select_previous))
            .on_action(cx.listener(Self::open_selected))
            .on_action(cx.listener(Self::cycle_panel_tab))
            .on_action(cx.listener(Self::select_diff_panel))
            .on_action(cx.listener(Self::select_review_panel))
            .on_action(cx.listener(Self::select_checks_panel))
            .on_action(cx.listener(Self::select_actions_panel))
            .on_action(cx.listener(Self::select_logs_panel))
            .on_action(cx.listener(Self::toggle_pull_request_inbox))
            .on_action(cx.listener(Self::toggle_repository_switcher))
            .on_action(cx.listener(Self::open_pull_request_search))
            .on_action(cx.listener(Self::close_panel))
            .on_action(cx.listener(Self::refresh_selected))
            .on_action(cx.listener(Self::checkout_pr))
            .on_action(cx.listener(Self::open_in_browser))
            .on_action(cx.listener(Self::open_pull_request_comment_dialog))
            .on_action(cx.listener(Self::approve_pr))
            .on_action(cx.listener(Self::request_changes))
            .on_action(cx.listener(Self::open_approve_comment_dialog))
            .on_action(cx.listener(Self::open_request_changes_comment_dialog))
            .on_action(cx.listener(Self::merge_pr))
            .on_action(cx.listener(Self::merge_pr_with_merge_commit))
            .on_action(cx.listener(Self::rebase_pr))
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
            .on_action(cx.listener(Self::sign_in_to_github))
            .on_action(cx.listener(Self::use_github_cli))
            .on_action(cx.listener(Self::sign_out_of_github))
            .on_action(cx.listener(Self::open_settings))
            .on_action(cx.listener(Self::close_settings))
            .on_action(cx.listener(Self::switch_github_auth_to_oauth))
            .on_action(cx.listener(Self::switch_github_auth_to_gh_cli))
            .size_full()
            .relative()
            .flex()
            .flex_col()
            .bg(color::app_background())
            .text_color(color::text_primary())
            .font_family(font::UI)
            .child(self.render_title_bar(window, cx))
            .child(content)
            .when(!show_auth_gate, |element| {
                element.child(self.render_status_bar(cx))
            })
            .when(self.review_action_comment_target.is_some(), |element| {
                element.child(self.render_review_action_comment_dialog(cx))
            })
            .children(Root::render_dialog_layer(window, cx))
    }
}
