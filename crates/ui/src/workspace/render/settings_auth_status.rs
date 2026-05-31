use gpui::{Context, Entity, IntoElement, div, prelude::*};
use gpui_component::{Sizable, StyledExt, button::Button};

use crate::{
    visual::color,
    workspace::{AppView, AuthSwitchStatus},
};

impl AppView {
    pub(super) fn render_auth_switch_status(
        &self,
        status: AuthSwitchStatus,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        render_auth_switch_status(status, cx.entity().clone())
    }
}

fn render_auth_switch_status(status: AuthSwitchStatus, view: Entity<AppView>) -> impl IntoElement {
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
