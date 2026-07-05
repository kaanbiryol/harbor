use gpui::{App, Context, Entity, IntoElement, Window, div, prelude::*, px};
use gpui_component::{Root, StyledExt, WindowExt};

use crate::{visual::color, workspace::AppView};

impl AppView {
    pub(crate) fn open_settings_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if window.root::<Root>().flatten().is_none() {
            return;
        }

        let view = cx.entity().clone();
        let on_close_view = view.clone();
        window.open_dialog(cx, move |dialog, _, _| {
            dialog
                .title("Settings")
                .w(px(620.))
                .max_h(px(560.))
                .on_close({
                    let on_close_view = on_close_view.clone();
                    move |_, _, cx| {
                        on_close_view.update(cx, |view, cx| {
                            view.settings_open = false;
                            view.status = "Closed settings".to_string();
                            cx.notify();
                        });
                    }
                })
                .content({
                    let view = view.clone();
                    move |content, _, cx| {
                        content.child(render_settings_dialog_content(view.clone(), cx))
                    }
                })
        });
    }
}

fn render_settings_dialog_content(view: Entity<AppView>, cx: &mut App) -> impl IntoElement {
    let view_state = view.read(cx);
    let auth_switch_status = view_state.auth_switch_status().cloned();
    let mut content = div()
        .min_w_0()
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
        .child(view_state.render_github_settings_account_for_dialog(view.clone()));

    if let Some(status) = auth_switch_status {
        content = content.child(super::settings_auth_status::render_auth_switch_status(
            status,
            view.clone(),
        ));
    }

    content.child(view_state.render_auth_method_rows_for_dialog(view.clone()))
}
