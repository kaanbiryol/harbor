use gpui::{Entity, IntoElement, div, prelude::*, px};
use gpui_component::{
    Icon, Sizable, StyledExt,
    button::{Button, ButtonVariants},
};

use crate::{
    actions::SignOutOfGitHub,
    icons::Octicon,
    visual::color,
    workspace::{AppView, GitHubAuthSource, GitHubAuthStatus},
};

impl AppView {
    pub(super) fn render_github_settings_account_for_dialog(
        &self,
        view: Entity<AppView>,
    ) -> impl IntoElement {
        render_github_settings_account(self, view)
    }
}

fn render_github_settings_account(view_state: &AppView, view: Entity<AppView>) -> impl IntoElement {
    let (login, source) = match view_state.auth_status() {
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
                    .child(Icon::new(Octicon::MarkGithub).small()),
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
        matches!(view_state.auth_status(), GitHubAuthStatus::SignedIn { .. }),
        |element| {
            element.child(
                Button::new("settings-github-sign-out")
                    .small()
                    .ghost()
                    .child("Sign out")
                    .on_click({
                        let view = view.clone();
                        move |_, window, cx| {
                            view.update(cx, |view, cx| {
                                view.sign_out_of_github(&SignOutOfGitHub, window, cx);
                            });
                        }
                    }),
            )
        },
    )
}
