use gpui::{AnyElement, Context, IntoElement, div, prelude::*, px, rgba};
use gpui_component::{
    Disableable, Icon, IconName, Sizable, StyledExt,
    button::{Button, ButtonVariants},
};

use crate::{
    actions::{CloseSettings, SignOutOfGitHub, SwitchGitHubAuthToGhCli, SwitchGitHubAuthToOAuth},
    visual::color,
    workspace::{AppView, AuthSwitchStatus, GitHubAuthSource, GitHubAuthStatus},
};

impl AppView {
    pub(super) fn render_settings_overlay(&self, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .absolute()
            .inset_0()
            .occlude()
            .on_any_mouse_down(|_, _, cx| {
                cx.stop_propagation();
            })
            .bg(rgba(0x00000080))
            .flex()
            .items_center()
            .justify_center()
            .child(
                div()
                    .w(px(620.))
                    .max_h(px(560.))
                    .border_1()
                    .border_color(color::border_strong())
                    .bg(color::panel_background())
                    .rounded_xs()
                    .shadow_lg()
                    .flex()
                    .flex_col()
                    .child(self.render_settings_header(cx))
                    .child(self.render_github_settings(cx)),
            )
    }

    fn render_settings_header(&self, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .h(px(44.))
            .flex()
            .items_center()
            .justify_between()
            .border_b_1()
            .border_color(color::border())
            .px_3()
            .child(
                div()
                    .text_base()
                    .font_semibold()
                    .text_color(color::text_primary())
                    .child("Settings"),
            )
            .child(
                Button::new("close-settings")
                    .ghost()
                    .small()
                    .compact()
                    .icon(IconName::Close)
                    .tooltip("Close settings")
                    .on_click(cx.listener(|view, _, window, cx| {
                        view.close_settings(&CloseSettings, window, cx);
                    })),
            )
    }

    fn render_github_settings(&self, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .min_w_0()
            .p_3()
            .flex()
            .flex_col()
            .gap_3()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(
                        div()
                            .text_sm()
                            .font_medium()
                            .text_color(color::text_primary())
                            .child("GitHub authentication"),
                    )
                    .child(
                        div()
                            .text_sm()
                            .text_color(color::text_muted())
                            .child("Choose how Harbor talks to GitHub."),
                    ),
            )
            .child(self.render_github_settings_account(cx))
            .when_some(self.auth_switch_status().cloned(), |element, status| {
                element.child(render_auth_switch_status(status, cx.entity().clone()))
            })
            .child(self.render_auth_method_rows(cx))
    }

    fn render_github_settings_account(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let (login, source) = match self.auth_status() {
            GitHubAuthStatus::SignedIn { login, source } => (login.clone(), Some(*source)),
            _ => (None, None),
        };
        let account_label = login.unwrap_or_else(|| "GitHub account".to_string());
        let source_label = source
            .map(GitHubAuthSource::label)
            .unwrap_or("Not connected");

        let mut account_row = div()
            .min_w_0()
            .border_1()
            .border_color(color::border())
            .bg(color::content_background())
            .rounded_xs()
            .px_3()
            .py_2()
            .flex()
            .items_center()
            .justify_between()
            .gap_3();

        account_row = account_row.child(
            div()
                .min_w_0()
                .flex()
                .items_center()
                .gap_3()
                .child(
                    div()
                        .size(px(32.))
                        .flex()
                        .items_center()
                        .justify_center()
                        .rounded_full()
                        .border_1()
                        .border_color(color::border())
                        .bg(color::row_selected_subtle())
                        .child(Icon::new(IconName::Github).small()),
                )
                .child(
                    div()
                        .min_w_0()
                        .flex()
                        .flex_col()
                        .child(
                            div()
                                .truncate()
                                .text_sm()
                                .font_medium()
                                .text_color(color::text_primary())
                                .child(account_label),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(color::text_muted())
                                .child(source_label),
                        ),
                ),
        );

        account_row.when(
            matches!(self.auth_status(), GitHubAuthStatus::SignedIn { .. }),
            |element| {
                element.child(
                    Button::new("settings-github-sign-out")
                        .small()
                        .ghost()
                        .child("Sign out")
                        .on_click(cx.listener(|view, _, window, cx| {
                            view.sign_out_of_github(&SignOutOfGitHub, window, cx);
                        })),
                )
            },
        )
    }

    fn render_auth_method_rows(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let current_source = self.current_github_auth_source();
        let oauth_reason = if current_source == Some(GitHubAuthSource::OAuth) {
            None
        } else {
            self.github_oauth_unavailable_reason().map(str::to_string)
        };
        let cli_reason = if current_source == Some(GitHubAuthSource::GhCli) {
            None
        } else {
            self.github_cli_availability()
                .unavailable_reason()
                .map(str::to_string)
        };

        let mut rows = div().flex().flex_col().gap_2();

        rows = rows.child(render_auth_method_row(
            IconName::Github,
            "GitHub OAuth",
            "Device login. Harbor stores the OAuth token in app credentials.",
            current_source == Some(GitHubAuthSource::OAuth),
            oauth_reason.as_deref(),
            render_auth_method_action(
                "settings-switch-github-oauth",
                current_source == Some(GitHubAuthSource::OAuth),
                oauth_reason.is_some(),
                cx.listener(|view, _, window, cx| {
                    view.switch_github_auth_to_oauth(&SwitchGitHubAuthToOAuth, window, cx);
                }),
            ),
        ));

        rows = rows.child(render_auth_method_row(
            IconName::SquareTerminal,
            "GitHub CLI",
            "Reuse your authenticated gh session. Harbor does not copy its token.",
            current_source == Some(GitHubAuthSource::GhCli),
            cli_reason.as_deref(),
            render_auth_method_action(
                "settings-switch-github-cli",
                current_source == Some(GitHubAuthSource::GhCli),
                cli_reason.is_some(),
                cx.listener(|view, _, window, cx| {
                    view.switch_github_auth_to_gh_cli(&SwitchGitHubAuthToGhCli, window, cx);
                }),
            ),
        ));

        rows
    }
}

fn render_auth_method_action(
    id: &'static str,
    selected: bool,
    disabled: bool,
    on_click: impl Fn(&gpui::ClickEvent, &mut gpui::Window, &mut gpui::App) + 'static,
) -> AnyElement {
    if selected {
        render_current_auth_badge().into_any_element()
    } else {
        Button::new(id)
            .small()
            .child("Switch")
            .disabled(disabled)
            .on_click(on_click)
            .into_any_element()
    }
}

fn render_current_auth_badge() -> impl IntoElement {
    div()
        .h_6()
        .px_2()
        .flex()
        .items_center()
        .gap_1()
        .rounded_xs()
        .bg(color::row_selected())
        .text_xs()
        .font_medium()
        .text_color(color::text_primary())
        .child(Icon::new(IconName::Check).xsmall())
        .child("Current")
}

fn render_auth_switch_status(
    status: AuthSwitchStatus,
    view: gpui::Entity<AppView>,
) -> impl IntoElement {
    let is_error = matches!(status, AuthSwitchStatus::Failed(_));
    div()
        .border_1()
        .border_color(if is_error {
            color::danger()
        } else {
            color::border_strong()
        })
        .bg(if is_error {
            color::danger_background()
        } else {
            color::content_background()
        })
        .rounded_xs()
        .p_2()
        .flex()
        .flex_col()
        .gap_1()
        .child(
            div()
                .text_sm()
                .font_medium()
                .text_color(if is_error {
                    color::danger()
                } else {
                    color::text_primary()
                })
                .child(status.label()),
        )
        .child(
            div()
                .text_sm()
                .text_color(color::text_muted())
                .child(status.message()),
        )
        .when_some(
            waiting_oauth_status(status),
            |element, (user_code, verification_uri)| {
                element
                    .child(
                        div()
                            .px_2()
                            .py_1()
                            .bg(color::app_background())
                            .font_semibold()
                            .text_base()
                            .child(user_code),
                    )
                    .child(
                        div()
                            .text_xs()
                            .truncate()
                            .text_color(color::text_muted())
                            .child(verification_uri),
                    )
                    .child(
                        div()
                            .flex()
                            .gap_2()
                            .child(
                                Button::new("settings-copy-github-oauth-code")
                                    .small()
                                    .child("Copy")
                                    .on_click({
                                        let view = view.clone();
                                        move |_, _, cx| {
                                            view.update(cx, |view, cx| {
                                                view.copy_github_auth_switch_device_code(cx);
                                            });
                                        }
                                    }),
                            )
                            .child(
                                Button::new("settings-open-github-oauth-code")
                                    .small()
                                    .child("Open")
                                    .on_click({
                                        let view = view.clone();
                                        move |_, _, cx| {
                                            view.update(cx, |view, cx| {
                                                view.open_github_auth_switch_verification(cx);
                                            });
                                        }
                                    }),
                            ),
                    )
            },
        )
}

fn waiting_oauth_status(status: AuthSwitchStatus) -> Option<(String, String)> {
    match status {
        AuthSwitchStatus::WaitingOAuth {
            user_code,
            verification_uri,
        } => Some((user_code, verification_uri)),
        _ => None,
    }
}

fn render_auth_method_row(
    icon: IconName,
    title: &'static str,
    description: &'static str,
    selected: bool,
    disabled_reason: Option<&str>,
    action: impl IntoElement,
) -> impl IntoElement {
    div()
        .border_1()
        .border_color(if selected {
            color::border_strong()
        } else {
            color::border()
        })
        .bg(if selected {
            color::row_selected_subtle()
        } else {
            color::content_background()
        })
        .rounded_xs()
        .px_3()
        .py_2()
        .flex()
        .items_center()
        .justify_between()
        .gap_3()
        .child(
            div()
                .min_w_0()
                .flex()
                .items_start()
                .gap_3()
                .child(
                    div()
                        .size(px(24.))
                        .flex()
                        .items_center()
                        .justify_center()
                        .text_color(if selected {
                            color::accent()
                        } else {
                            color::text_secondary()
                        })
                        .child(Icon::new(icon).small()),
                )
                .child(
                    div()
                        .min_w_0()
                        .flex()
                        .flex_col()
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap_2()
                                .text_sm()
                                .font_medium()
                                .text_color(color::text_primary())
                                .child(title),
                        )
                        .child(
                            div()
                                .text_sm()
                                .text_color(color::text_muted())
                                .child(description),
                        )
                        .when_some(disabled_reason.map(str::to_string), |element, reason| {
                            element
                                .child(div().text_xs().text_color(color::warning()).child(reason))
                        }),
                ),
        )
        .child(div().flex_none().child(action))
}
