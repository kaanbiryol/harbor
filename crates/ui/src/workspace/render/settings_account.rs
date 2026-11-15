use crate::gpui_component::{
    Icon, Sizable, StyledExt,
    button::{Button, ButtonVariants},
};
use gpui::{Entity, IntoElement, div, prelude::*, px};

use crate::{
    actions::SignOutOfGitHub,
    icons::Octicon,
    panels::people::render_avatar,
    visual::color,
    workspace::{AppView, GitHubAuthStatus},
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
    let detail_label = source
        .map(|source| format!("Signed in with {}", source.label()))
        .unwrap_or_else(|| "Not connected".to_string());
    let avatar = match login.as_deref() {
        Some(login) => render_avatar(login, None, 32.0),
        None => div()
            .size(px(32.))
            .flex()
            .items_center()
            .justify_center()
            .rounded_full()
            .bg(color::row_selected_subtle())
            .child(Icon::new(Octicon::MarkGithub).small())
            .into_any_element(),
    };
    let account_label = login.unwrap_or_else(|| "GitHub account".to_string());

    div()
        .min_w_0()
        .pb_4()
        .flex()
        .items_center()
        .justify_between()
        .gap_3()
        .child(
            div()
                .min_w_0()
                .flex()
                .items_center()
                .gap_3()
                .child(avatar)
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
                                .child(detail_label),
                        ),
                ),
        )
        .when(
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
