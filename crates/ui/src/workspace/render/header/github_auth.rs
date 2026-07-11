use gpui::{Anchor, AnyElement, Context, Entity, IntoElement, div, prelude::*, px};
use gpui_component::{
    Icon, Sizable, StyledExt,
    avatar::Avatar,
    button::{Button, ButtonVariants},
    popover::Popover,
};

use crate::{
    actions::SignOutOfGitHub,
    github::avatar_url,
    icons::Octicon,
    visual::color,
    workspace::{AppView, GitHubAuthSource, GitHubAuthStatus},
};

impl AppView {
    pub(super) fn render_github_auth_control(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let view = cx.entity().clone();
        let status = self.auth_status().clone();
        let trigger = render_github_auth_trigger(&status);

        Popover::new("github-auth-popover")
            .appearance(false)
            .anchor(Anchor::TopRight)
            .open(self.github_auth_popover_open())
            .on_open_change({
                let view = view.clone();
                move |open, _window, cx| {
                    view.update(cx, |view, cx| {
                        if *open {
                            view.open_github_auth_popover(cx);
                        } else {
                            view.dismiss_github_auth_popover(cx);
                        }
                    });
                }
            })
            .trigger(trigger)
            .content(move |_, _window, _cx| {
                render_github_auth_popover(status.clone(), view.clone())
            })
    }
}

fn render_github_auth_trigger(status: &GitHubAuthStatus) -> Button {
    match status {
        GitHubAuthStatus::SignedIn { login, .. } => Button::new("github-account")
            .ghost()
            .small()
            .compact()
            .rounded(px(999.0))
            .tooltip("GitHub account")
            .child(render_github_account_avatar(login.as_deref(), 20.0)),
        _ => Button::new("github-auth")
            .ghost()
            .small()
            .compact()
            .icon(Octicon::MarkGithub)
            .child(status.label()),
    }
}

fn render_github_auth_popover(status: GitHubAuthStatus, view: Entity<AppView>) -> AnyElement {
    match status {
        GitHubAuthStatus::SigningIn {
            user_code,
            verification_uri,
        } => {
            render_github_device_code_popover(user_code, verification_uri, view).into_any_element()
        }
        GitHubAuthStatus::SignedIn { login, source } => {
            render_github_account_popover(login, source, view).into_any_element()
        }
        GitHubAuthStatus::SignedOut => {
            render_github_message_popover("Choose a GitHub sign-in method to continue.", false)
                .into_any_element()
        }
        GitHubAuthStatus::MissingClientId => render_github_message_popover(
            "Set HARBOR_GITHUB_OAUTH_CLIENT_ID to sign in with GitHub.",
            true,
        )
        .into_any_element(),
        GitHubAuthStatus::Failed(error) => {
            render_github_message_popover(error, true).into_any_element()
        }
        GitHubAuthStatus::Loading => div().into_any_element(),
    }
}

fn render_github_popover_frame(width: f32) -> gpui::Div {
    div()
        .w(px(width))
        .border_1()
        .border_color(color::border())
        .bg(color::elevated_background())
        .rounded_xs()
        .shadow_lg()
}

fn render_github_device_code_popover(
    user_code: String,
    verification_uri: String,
    view: Entity<AppView>,
) -> impl IntoElement {
    let mut content = div().p_3().flex().flex_col().gap_2();

    content = content
        .child(
            div()
                .text_sm()
                .text_color(color::text_muted())
                .child("Enter this GitHub device code in your browser."),
        )
        .child(
            div()
                .px_2()
                .py_2()
                .bg(color::app_background())
                .text_lg()
                .font_semibold()
                .child(user_code),
        )
        .child(
            div()
                .text_xs()
                .text_color(color::text_muted())
                .truncate()
                .child(verification_uri),
        )
        .child(
            div()
                .flex()
                .gap_2()
                .child(
                    Button::new("copy-github-device-code")
                        .small()
                        .child("Copy")
                        .on_click({
                            let view = view.clone();
                            move |_, _, cx| {
                                view.update(cx, |view, cx| {
                                    view.copy_github_device_code(cx);
                                });
                            }
                        }),
                )
                .child(
                    Button::new("open-github-device-code")
                        .small()
                        .child("Open")
                        .on_click({
                            let view = view.clone();
                            move |_, _, cx| {
                                view.update(cx, |view, cx| {
                                    view.open_github_device_verification(cx);
                                });
                            }
                        }),
                ),
        );

    render_github_popover_frame(300.0).child(content)
}

fn render_github_message_popover(message: impl Into<String>, is_error: bool) -> impl IntoElement {
    render_github_popover_frame(300.0).p_3().child(
        div()
            .text_sm()
            .text_color(if is_error {
                color::danger()
            } else {
                color::text_muted()
            })
            .child(message.into()),
    )
}

fn render_github_account_popover(
    login: Option<String>,
    source: GitHubAuthSource,
    view: Entity<AppView>,
) -> impl IntoElement {
    let account_label = login
        .clone()
        .unwrap_or_else(|| "GitHub account".to_string());

    render_github_popover_frame(200.0)
        .p_1()
        .flex()
        .flex_col()
        .child(
            div()
                .flex()
                .items_center()
                .gap_2()
                .px_2()
                .py_1p5()
                .child(render_github_account_avatar(login.as_deref(), 24.0))
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
                                .child(source.label()),
                        ),
                ),
        )
        .child(
            div()
                .mt_1()
                .border_t_1()
                .border_color(color::border())
                .pt_1()
                .child(
                    render_github_account_menu_row(
                        "github-account-settings",
                        Octicon::Gear,
                        "Settings",
                        false,
                    )
                    .on_click({
                        let view = view.clone();
                        move |_, window, cx| {
                            view.update(cx, |view, cx| {
                                view.open_github_settings(window, cx);
                            });
                        }
                    }),
                ),
        )
        .child(
            render_github_account_menu_row(
                "github-account-sign-out",
                Octicon::ArrowRight,
                "Sign out",
                true,
            )
            .on_click(move |_, window, cx| {
                view.update(cx, |view, cx| {
                    view.sign_out_of_github(&SignOutOfGitHub, window, cx);
                });
            }),
        )
}

fn render_github_account_menu_row(
    id: &'static str,
    icon: Octicon,
    label: &'static str,
    danger: bool,
) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
        .h_7()
        .w_full()
        .flex()
        .items_center()
        .gap_2()
        .rounded_xs()
        .px_2()
        .text_sm()
        .cursor_pointer()
        .text_color(if danger {
            color::danger()
        } else {
            color::text_primary()
        })
        .hover(|element| element.bg(color::row_hover()))
        .child(Icon::new(icon).xsmall().text_color(if danger {
            color::danger()
        } else {
            color::text_muted()
        }))
        .child(div().flex_1().child(label))
}

fn render_github_account_avatar(login: Option<&str>, size: f32) -> AnyElement {
    match login.and_then(avatar_url) {
        Some(avatar_url) => Avatar::new()
            .src(avatar_url)
            .name(login.unwrap_or("GitHub").to_string())
            .with_size(px(size))
            .into_any_element(),
        None => Avatar::new()
            .placeholder(Octicon::MarkGithub)
            .with_size(px(size))
            .into_any_element(),
    }
}
