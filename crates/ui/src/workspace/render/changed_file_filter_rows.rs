use gpui::{Div, Stateful, div, prelude::*, px};
use gpui_component::{Icon, IconName, Sizable};

use crate::visual::color;

const FILE_FILTER_ROW_HEIGHT: f32 = 34.0;
const FILE_FILTER_MAX_VISIBLE_ROWS: usize = 8;

pub(super) fn file_filter_list_height(row_count: usize) -> f32 {
    FILE_FILTER_ROW_HEIGHT * row_count.min(FILE_FILTER_MAX_VISIBLE_ROWS) as f32
}

pub(super) fn render_file_filter_row(
    id: impl Into<gpui::ElementId>,
    label: String,
    count: Option<usize>,
    checked: bool,
    disabled: bool,
) -> Stateful<Div> {
    div()
        .id(id)
        .h(px(FILE_FILTER_ROW_HEIGHT))
        .w_full()
        .min_w_0()
        .flex()
        .items_center()
        .justify_between()
        .gap_3()
        .rounded_xs()
        .px_2()
        .mb_1()
        .text_sm()
        .cursor_pointer()
        .when(checked && !disabled, |element| {
            element.bg(color::row_selected())
        })
        .when(disabled, |element| element.cursor_default().opacity(0.45))
        .hover(move |element| {
            if disabled {
                element
            } else {
                element.bg(color::row_hover())
            }
        })
        .child(
            div()
                .min_w_0()
                .flex()
                .items_center()
                .gap_2()
                .child(
                    div()
                        .w(px(16.))
                        .flex()
                        .items_center()
                        .justify_center()
                        .when(checked, |element| {
                            element.child(
                                Icon::new(IconName::Check)
                                    .xsmall()
                                    .text_color(color::accent()),
                            )
                        }),
                )
                .child(
                    div()
                        .min_w_0()
                        .truncate()
                        .text_color(if disabled {
                            color::text_disabled()
                        } else {
                            color::text_primary()
                        })
                        .child(label),
                ),
        )
        .when_some(count, |element, count| {
            element.child(
                div()
                    .flex_none()
                    .min_w(px(24.))
                    .px_1()
                    .text_align(gpui::TextAlign::Right)
                    .text_xs()
                    .text_color(color::text_muted())
                    .child(count.to_string()),
            )
        })
}
