#[path = "header/open_with.rs"]
mod open_with;
#[path = "header/switchers.rs"]
mod switchers;

use gpui::{Anchor, Context, IntoElement, div, prelude::*, px};
use gpui_component::{
    IconName, Sizable, StyledExt, TitleBar,
    button::{Button, ButtonVariants},
    popover::Popover,
};

use crate::{
    actions::{SignInToGitHub, SignOutOfGitHub},
    visual::color,
    workspace::{AppView, GitHubAuthStatus},
};

impl AppView {
    fn header_repository_label(&self) -> String {
        self.current_repository()
            .map(|repository| repository.name.clone())
            .unwrap_or_else(|| "repository".to_string())
    }

    pub(super) fn render_title_bar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let show_repository_switcher = !self.github_auth_gate_visible();
        let mut left_controls = div().flex().h_full().min_w_0().items_center().gap_1();
        if show_repository_switcher {
            left_controls = left_controls.child(self.render_repository_switcher(cx));
        } else {
            left_controls = left_controls.child(
                div()
                    .pl_2()
                    .font_medium()
                    .text_color(color::text_secondary())
                    .child("Harbor"),
            );
        }

        TitleBar::new()
            .bg(color::app_background())
            .border_color(color::border())
            .child(
                div()
                    .flex()
                    .h_full()
                    .w_full()
                    .min_w_0()
                    .items_center()
                    .justify_between()
                    .gap_3()
                    .pr_2()
                    .child(left_controls)
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(self.render_github_auth_control(cx))
                            .child(self.render_open_with_dropdown()),
                    ),
            )
    }

    fn render_github_auth_control(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let view = cx.entity().clone();
        let status = self.auth_status().clone();
        let label = status.label();
        let is_signed_in = matches!(status, GitHubAuthStatus::SignedIn { .. });

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
            .trigger(
                Button::new("github-auth")
                    .ghost()
                    .small()
                    .compact()
                    .icon(IconName::Github)
                    .child(label)
                    .on_click({
                        let view = view.clone();
                        let status = status.clone();
                        move |_, window, cx| {
                            view.update(cx, |view, cx| {
                                if is_signed_in {
                                    view.sign_out_of_github(&SignOutOfGitHub, window, cx);
                                } else if matches!(
                                    status,
                                    GitHubAuthStatus::SigningIn { .. }
                                        | GitHubAuthStatus::MissingClientId
                                        | GitHubAuthStatus::Failed(_)
                                ) {
                                    view.open_github_auth_popover(cx);
                                } else {
                                    view.sign_in_to_github(&SignInToGitHub, window, cx);
                                }
                            });
                        }
                    }),
            )
            .content(move |_, _window, _cx| {
                render_github_auth_popover(status.clone(), view.clone())
            })
    }
}

fn render_github_auth_popover(
    status: GitHubAuthStatus,
    view: gpui::Entity<AppView>,
) -> impl IntoElement {
    let mut content = div()
        .w(px(300.))
        .border_1()
        .border_color(color::border_strong())
        .bg(color::elevated_background())
        .p_3()
        .shadow_lg()
        .flex()
        .flex_col()
        .gap_2();

    match status {
        GitHubAuthStatus::SigningIn {
            user_code,
            verification_uri,
        } => {
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
        }
        GitHubAuthStatus::MissingClientId => {
            content = content.child(
                div()
                    .text_sm()
                    .text_color(color::danger())
                    .child("Set HARBOR_GITHUB_OAUTH_CLIENT_ID to sign in with GitHub."),
            );
        }
        GitHubAuthStatus::Failed(error) => {
            content = content.child(div().text_sm().text_color(color::danger()).child(error));
        }
        GitHubAuthStatus::Loading
        | GitHubAuthStatus::SignedOut
        | GitHubAuthStatus::SignedIn { .. } => {}
    }

    content
}
