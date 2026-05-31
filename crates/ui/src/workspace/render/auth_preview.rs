use gpui::{IntoElement, Rgba, div, prelude::*, px};
use gpui_component::StyledExt;

use crate::visual::{color, font};

pub(super) fn render_auth_option_reason(reason: String) -> impl IntoElement {
    div()
        .text_xs()
        .text_color(color::text_muted())
        .child(reason)
}

pub(super) fn render_signed_out_workspace_preview() -> impl IntoElement {
    div()
        .absolute()
        .inset_0()
        .p_3()
        .flex()
        .gap_2()
        .opacity(0.58)
        .child(render_auth_preview_inbox())
        .child(render_auth_preview_details())
        .child(render_auth_preview_diff())
}

fn render_auth_preview_inbox() -> impl IntoElement {
    div()
        .h_full()
        .w(px(310.))
        .min_w(px(240.))
        .flex_none()
        .overflow_hidden()
        .border_1()
        .border_color(color::border())
        .bg(color::content_background())
        .flex()
        .flex_col()
        .child(
            div()
                .h(px(76.))
                .flex_none()
                .border_b_1()
                .border_color(color::border())
                .p_3()
                .flex()
                .flex_col()
                .gap_3()
                .child(
                    div()
                        .flex()
                        .items_center()
                        .justify_between()
                        .child(
                            div()
                                .text_sm()
                                .font_semibold()
                                .text_color(color::text_secondary())
                                .child("Pull requests"),
                        )
                        .child(render_auth_preview_bar(34., color::row_selected())),
                )
                .child(
                    div()
                        .flex()
                        .gap_2()
                        .child(render_auth_preview_pill("Open", true))
                        .child(render_auth_preview_pill("Needs review", false)),
                ),
        )
        .child(
            div()
                .flex_1()
                .min_h_0()
                .children((0..9).map(|index| render_auth_preview_skeleton_row(index, index == 1))),
        )
}

fn render_auth_preview_details() -> impl IntoElement {
    div()
        .h_full()
        .w(px(380.))
        .min_w(px(280.))
        .flex_none()
        .overflow_hidden()
        .border_1()
        .border_color(color::border())
        .bg(color::panel_background())
        .flex()
        .flex_col()
        .child(
            div()
                .border_b_1()
                .border_color(color::border())
                .p_3()
                .flex()
                .flex_col()
                .gap_2()
                .child(render_auth_preview_bar(260., color::row_selected()))
                .child(render_auth_preview_bar(180., color::border_strong()))
                .child(
                    div()
                        .mt_2()
                        .flex()
                        .gap_2()
                        .child(render_auth_preview_pill("review", false))
                        .child(render_auth_preview_pill("checks", false))
                        .child(render_auth_preview_pill("merge", false)),
                ),
        )
        .child(
            div()
                .p_3()
                .border_b_1()
                .border_color(color::border())
                .flex()
                .items_center()
                .justify_between()
                .child(
                    div()
                        .text_sm()
                        .font_semibold()
                        .text_color(color::text_secondary())
                        .child("Changed files"),
                )
                .child(render_auth_preview_bar(44., color::border_strong())),
        )
        .child(
            div()
                .p_2()
                .flex()
                .flex_col()
                .gap_1()
                .child(render_auth_preview_file_row("src", false))
                .child(render_auth_preview_file_row("workspace.rs", true))
                .child(render_auth_preview_file_row("github.rs", false))
                .child(render_auth_preview_file_row("auth.rs", false)),
        )
}

fn render_auth_preview_diff() -> impl IntoElement {
    div()
        .h_full()
        .flex_1()
        .min_w(px(360.))
        .overflow_hidden()
        .border_1()
        .border_color(color::border())
        .bg(color::content_background())
        .flex()
        .flex_col()
        .child(
            div()
                .h(px(58.))
                .flex_none()
                .border_b_1()
                .border_color(color::border())
                .p_2()
                .flex()
                .items_center()
                .gap_2()
                .child(render_auth_preview_tab("Diff", true))
                .child(render_auth_preview_tab("Review", false))
                .child(render_auth_preview_tab("Checks", false))
                .child(render_auth_preview_tab("Actions", false))
                .child(render_auth_preview_tab("Logs", false)),
        )
        .child(
            div()
                .p_3()
                .border_b_1()
                .border_color(color::border())
                .flex()
                .items_center()
                .justify_between()
                .child(
                    div()
                        .text_sm()
                        .font_semibold()
                        .text_color(color::text_secondary())
                        .child("Unified diff preview"),
                )
                .child(render_auth_preview_bar(52., color::border_strong())),
        )
        .child(
            div().flex_1().min_h_0().p_3().child(
                div()
                    .border_1()
                    .border_color(color::border())
                    .overflow_hidden()
                    .children((0..18).map(render_auth_preview_diff_row)),
            ),
        )
}

fn render_auth_preview_skeleton_row(index: usize, selected: bool) -> impl IntoElement {
    let title_widths = [186., 224., 154., 205., 168.];
    let meta_widths = [96., 128., 112., 84., 140.];

    div()
        .h(px(52.))
        .border_b_1()
        .border_color(color::border_subtle())
        .px_3()
        .flex()
        .flex_col()
        .justify_center()
        .gap_2()
        .when(selected, |element| element.bg(color::row_selected()))
        .child(render_auth_preview_bar(
            title_widths[index % title_widths.len()],
            color::border_strong(),
        ))
        .child(render_auth_preview_bar(
            meta_widths[index % meta_widths.len()],
            color::border(),
        ))
}

fn render_auth_preview_file_row(label: &'static str, selected: bool) -> impl IntoElement {
    div()
        .h(px(34.))
        .px_2()
        .flex()
        .items_center()
        .justify_between()
        .when(selected, |element| element.bg(color::row_selected()))
        .child(
            div()
                .min_w_0()
                .truncate()
                .text_sm()
                .text_color(color::text_secondary())
                .child(label),
        )
        .child(render_auth_preview_bar(42., color::border_strong()))
}

fn render_auth_preview_diff_row(index: usize) -> impl IntoElement {
    let removed = index % 4 == 1;
    let added = index % 4 == 2;
    let background = if removed {
        color::danger_background()
    } else if added {
        color::success_background()
    } else {
        color::content_background()
    };
    let marker = if removed {
        "-"
    } else if added {
        "+"
    } else {
        " "
    };
    let line_widths = [320., 460., 260., 520., 380., 300.];

    div()
        .h(px(27.))
        .border_b_1()
        .border_color(color::border_subtle())
        .bg(background)
        .flex()
        .items_center()
        .gap_3()
        .px_3()
        .font_family(font::MONO)
        .text_xs()
        .child(
            div()
                .w(px(26.))
                .text_color(color::text_muted())
                .child(format!("{}", index + 1)),
        )
        .child(
            div()
                .w(px(10.))
                .text_color(color::text_secondary())
                .child(marker),
        )
        .child(render_auth_preview_bar(
            line_widths[index % line_widths.len()],
            color::border_strong(),
        ))
}

fn render_auth_preview_pill(label: &'static str, selected: bool) -> impl IntoElement {
    div()
        .border_1()
        .border_color(if selected {
            color::border_strong()
        } else {
            color::border()
        })
        .bg(if selected {
            color::row_selected()
        } else {
            color::content_background()
        })
        .px_2()
        .py_1()
        .text_xs()
        .text_color(color::text_secondary())
        .child(label)
}

fn render_auth_preview_tab(label: &'static str, selected: bool) -> impl IntoElement {
    div()
        .px_3()
        .py_2()
        .text_sm()
        .text_color(if selected {
            color::text_primary()
        } else {
            color::text_muted()
        })
        .when(selected, |element| {
            element
                .border_1()
                .border_color(color::border_strong())
                .bg(color::row_selected())
        })
        .child(label)
}

fn render_auth_preview_bar(width: f32, background: Rgba) -> impl IntoElement {
    div().h(px(7.)).w(px(width)).bg(background)
}
