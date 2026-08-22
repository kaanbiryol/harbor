use gpui::{AnyElement, Context, div, prelude::*, px};
use gpui_component::{Icon, Sizable, StyledExt};
use harbor_domain::DiffFile;

use crate::{
    file_icons::render_file_icon,
    icons::Octicon,
    panels::{render_diff_stats, render_file_review_button},
    visual::{color, layout, leading_truncated_path},
    workspace::{AppView, ChangedFileFolderRow, ChangedFileRow},
};

pub(crate) fn render_changed_folder_row(
    folder: &ChangedFileFolderRow,
    cx: &mut Context<AppView>,
) -> AnyElement {
    let folder_path = folder.path.clone();
    let chevron = if folder.expanded {
        Octicon::ChevronDown
    } else {
        Octicon::ChevronRight
    };
    let folder_icon = if folder.expanded {
        Octicon::FileDirectoryOpen
    } else {
        Octicon::FileDirectory
    };

    div()
        .id(format!("folder-row-{}", folder.path))
        .h(px(layout::CHANGED_FILE_TREE_ROW_HEIGHT))
        .w_full()
        .min_w_0()
        .flex()
        .items_center()
        .overflow_hidden()
        .pl(file_tree_padding(folder.depth))
        .pr_2()
        .gap_2()
        .text_sm()
        .cursor_pointer()
        .hover(|style| style.bg(color::row_hover()))
        .on_click(cx.listener(move |view, _, _, cx| {
            view.toggle_changed_file_folder(folder_path.clone(), cx);
        }))
        .child(Icon::new(chevron).xsmall().text_color(color::text_muted()))
        .child(
            Icon::new(folder_icon)
                .xsmall()
                .text_color(color::text_muted()),
        )
        .child(
            div()
                .min_w_0()
                .flex_1()
                .truncate()
                .font_medium()
                .text_color(color::text_secondary())
                .child(leading_truncated_path(&folder.name, 48)),
        )
        .into_any_element()
}

pub(crate) fn render_changed_file_row(
    row: &ChangedFileRow,
    file: &DiffFile,
    selected: bool,
    reviewed: bool,
    cx: &mut Context<AppView>,
) -> AnyElement {
    let index = row.file_index;
    let review_button = render_file_review_button(format!("file-reviewed-{index}"), reviewed);

    div()
        .id(("file-row", index))
        .h(px(layout::CHANGED_FILE_TREE_ROW_HEIGHT))
        .w_full()
        .min_w_0()
        .flex()
        .items_center()
        .overflow_hidden()
        .pl(file_tree_padding(row.depth))
        .pr_2()
        .gap_2()
        .when(selected, |element| element.bg(color::row_selected_active()))
        .hover(move |style| {
            if selected {
                style
            } else {
                style.bg(color::row_hover())
            }
        })
        .on_click(cx.listener(move |view, _, _, cx| {
            view.select_file(index, cx);
        }))
        .child(render_file_icon(file.status))
        .child(
            div()
                .min_w_0()
                .flex_1()
                .flex()
                .items_center()
                .gap_1()
                .child(
                    div()
                        .min_w_0()
                        .flex_1()
                        .truncate()
                        .text_sm()
                        .text_color(if reviewed {
                            color::text_muted()
                        } else {
                            color::text_primary()
                        })
                        .child(row.name.clone()),
                ),
        )
        .child(render_diff_stats(file.additions, file.deletions))
        .child(review_button.on_click(cx.listener(move |view, _, _, cx| {
            view.toggle_changed_file_reviewed(index, cx);
        })))
        .into_any_element()
}

fn file_tree_padding(depth: usize) -> gpui::Pixels {
    px(layout::CHANGED_FILE_TREE_BASE_INDENT
        + depth as f32 * layout::CHANGED_FILE_TREE_DEPTH_INDENT)
}
