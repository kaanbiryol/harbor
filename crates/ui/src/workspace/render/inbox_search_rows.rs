use gpui::{Div, IntoElement, Stateful, div, prelude::*, px};
use gpui_component::StyledExt;

use crate::visual::color;

const PULL_REQUEST_INBOX_SEARCH_ROW_HEIGHT: f32 = 58.0;
const PULL_REQUEST_INBOX_SEARCH_MAX_VISIBLE_ROWS: usize = 7;

pub(super) fn pull_request_inbox_search_list_height(row_count: usize) -> f32 {
    PULL_REQUEST_INBOX_SEARCH_ROW_HEIGHT
        * row_count.min(PULL_REQUEST_INBOX_SEARCH_MAX_VISIBLE_ROWS) as f32
}

pub(super) fn render_pull_request_inbox_search_empty_row(label: &'static str) -> impl IntoElement {
    div()
        .px_2()
        .py_2()
        .text_sm()
        .text_color(color::text_muted())
        .child(label)
}

pub(super) fn render_pull_request_inbox_search_row(
    number: u64,
    title: String,
    author: String,
    current: bool,
    highlighted: bool,
) -> Stateful<Div> {
    div()
        .id(("pull-request-inbox-search-row", number))
        .h(px(PULL_REQUEST_INBOX_SEARCH_ROW_HEIGHT))
        .flex()
        .flex_col()
        .gap_1()
        .px_2()
        .py_2()
        .text_sm()
        .cursor_pointer()
        .when(highlighted, |element| element.bg(color::row_selected()))
        .when(current && !highlighted, |element| {
            element.bg(color::row_selected_subtle())
        })
        .hover(|element| element.bg(color::row_hover()))
        .child(
            div()
                .flex()
                .min_w_0()
                .items_center()
                .gap_2()
                .child(
                    div()
                        .flex_none()
                        .font_medium()
                        .text_color(color::text_primary())
                        .child(format!("#{number}")),
                )
                .child(
                    div()
                        .min_w_0()
                        .truncate()
                        .text_color(color::text_secondary())
                        .child(title),
                ),
        )
        .child(
            div()
                .flex()
                .min_w_0()
                .items_center()
                .gap_2()
                .text_xs()
                .text_color(color::text_muted())
                .child("by")
                .child(div().min_w_0().truncate().child(author)),
        )
}
