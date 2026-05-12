use gpui::{Div, IntoElement, div, prelude::*, px};
use gpui_component::StyledExt;

use crate::visual::{Tone, color, tone_colors};

pub(crate) fn render_panel_header(
    title: impl Into<String>,
    metadata: Option<String>,
) -> impl IntoElement {
    div()
        .flex()
        .items_center()
        .justify_between()
        .gap_3()
        .child(
            div()
                .min_w_0()
                .flex_1()
                .truncate()
                .font_medium()
                .text_color(color::text_primary())
                .child(title.into()),
        )
        .when_some(metadata, |element, metadata| {
            element.child(
                div()
                    .flex_none()
                    .max_w(px(280.0))
                    .truncate()
                    .text_xs()
                    .text_color(color::text_muted())
                    .child(metadata),
            )
        })
}

pub(crate) fn render_panel_card() -> Div {
    div()
        .border_1()
        .border_color(color::border())
        .bg(color::content_background())
}

pub(crate) fn render_empty_panel_card(message: impl Into<String>) -> impl IntoElement {
    render_panel_card()
        .p_3()
        .text_color(color::text_muted())
        .child(message.into())
}

pub(crate) fn render_error_panel_card(message: impl Into<String>) -> impl IntoElement {
    div()
        .border_1()
        .border_color(color::danger_background())
        .bg(color::danger_background())
        .p_3()
        .text_color(color::danger())
        .child(message.into())
}

pub(crate) fn render_status_pill(label: impl Into<String>, tone: Tone) -> impl IntoElement {
    let colors = tone_colors(tone);

    div()
        .flex_none()
        .rounded_xs()
        .border_1()
        .border_color(colors.border)
        .bg(colors.background)
        .px_1()
        .py_0p5()
        .text_xs()
        .font_medium()
        .text_color(colors.text)
        .child(label.into())
}

pub(crate) fn render_metric_pill(
    label: impl Into<String>,
    value: usize,
    tone: Tone,
) -> impl IntoElement {
    let label = label.into();

    render_status_pill(format!("{label} {value}"), tone)
}

pub(crate) fn render_key_hint(label: impl Into<String>) -> impl IntoElement {
    div()
        .rounded_xs()
        .border_1()
        .border_color(color::border_strong())
        .bg(color::input_background())
        .px_1()
        .py_0p5()
        .text_xs()
        .font_medium()
        .text_color(color::text_secondary())
        .child(label.into())
}
