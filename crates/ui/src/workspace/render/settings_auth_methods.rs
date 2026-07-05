use gpui::{AnyElement, Context, IntoElement, div, prelude::*, px};
use gpui_component::{Disableable, Icon, Sizable, StyledExt, button::Button};

use crate::{
    actions::{SwitchGitHubAuthToGhCli, SwitchGitHubAuthToOAuth},
    icons::Octicon,
    visual::color,
    workspace::{AppView, GitHubAuthSource},
};

impl AppView {
    pub(super) fn render_auth_method_rows(&self, cx: &mut Context<Self>) -> impl IntoElement {
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
            Octicon::MarkGithub,
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
            Octicon::Terminal,
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
        .child(Icon::new(Octicon::Check).xsmall())
        .child("Current")
}

fn render_auth_method_row(
    icon: Octicon,
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
