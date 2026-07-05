use gpui::{AnyElement, div, img, prelude::*, px};
use gpui_component::{Icon, Sizable};

use crate::{icons::Octicon, material_file_icons::material_file_icon_path, visual::color};

pub(crate) fn render_file_icon(path: &str) -> AnyElement {
    let icon_size = px(16.);

    div()
        .size(icon_size)
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .child(match material_file_icon_path(path) {
            Some(icon_path) => img(icon_path).size(icon_size).into_any_element(),
            None => Icon::new(Octicon::File)
                .xsmall()
                .text_color(color::text_muted())
                .into_any_element(),
        })
        .into_any_element()
}
