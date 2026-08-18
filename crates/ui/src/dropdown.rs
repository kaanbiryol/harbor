use gpui::{App, Div, ElementId, SharedString, div, prelude::*, px};
use gpui_component::{Icon, Sizable, StyledExt, list::ListItem};

use crate::{icons::Octicon, visual::color};

const DROPDOWN_MENU_ROW_HEIGHT: f32 = 30.0;
const DROPDOWN_MENU_MAX_VISIBLE_ROWS: usize = 9;

pub(crate) fn dropdown_menu_surface(cx: &App, width: f32) -> Div {
    div().w(px(width)).popover_style(cx)
}

pub(crate) fn dropdown_menu_section(label: impl Into<SharedString>) -> Div {
    div()
        .px_2()
        .py_1()
        .text_xs()
        .text_color(color::text_muted())
        .child(label.into())
}

pub(crate) fn dropdown_menu_separator() -> Div {
    div().my_1().border_t_1().border_color(color::border())
}

pub(crate) fn dropdown_menu_list_height(row_count: usize) -> f32 {
    DROPDOWN_MENU_ROW_HEIGHT * row_count.min(DROPDOWN_MENU_MAX_VISIBLE_ROWS) as f32
}

pub(crate) fn dropdown_menu_item(
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    count: Option<usize>,
    checked: bool,
    selected: bool,
    disabled: bool,
) -> ListItem {
    let count = count.map(|count| count.to_string());

    ListItem::new(id)
        .h(px(DROPDOWN_MENU_ROW_HEIGHT))
        .w_full()
        .min_w_0()
        .px_2()
        .py_0()
        .mb_0p5()
        .rounded_xs()
        .disabled(disabled)
        .when(selected && !disabled, |item| {
            item.bg(color::row_selected_subtle())
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
                        .flex_none()
                        .flex()
                        .items_center()
                        .justify_center()
                        .when(checked, |element| {
                            element.child(
                                Icon::new(Octicon::Check)
                                    .xsmall()
                                    .text_color(color::accent()),
                            )
                        }),
                )
                .child(
                    div()
                        .min_w_0()
                        .truncate()
                        .text_sm()
                        .text_color(if disabled {
                            color::text_disabled()
                        } else {
                            color::text_primary()
                        })
                        .child(label.into()),
                ),
        )
        .when_some(count, |item, count| {
            item.suffix(move |_, _| {
                div()
                    .flex_none()
                    .min_w(px(24.))
                    .px_1()
                    .text_align(gpui::TextAlign::Right)
                    .text_xs()
                    .text_color(color::text_muted())
                    .child(count.clone())
            })
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn caps_dropdown_lists_at_nine_visible_rows() {
        assert_eq!(dropdown_menu_list_height(0), 0.0);
        assert_eq!(dropdown_menu_list_height(4), 120.0);
        assert_eq!(dropdown_menu_list_height(12), 270.0);
    }
}
