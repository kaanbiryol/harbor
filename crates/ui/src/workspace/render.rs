use std::time::{SystemTime, UNIX_EPOCH};

use gpui::{
    App, Context, FocusHandle, Focusable, IntoElement, Render, Rgba, Window, div, prelude::*, px,
};
use gpui_component::{
    Disableable, Icon, IconName, Sizable, StyledExt,
    button::{Button, ButtonVariants},
    spinner::Spinner,
};
use harbor_domain::PullRequest;
use harbor_github::GitHubRateLimitStatus;

use crate::actions::*;
use crate::panels::{
    DiffPanelRenderInput, render_actions_panel, render_checks_panel, render_diff_panel,
    render_logs_panel, render_review_panel,
};
use crate::visual::{color, font};
use crate::workspace::{AppView, GitHubAuthStatus, GitHubCliAvailability};

const SHOW_STATUS_BAR_RATE_LIMITS: bool = true;

#[derive(Clone, Copy)]
enum AuthGateButton {
    SignIn,
    ShowDeviceCode,
}

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
#[path = "render/settings.rs"]
mod settings;

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

        if self.active_tab == PanelTab::Diff {
            self.sync_diff_list_items(cx);
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
            .child(self.render_title_bar(cx))
            .child(content)
            .when(!show_auth_gate, |element| {
                element.child(self.render_status_bar(cx))
            })
            .when(self.settings_open(), |element| {
                element.child(self.render_settings_overlay(cx))
            })
    }
}

impl AppView {
    fn render_github_auth_gate(&self, cx: &mut Context<Self>) -> impl IntoElement {
        if matches!(self.auth_status(), GitHubAuthStatus::SignedOut) {
            return div()
                .flex_1()
                .min_h_0()
                .min_w_0()
                .p_2()
                .child(self.render_signed_out_github_gate(cx));
        }

        let (title, message, button, show_icon, show_spinner, is_error) = match self.auth_status() {
            GitHubAuthStatus::Loading => (
                Some("Checking GitHub".to_string()),
                Some("Harbor will load repositories after it finds saved GitHub auth.".to_string()),
                None,
                true,
                true,
                false,
            ),
            GitHubAuthStatus::SigningIn { .. } => {
                if self.github_auth_popover_open() {
                    (
                        Some("Finish in your browser".to_string()),
                        Some(
                            "Enter the GitHub device code to load repositories and pull requests."
                                .to_string(),
                        ),
                        None,
                        true,
                        true,
                        false,
                    )
                } else {
                    (
                        Some("Connecting to GitHub".to_string()),
                        Some("Waiting for GitHub to return the token.".to_string()),
                        Some(("Show code", AuthGateButton::ShowDeviceCode)),
                        true,
                        true,
                        false,
                    )
                }
            }
            GitHubAuthStatus::MissingClientId => (
                Some("GitHub sign in is not configured".to_string()),
                Some(
                    "Set HARBOR_GITHUB_OAUTH_CLIENT_ID to enable GitHub device login.".to_string(),
                ),
                None,
                true,
                false,
                true,
            ),
            GitHubAuthStatus::Failed(error) => (
                Some("Could not connect GitHub".to_string()),
                Some(error.clone()),
                Some(("Try again", AuthGateButton::SignIn)),
                true,
                false,
                true,
            ),
            GitHubAuthStatus::SignedOut => unreachable!("signed-out auth gate is rendered above"),
            GitHubAuthStatus::SignedIn { .. } => (
                Some("Signed in to GitHub".to_string()),
                Some("Loading repositories...".to_string()),
                None,
                true,
                true,
                false,
            ),
        };

        div().flex_1().min_h_0().min_w_0().p_2().child(
            div()
                .size_full()
                .border_1()
                .border_color(color::border())
                .bg(color::panel_background())
                .flex()
                .items_center()
                .justify_center()
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .items_center()
                        .gap_2()
                        .max_w(px(460.))
                        .text_center()
                        .when(show_icon, |element| {
                            element.child(
                                div()
                                    .mb_2()
                                    .size(px(44.))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .rounded_full()
                                    .border_1()
                                    .border_color(color::border_strong())
                                    .bg(color::row_selected_subtle())
                                    .text_color(if is_error {
                                        color::danger()
                                    } else {
                                        color::text_primary()
                                    })
                                    .child(if show_spinner {
                                        Spinner::new().large().into_any_element()
                                    } else {
                                        Icon::new(IconName::Github).large().into_any_element()
                                    }),
                            )
                        })
                        .when_some(title, |element, title| {
                            element.child(
                                div()
                                    .text_lg()
                                    .font_semibold()
                                    .text_color(color::text_primary())
                                    .child(title),
                            )
                        })
                        .when_some(message, |element, message| {
                            element.child(
                                div()
                                    .text_sm()
                                    .text_color(if is_error {
                                        color::danger()
                                    } else {
                                        color::text_muted()
                                    })
                                    .child(message),
                            )
                        })
                        .when_some(button, |element, (label, action)| {
                            let button = Button::new("github-auth-empty-state-action")
                                .icon(IconName::Github)
                                .child(label)
                                .on_click(cx.listener(move |view, _, window, cx| match action {
                                    AuthGateButton::SignIn => {
                                        view.sign_in_to_github(&SignInToGitHub, window, cx);
                                    }
                                    AuthGateButton::ShowDeviceCode => {
                                        view.open_github_auth_popover(cx);
                                    }
                                }));

                            let button = match action {
                                AuthGateButton::SignIn => button.primary(),
                                AuthGateButton::ShowDeviceCode => button,
                            };

                            element.child(button)
                        }),
                ),
        )
    }

    fn render_signed_out_github_gate(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let oauth_reason = self.github_oauth_unavailable_reason().map(str::to_string);
        let cli_reason = self
            .github_cli_availability()
            .unavailable_reason()
            .map(str::to_string);
        let oauth_disabled = oauth_reason.is_some();
        let cli_disabled = !matches!(
            self.github_cli_availability(),
            GitHubCliAvailability::Available
        );

        div()
            .size_full()
            .relative()
            .overflow_hidden()
            .border_1()
            .border_color(color::border())
            .bg(color::panel_background())
            .child(render_signed_out_workspace_preview())
            .child(
                div()
                    .absolute()
                    .inset_0()
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(
                        div()
                            .w(px(360.))
                            .p_4()
                            .flex()
                            .flex_col()
                            .gap_3()
                            .border_1()
                            .border_color(color::border_strong())
                            .bg(color::panel_background())
                            .shadow_lg()
                            .child(
                                div()
                                    .size(px(44.))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .rounded_full()
                                    .border_1()
                                    .border_color(color::border_strong())
                                    .bg(color::row_selected_subtle())
                                    .child(Icon::new(IconName::Github).large()),
                            )
                            .child(
                                div()
                                    .text_lg()
                                    .font_semibold()
                                    .text_color(color::text_primary())
                                    .child("Connect GitHub"),
                            )
                            .child(
                                div()
                                    .text_sm()
                                    .text_color(color::text_muted())
                                    .child("Choose how Harbor should authenticate with GitHub."),
                            )
                            .child(
                                Button::new("github-auth-empty-state-sign-in")
                                    .primary()
                                    .large()
                                    .icon(IconName::Github)
                                    .child("Continue with GitHub")
                                    .w_full()
                                    .disabled(oauth_disabled)
                                    .on_click(cx.listener(|view, _, window, cx| {
                                        view.sign_in_to_github(&SignInToGitHub, window, cx);
                                    })),
                            )
                            .when_some(oauth_reason, |element, reason| {
                                element.child(render_auth_option_reason(reason))
                            })
                            .child(
                                Button::new("github-auth-empty-state-gh-cli")
                                    .large()
                                    .icon(IconName::SquareTerminal)
                                    .child("Use GitHub CLI")
                                    .w_full()
                                    .disabled(cli_disabled)
                                    .on_click(cx.listener(|view, _, window, cx| {
                                        view.use_github_cli(&UseGitHubCli, window, cx);
                                    })),
                            )
                            .when_some(cli_reason, |element, reason| {
                                element.child(render_auth_option_reason(reason))
                            }),
                    ),
            )
    }

    fn render_status_bar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let inbox_toggle_icon = if self.pull_request_inbox.is_visible() {
            IconName::PanelLeft
        } else {
            IconName::PanelLeftOpen
        };
        let inbox_toggle_tooltip = if self.pull_request_inbox.is_visible() {
            "Hide pull request inbox"
        } else {
            "Show pull request inbox"
        };
        let status_label = self.status.clone();
        let (rate_limit_label, rate_limit_color) = if SHOW_STATUS_BAR_RATE_LIMITS {
            let rate_limits = self.github_api.latest_rate_limits();
            let rate_limit = self.github_api.latest_rate_limit();
            (
                github_rate_limits_label(&rate_limits)
                    .or_else(|| rate_limit.as_ref().and_then(github_rate_limit_label)),
                rate_limit
                    .as_ref()
                    .map(github_rate_limit_color)
                    .unwrap_or_else(color::text_muted),
            )
        } else {
            (None, color::text_muted())
        };

        div()
            .flex()
            .items_center()
            .gap_2()
            .px_2()
            .py_1()
            .text_xs()
            .text_color(color::text_muted())
            .border_1()
            .border_color(color::border())
            .child(
                Button::new("toggle-pull-request-inbox")
                    .ghost()
                    .small()
                    .compact()
                    .icon(inbox_toggle_icon)
                    .tooltip(inbox_toggle_tooltip)
                    .on_click(cx.listener(|view, _, window, cx| {
                        view.toggle_pull_request_inbox(&TogglePullRequestInbox, window, cx);
                    })),
            )
            .child(div().min_w_0().flex_1().truncate().child(status_label))
            .when_some(rate_limit_label, |element, label| {
                element.child(
                    div()
                        .flex_none()
                        .max_w(px(260.))
                        .truncate()
                        .text_color(rate_limit_color)
                        .child(label),
                )
            })
    }

    fn render_panel(&self, pr: Option<&PullRequest>, cx: &mut Context<Self>) -> impl IntoElement {
        let view = cx.entity().clone();

        div()
            .flex_1()
            .flex()
            .flex_col()
            .min_h_0()
            .min_w_0()
            .border_1()
            .border_color(color::border())
            .bg(color::panel_background())
            .overflow_hidden()
            .child(
                div()
                    .flex()
                    .gap_2()
                    .p_2()
                    .border_1()
                    .border_color(color::border())
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
                                    .rounded_xs()
                                    .text_sm()
                                    .text_color(if active {
                                        color::text_primary()
                                    } else {
                                        color::text_secondary()
                                    })
                                    .cursor_pointer()
                                    .when(active, |element| {
                                        element
                                            .border_1()
                                            .border_color(color::border_strong())
                                            .bg(color::row_selected())
                                    })
                                    .hover(move |element| {
                                        if active {
                                            element
                                        } else {
                                            element.bg(color::row_hover())
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
                                DiffPanelRenderInput {
                                    files: &self.detail_state.files,
                                    diffs: &self.detail_state.diffs,
                                    visible_file_indices: &visible_file_indices,
                                    reviewed_file_paths: &self.reviewed_file_paths,
                                    review_threads: &self.review_state.review_threads,
                                    review_composer: self
                                        .review_state
                                        .review_composer_state
                                        .inline_composer(),
                                    active_file_index: self.active_file_index(),
                                    is_loading: self.detail_state.files_loading(),
                                    error: self.detail_state.files_error(),
                                    list_state: self.diff_list_state.clone(),
                                    list_items: &self.diff_list_items,
                                },
                                cx,
                            )
                            .into_any_element()
                        }
                        PanelTab::Review => render_review_panel(
                            &self.review_state.pull_request_reviews,
                            &self.review_state.review_threads,
                            self.review_state.reviews_loading(),
                            self.review_state.reviews_error(),
                            self.review_list_scroll.clone(),
                            cx,
                        )
                        .into_any_element(),
                        PanelTab::Checks => render_checks_panel(
                            pr.map(|pr| pr.checks_summary).unwrap_or_default(),
                            &self.detail_state.check_runs,
                            self.detail_state.checks_loading(),
                            self.detail_state.checks_error(),
                        )
                        .into_any_element(),
                        PanelTab::Actions => render_actions_panel(
                            pr,
                            &self.detail_state.workflow_runs,
                            self.detail_state.workflows_loading(),
                            self.detail_state.workflows_error(),
                            self.action_runtime.workflow_action_error(),
                            self.action_runtime.workflow_action_running(),
                            cx,
                        )
                        .into_any_element(),
                        PanelTab::Logs => render_logs_panel(
                            self.selected_workflow_run_for_logs(),
                            &self.detail_state.workflow_jobs,
                            self.detail_state.log_state.chunk(),
                            self.detail_state.log_state.is_loading(),
                            self.detail_state.log_state.error(),
                            self.detail_state.log_state.list_scroll.clone(),
                            cx,
                        )
                        .into_any_element(),
                    }),
            )
    }
}

fn render_auth_option_reason(reason: String) -> impl IntoElement {
    div()
        .text_xs()
        .text_color(color::text_muted())
        .child(reason)
}

fn render_signed_out_workspace_preview() -> impl IntoElement {
    div()
        .absolute()
        .inset_0()
        .p_3()
        .flex()
        .gap_2()
        .opacity(0.58)
        .child(render_auth_preview_inbox())
        .child(render_auth_preview_details())
        .child(render_auth_preview_diff())
}

fn render_auth_preview_inbox() -> impl IntoElement {
    div()
        .h_full()
        .w(px(310.))
        .min_w(px(240.))
        .flex_none()
        .overflow_hidden()
        .border_1()
        .border_color(color::border())
        .bg(color::content_background())
        .flex()
        .flex_col()
        .child(
            div()
                .h(px(76.))
                .flex_none()
                .border_b_1()
                .border_color(color::border())
                .p_3()
                .flex()
                .flex_col()
                .gap_3()
                .child(
                    div()
                        .flex()
                        .items_center()
                        .justify_between()
                        .child(
                            div()
                                .text_sm()
                                .font_semibold()
                                .text_color(color::text_secondary())
                                .child("Pull requests"),
                        )
                        .child(render_auth_preview_bar(34., color::row_selected())),
                )
                .child(
                    div()
                        .flex()
                        .gap_2()
                        .child(render_auth_preview_pill("Open", true))
                        .child(render_auth_preview_pill("Needs review", false)),
                ),
        )
        .child(
            div()
                .flex_1()
                .min_h_0()
                .children((0..9).map(|index| render_auth_preview_skeleton_row(index, index == 1))),
        )
}

fn render_auth_preview_details() -> impl IntoElement {
    div()
        .h_full()
        .w(px(380.))
        .min_w(px(280.))
        .flex_none()
        .overflow_hidden()
        .border_1()
        .border_color(color::border())
        .bg(color::panel_background())
        .flex()
        .flex_col()
        .child(
            div()
                .border_b_1()
                .border_color(color::border())
                .p_3()
                .flex()
                .flex_col()
                .gap_2()
                .child(render_auth_preview_bar(260., color::row_selected()))
                .child(render_auth_preview_bar(180., color::border_strong()))
                .child(
                    div()
                        .mt_2()
                        .flex()
                        .gap_2()
                        .child(render_auth_preview_pill("review", false))
                        .child(render_auth_preview_pill("checks", false))
                        .child(render_auth_preview_pill("merge", false)),
                ),
        )
        .child(
            div()
                .p_3()
                .border_b_1()
                .border_color(color::border())
                .flex()
                .items_center()
                .justify_between()
                .child(
                    div()
                        .text_sm()
                        .font_semibold()
                        .text_color(color::text_secondary())
                        .child("Changed files"),
                )
                .child(render_auth_preview_bar(44., color::border_strong())),
        )
        .child(
            div()
                .p_2()
                .flex()
                .flex_col()
                .gap_1()
                .child(render_auth_preview_file_row("src", false))
                .child(render_auth_preview_file_row("workspace.rs", true))
                .child(render_auth_preview_file_row("github.rs", false))
                .child(render_auth_preview_file_row("auth.rs", false)),
        )
}

fn render_auth_preview_diff() -> impl IntoElement {
    div()
        .h_full()
        .flex_1()
        .min_w(px(360.))
        .overflow_hidden()
        .border_1()
        .border_color(color::border())
        .bg(color::content_background())
        .flex()
        .flex_col()
        .child(
            div()
                .h(px(58.))
                .flex_none()
                .border_b_1()
                .border_color(color::border())
                .p_2()
                .flex()
                .items_center()
                .gap_2()
                .child(render_auth_preview_tab("Diff", true))
                .child(render_auth_preview_tab("Review", false))
                .child(render_auth_preview_tab("Checks", false))
                .child(render_auth_preview_tab("Actions", false))
                .child(render_auth_preview_tab("Logs", false)),
        )
        .child(
            div()
                .p_3()
                .border_b_1()
                .border_color(color::border())
                .flex()
                .items_center()
                .justify_between()
                .child(
                    div()
                        .text_sm()
                        .font_semibold()
                        .text_color(color::text_secondary())
                        .child("Unified diff preview"),
                )
                .child(render_auth_preview_bar(52., color::border_strong())),
        )
        .child(
            div().flex_1().min_h_0().p_3().child(
                div()
                    .border_1()
                    .border_color(color::border())
                    .overflow_hidden()
                    .children((0..18).map(render_auth_preview_diff_row)),
            ),
        )
}

fn render_auth_preview_skeleton_row(index: usize, selected: bool) -> impl IntoElement {
    let title_widths = [186., 224., 154., 205., 168.];
    let meta_widths = [96., 128., 112., 84., 140.];

    div()
        .h(px(52.))
        .border_b_1()
        .border_color(color::border_subtle())
        .px_3()
        .flex()
        .flex_col()
        .justify_center()
        .gap_2()
        .when(selected, |element| element.bg(color::row_selected()))
        .child(render_auth_preview_bar(
            title_widths[index % title_widths.len()],
            color::border_strong(),
        ))
        .child(render_auth_preview_bar(
            meta_widths[index % meta_widths.len()],
            color::border(),
        ))
}

fn render_auth_preview_file_row(label: &'static str, selected: bool) -> impl IntoElement {
    div()
        .h(px(34.))
        .px_2()
        .flex()
        .items_center()
        .justify_between()
        .when(selected, |element| element.bg(color::row_selected()))
        .child(
            div()
                .min_w_0()
                .truncate()
                .text_sm()
                .text_color(color::text_secondary())
                .child(label),
        )
        .child(render_auth_preview_bar(42., color::border_strong()))
}

fn render_auth_preview_diff_row(index: usize) -> impl IntoElement {
    let removed = index % 4 == 1;
    let added = index % 4 == 2;
    let background = if removed {
        color::danger_background()
    } else if added {
        color::success_background()
    } else {
        color::content_background()
    };
    let marker = if removed {
        "-"
    } else if added {
        "+"
    } else {
        " "
    };
    let line_widths = [320., 460., 260., 520., 380., 300.];

    div()
        .h(px(27.))
        .border_b_1()
        .border_color(color::border_subtle())
        .bg(background)
        .flex()
        .items_center()
        .gap_3()
        .px_3()
        .font_family(font::MONO)
        .text_xs()
        .child(
            div()
                .w(px(26.))
                .text_color(color::text_muted())
                .child(format!("{}", index + 1)),
        )
        .child(
            div()
                .w(px(10.))
                .text_color(color::text_secondary())
                .child(marker),
        )
        .child(render_auth_preview_bar(
            line_widths[index % line_widths.len()],
            color::border_strong(),
        ))
}

fn render_auth_preview_pill(label: &'static str, selected: bool) -> impl IntoElement {
    div()
        .border_1()
        .border_color(if selected {
            color::border_strong()
        } else {
            color::border()
        })
        .bg(if selected {
            color::row_selected()
        } else {
            color::content_background()
        })
        .px_2()
        .py_1()
        .text_xs()
        .text_color(color::text_secondary())
        .child(label)
}

fn render_auth_preview_tab(label: &'static str, selected: bool) -> impl IntoElement {
    div()
        .px_3()
        .py_2()
        .text_sm()
        .text_color(if selected {
            color::text_primary()
        } else {
            color::text_muted()
        })
        .when(selected, |element| {
            element
                .border_1()
                .border_color(color::border_strong())
                .bg(color::row_selected())
        })
        .child(label)
}

fn render_auth_preview_bar(width: f32, background: Rgba) -> impl IntoElement {
    div().h(px(7.)).w(px(width)).bg(background)
}

fn github_rate_limit_label(rate_limit: &GitHubRateLimitStatus) -> Option<String> {
    let resource = rate_limit.resource.as_deref().unwrap_or("api");
    let budget = match (rate_limit.remaining, rate_limit.limit) {
        (Some(remaining), Some(limit)) => format!("{remaining}/{limit}"),
        (Some(remaining), None) => format!("{remaining} left"),
        (None, Some(limit)) => format!("limit {limit}"),
        (None, None) => return None,
    };

    if github_rate_limit_should_warn(rate_limit) {
        if let Some(retry_after_seconds) = rate_limit.retry_after_seconds {
            return Some(format!(
                "github {resource}: {budget} retry {}",
                duration_label(retry_after_seconds)
            ));
        }

        if let Some(reset_label) = rate_limit.reset_epoch_seconds.and_then(reset_epoch_label) {
            return Some(format!("github {resource}: {budget} resets {reset_label}"));
        }
    }

    Some(format!("github {resource}: {budget}"))
}

fn github_rate_limits_label(rate_limits: &[GitHubRateLimitStatus]) -> Option<String> {
    if rate_limits.len() <= 1 {
        return rate_limits.first().and_then(github_rate_limit_label);
    }

    let labels = rate_limits
        .iter()
        .filter_map(|rate_limit| {
            let resource = rate_limit.resource.as_deref().unwrap_or("api");
            match (rate_limit.remaining, rate_limit.limit) {
                (Some(remaining), Some(limit)) => Some(format!("{resource} {remaining}/{limit}")),
                (Some(remaining), None) => Some(format!("{resource} {remaining} left")),
                _ => None,
            }
        })
        .collect::<Vec<_>>();

    (!labels.is_empty()).then(|| format!("github {}", labels.join(" ")))
}

fn github_rate_limit_color(rate_limit: &GitHubRateLimitStatus) -> gpui::Rgba {
    if rate_limit.remaining == Some(0) {
        color::danger()
    } else if github_rate_limit_should_warn(rate_limit) {
        color::warning()
    } else {
        color::text_muted()
    }
}

fn github_rate_limit_should_warn(rate_limit: &GitHubRateLimitStatus) -> bool {
    match (rate_limit.remaining, rate_limit.limit) {
        (Some(remaining), Some(limit)) if limit > 0 => remaining.saturating_mul(5) <= limit,
        (Some(remaining), _) => remaining <= 100,
        _ => false,
    }
}

fn reset_epoch_label(epoch_seconds: u64) -> Option<String> {
    let now = SystemTime::now().duration_since(UNIX_EPOCH).ok()?.as_secs();

    if epoch_seconds <= now {
        return Some("now".to_string());
    }

    Some(duration_label(epoch_seconds - now))
}

fn duration_label(seconds: u64) -> String {
    if seconds < 60 {
        format!("in {seconds}s")
    } else if seconds < 3600 {
        format!("in {}m", seconds.div_ceil(60))
    } else {
        format!("in {}h", seconds.div_ceil(3600))
    }
}
