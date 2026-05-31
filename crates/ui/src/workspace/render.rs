use gpui::{
    App, Context, FocusHandle, Focusable, IntoElement, Render, Window, div, prelude::*, px,
};
use gpui_component::{
    Disableable, Icon, IconName, Sizable, StyledExt,
    button::{Button, ButtonVariants},
    spinner::Spinner,
};
use harbor_domain::PullRequest;

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

#[path = "render/auth_preview.rs"]
mod auth_preview;
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
#[path = "render/rate_limits.rs"]
mod rate_limits;
#[path = "render/settings.rs"]
mod settings;

use auth_preview::{render_auth_option_reason, render_signed_out_workspace_preview};
use rate_limits::{github_rate_limit_color, github_rate_limit_label, github_rate_limits_label};

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
