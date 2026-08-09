use gpui::{Div, ElementId, IntoElement, ListState, RenderOnce, SharedString, div, prelude::*, px};
use gpui_component::{Icon, Selectable, Sizable, StyledExt, spinner::Spinner, tooltip::Tooltip};

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

pub(crate) fn render_loading_panel_card(message: impl Into<String>) -> impl IntoElement {
    render_panel_card()
        .p_3()
        .flex()
        .items_center()
        .gap_2()
        .text_color(color::text_muted())
        .child(Spinner::new().small())
        .child(message.into())
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
