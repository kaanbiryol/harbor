use gpui::{IntoElement, Rgba, div, prelude::*, px};

use crate::visual::color;

pub(super) fn render_auth_preview_skeleton_row(index: usize, selected: bool) -> impl IntoElement {
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

pub(super) fn render_auth_preview_file_row(
    label: &'static str,
    selected: bool,
) -> impl IntoElement {
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

pub(super) fn render_auth_preview_diff_row(index: usize) -> impl IntoElement {
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

pub(super) fn render_auth_preview_pill(label: &'static str, selected: bool) -> impl IntoElement {
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

pub(super) fn render_auth_preview_tab(label: &'static str, selected: bool) -> impl IntoElement {
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

pub(super) fn render_auth_preview_bar(width: f32, background: Rgba) -> impl IntoElement {
    div().h(px(7.)).w(px(width)).bg(background)
}
