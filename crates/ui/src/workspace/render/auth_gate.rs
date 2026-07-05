use gpui::{Context, IntoElement, div, prelude::*, px};
use gpui_component::{
    Icon, Sizable, StyledExt,
    button::{Button, ButtonVariants},
    spinner::Spinner,
};

use crate::{
    actions::SignInToGitHub,
    icons::Octicon,
    visual::color,
    workspace::{AppView, GitHubAuthStatus},
};

#[derive(Clone, Copy)]
enum AuthGateButton {
    SignIn,
    ShowDeviceCode,
}

impl AppView {
    pub(super) fn render_github_auth_gate(&self, cx: &mut Context<Self>) -> impl IntoElement {
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
                                        Icon::new(Octicon::MarkGithub).large().into_any_element()
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
                                .icon(Octicon::MarkGithub)
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
}
