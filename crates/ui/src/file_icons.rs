use gpui::{AnyElement, div, prelude::*, px};
use gpui_component::{Icon, Sizable};
use harbor_domain::FileStatus;

use crate::{icons::Octicon, visual::color};

pub(crate) fn render_file_icon(status: FileStatus) -> AnyElement {
    let icon_size = px(16.);
    let (icon, icon_color) = status_icon(status);

    div()
        .size(icon_size)
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .child(
            Icon::new(icon)
                .xsmall()
                .text_color(icon_color)
                .into_any_element(),
        )
        .into_any_element()
}

fn status_icon(status: FileStatus) -> (Octicon, gpui::Rgba) {
    match status {
        FileStatus::Added => (Octicon::FileAdded, color::success()),
        FileStatus::Removed => (Octicon::FileRemoved, color::danger()),
        FileStatus::Renamed => (Octicon::FileRenamed, color::accent()),
        FileStatus::Copied => (Octicon::File, color::accent()),
        FileStatus::Modified | FileStatus::Changed | FileStatus::Unchanged => {
            (Octicon::File, color::text_muted())
        }
    }
}
