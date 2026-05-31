use gpui::{Context, IntoElement, div, prelude::*, px};
use gpui_component::{
    Disableable, Icon, IconName, Sizable, StyledExt,
    button::{Button, ButtonVariants},
};

use crate::{
    actions::{SignInToGitHub, UseGitHubCli},
    visual::color,
    workspace::{AppView, GitHubCliAvailability},
};

use super::auth_preview::{render_auth_option_reason, render_signed_out_workspace_preview};

impl AppView {
    pub(super) fn render_signed_out_github_gate(&self, cx: &mut Context<Self>) -> impl IntoElement {
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
}
