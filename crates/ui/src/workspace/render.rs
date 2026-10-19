use crate::gpui_component::{
    Root, WindowExt,
    notification::Notification,
    resizable::{h_resizable, resizable_panel},
};
use gpui::{
    App, Context, FocusHandle, Focusable, IntoElement, Pixels, Render, StyleRefinement, Window,
    div, prelude::*, px,
};

use crate::actions::*;
use crate::visual::{color, font, layout};
use crate::workspace::AppView;

mod auth_gate;
mod auth_gate_signed_out;
mod auth_preview;
pub(crate) mod header;
mod inbox;
mod inbox_body;
mod inbox_filters;
mod inbox_header;
mod inbox_page_footer;
mod inbox_pull_request_row;
mod inbox_search;
mod inbox_search_rows;
mod panel;
mod pending_review;
mod pull_request_details_header;
mod rate_limits;
mod region;
mod review_action_comment_dialog;
mod settings;
mod settings_account;
mod settings_auth_methods;
mod settings_auth_status;
mod stacks;
mod status_bar;
#[cfg(test)]
mod tests;

pub(crate) use region::WorkspaceRegions;

struct ErrorToast;

impl Focusable for AppView {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
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

        if !self.pending_error_toasts.is_empty() {
            let messages = std::mem::take(&mut self.pending_error_toasts);
            if window.root::<Root>().flatten().is_some() {
                // Pushing updates the Root entity, which must not happen mid-draw.
                window.defer(cx, move |window, cx| {
                    for message in messages {
                        window.push_notification(
                            Notification::error(message.clone()).id1::<ErrorToast>(message),
                            cx,
                        );
                    }
                });
            }
        }

        let regions = self
            .regions
            .get_or_insert_with(|| WorkspaceRegions::new(cx));
        let inbox_region = regions.inbox.clone();
        let changed_files_region = regions.changed_files.clone();
        let panel_region = regions.panel.clone();
        let panel_style = StyleRefinement::default().flex_1().min_h_0().min_w_0();

        let selected_pr = self.selected_pull_request().cloned();
        let show_auth_gate = self.github_auth_gate_visible();
        let content = if show_auth_gate {
            self.render_github_auth_gate(cx).into_any_element()
        } else {
            let mut pull_request_workspace =
                div().flex_1().flex().flex_col().min_h_0().min_w_0().gap_3();

            if let Some(pr) = selected_pr.as_ref() {
                pull_request_workspace =
                    pull_request_workspace.child(self.render_pull_request_details_header(pr, cx));
            }

            let panel_workspace = if self.active_tab == PanelTab::Diff && selected_pr.is_some() {
                div()
                    .flex()
                    .flex_1()
                    .min_h_0()
                    .min_w_0()
                    .child(
                        h_resizable("changed-files-diff-resizable")
                            .child(
                                resizable_panel()
                                    .size(px(layout::PULL_REQUEST_DETAILS_WIDTH))
                                    .size_range(
                                        px(layout::PULL_REQUEST_DETAILS_MIN_WIDTH)..Pixels::MAX,
                                    )
                                    .flex_none()
                                    .child(
                                        changed_files_region
                                            .cached(StyleRefinement::default().size_full()),
                                    ),
                            )
                            .child(
                                resizable_panel()
                                    .size_range(
                                        px(layout::PULL_REQUEST_DIFF_MIN_WIDTH)..Pixels::MAX,
                                    )
                                    .pl_3()
                                    .child(panel_region.cached(panel_style)),
                            ),
                    )
                    .into_any_element()
            } else {
                panel_region.cached(panel_style).into_any_element()
            };
            pull_request_workspace = pull_request_workspace.child(panel_workspace);

            let pull_request_workspace = if self.pull_request_inbox.is_visible() {
                h_resizable("pull-request-inbox-resizable")
                    .child(
                        resizable_panel()
                            .size(px(layout::PULL_REQUEST_INBOX_WIDTH))
                            .size_range(px(layout::PULL_REQUEST_INBOX_MIN_WIDTH)..Pixels::MAX)
                            .flex_none()
                            .child(inbox_region.cached(StyleRefinement::default().size_full())),
                    )
                    .child(
                        resizable_panel()
                            .size_range(px(layout::PULL_REQUEST_DIFF_MIN_WIDTH)..Pixels::MAX)
                            .pl_3()
                            .child(pull_request_workspace),
                    )
                    .into_any_element()
            } else {
                pull_request_workspace.into_any_element()
            };

            div()
                .flex()
                .flex_1()
                .min_h_0()
                .min_w_0()
                .overflow_hidden()
                .p_2()
                .child(pull_request_workspace)
                .into_any_element()
        };

        div()
            .key_context(KEY_CONTEXT)
            .track_focus(&self.focus_handle(cx))
            .on_action(cx.listener(Self::select_next))
            .on_action(cx.listener(Self::select_previous))
            .on_action(cx.listener(Self::select_stack_down))
            .on_action(cx.listener(Self::select_stack_up))
            .on_action(cx.listener(Self::cycle_panel_tab))
            .on_action(cx.listener(Self::select_overview_panel))
            .on_action(cx.listener(Self::select_diff_panel))
            .on_action(cx.listener(Self::select_commits_panel))
            .on_action(cx.listener(Self::select_checks_panel))
            .on_action(cx.listener(Self::select_actions_panel))
            .on_action(cx.listener(Self::select_logs_panel))
            .on_action(cx.listener(Self::toggle_pull_request_inbox))
            .on_action(cx.listener(Self::toggle_repository_switcher))
            .on_action(cx.listener(Self::open_pull_request_search))
            .on_action(cx.listener(Self::open_command_palette))
            .on_action(cx.listener(Self::close_panel))
            .on_action(cx.listener(Self::refresh_selected))
            .on_action(cx.listener(Self::checkout_pr))
            .on_action(cx.listener(Self::open_in_browser))
            .on_action(cx.listener(Self::approve_pr))
            .on_action(cx.listener(Self::request_changes))
            .on_action(cx.listener(Self::open_approve_comment_dialog))
            .on_action(cx.listener(Self::open_request_changes_comment_dialog))
            .on_action(cx.listener(Self::merge_pr))
            .on_action(cx.listener(Self::merge_pr_with_merge_commit))
            .on_action(cx.listener(Self::rebase_pr))
            .on_action(cx.listener(Self::toggle_merge_requirements_bypass))
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
    }
}
