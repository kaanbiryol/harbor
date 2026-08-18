use gpui::{
    AnyElement, Div, ElementId, IntoElement, ListState, RenderOnce, SharedString, div, prelude::*,
    px,
};
use gpui_component::{Icon, Selectable, Sizable, StyledExt, skeleton::Skeleton, tooltip::Tooltip};

use crate::icons::Octicon;
use crate::visual::{Tone, color, tone_colors};

#[derive(IntoElement)]
pub(crate) struct ImmediateTooltip<T>
where
    T: IntoElement + 'static,
{
    id: ElementId,
    label: SharedString,
    trigger: T,
}

impl<T> ImmediateTooltip<T>
where
    T: IntoElement + 'static,
{
    pub(crate) fn new(
        id: impl Into<ElementId>,
        label: impl Into<SharedString>,
        trigger: T,
    ) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            trigger,
        }
    }
}

impl<T> Selectable for ImmediateTooltip<T>
where
    T: IntoElement + Selectable + 'static,
{
    fn selected(mut self, selected: bool) -> Self {
        self.trigger = self.trigger.selected(selected);
        self
    }

    fn is_selected(&self) -> bool {
        self.trigger.is_selected()
    }
}

impl<T> RenderOnce for ImmediateTooltip<T>
where
    T: IntoElement + 'static,
{
    fn render(self, _: &mut gpui::Window, _: &mut gpui::App) -> impl IntoElement {
        let label = self.label;

        div()
            .id(self.id)
            .child(self.trigger)
            .tooltip(move |window, cx| Tooltip::new(label.clone()).build(window, cx))
    }
}

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
        .w_full()
        .min_w_0()
        .border_1()
        .border_color(color::border())
        .bg(color::content_background())
}

pub(crate) fn render_empty_state(
    icon: Octicon,
    title: impl Into<String>,
    description: impl Into<String>,
) -> Div {
    div()
        .flex_1()
        .min_h_0()
        .w_full()
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .gap_2()
        .px_5()
        .py_8()
        .text_center()
        .child(
            div()
                .mb_2()
                .size(px(44.0))
                .flex()
                .items_center()
                .justify_center()
                .rounded_full()
                .border_1()
                .border_color(color::border_strong())
                .bg(color::row_selected_subtle())
                .child(Icon::new(icon).large().text_color(color::accent())),
        )
        .child(
            div()
                .text_lg()
                .font_semibold()
                .text_color(color::text_primary())
                .child(title.into()),
        )
        .child(
            div()
                .max_w(px(420.0))
                .text_sm()
                .text_color(color::text_muted())
                .child(description.into()),
        )
}

pub(crate) fn render_loading_panel_skeleton(
    selector: &'static str,
    row_count: usize,
    row_height: f32,
) -> AnyElement {
    render_panel_card()
        .flex()
        .flex_col()
        .flex_1()
        .min_h_0()
        .overflow_hidden()
        .child(render_loading_skeleton_rows(
            selector, row_count, row_height,
        ))
        .into_any_element()
}

pub(crate) fn render_loading_skeleton_rows(
    selector: &'static str,
    row_count: usize,
    row_height: f32,
) -> AnyElement {
    div()
        .debug_selector(move || selector.to_string())
        .w_full()
        .flex()
        .flex_col()
        .overflow_hidden()
        .children(
            (0..row_count)
                .map(move |index| render_loading_skeleton_row(selector, index, row_height)),
        )
        .into_any_element()
}

fn render_loading_skeleton_row(
    selector: &'static str,
    index: usize,
    row_height: f32,
) -> AnyElement {
    let primary = Skeleton::new()
        .w(if index % 3 == 1 { px(168.0) } else { px(220.0) })
        .max_w_full()
        .h(px(10.0))
        .rounded_sm();
    let content = if row_height >= 44.0 {
        div()
            .flex_1()
            .min_w_0()
            .flex()
            .flex_col()
            .gap_2()
            .child(primary)
            .child(
                Skeleton::new()
                    .secondary()
                    .w(if index.is_multiple_of(2) {
                        px(112.0)
                    } else {
                        px(144.0)
                    })
                    .max_w_full()
                    .h(px(8.0))
                    .rounded_sm(),
            )
            .into_any_element()
    } else {
        div().flex_1().min_w_0().child(primary).into_any_element()
    };
    let leading = if row_height >= 44.0 {
        Skeleton::new().size(px(18.0)).rounded_full()
    } else {
        Skeleton::new().size(px(18.0)).rounded_sm()
    };

    div()
        .debug_selector(move || format!("{selector}-row-{index}"))
        .h(px(row_height))
        .w_full()
        .min_w_0()
        .flex_none()
        .flex()
        .items_center()
        .gap_3()
        .px_3()
        .border_b_1()
        .border_color(color::border_subtle())
        .child(leading)
        .child(content)
        .child(
            Skeleton::new()
                .secondary()
                .w(if index.is_multiple_of(2) {
                    px(52.0)
                } else {
                    px(72.0)
                })
                .h(px(9.0))
                .rounded_sm(),
        )
        .into_any_element()
}

pub(crate) fn render_error_panel_card(message: impl Into<String>) -> impl IntoElement {
    div()
        .w_full()
        .min_w_0()
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

pub(crate) fn sync_virtual_list_item_count(list_state: &ListState, item_count: usize) {
    let current_item_count = list_state.item_count();
    if current_item_count == item_count {
        return;
    }

    if current_item_count == 0 {
        list_state.reset(item_count);
    } else {
        list_state.splice(0..current_item_count, item_count);
    }
}

#[cfg(test)]
mod tests {
    use gpui::{ListAlignment, ListOffset, ListState, px};

    use super::sync_virtual_list_item_count;

    #[test]
    fn sync_virtual_list_item_count_keeps_empty_list_at_top_when_items_arrive() {
        let list_state = ListState::new(0, ListAlignment::Top, px(160.0));
        list_state.scroll_to(ListOffset {
            item_ix: 0,
            offset_in_item: px(0.0),
        });

        sync_virtual_list_item_count(&list_state, 6);

        let scroll_top = list_state.logical_scroll_top();
        assert_eq!(scroll_top.item_ix, 0);
        assert_eq!(scroll_top.offset_in_item, px(0.0));
    }
}
