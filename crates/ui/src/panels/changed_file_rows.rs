use gpui::{AnyElement, Context, div, prelude::*, px};
use gpui_component::{
    Icon, IconName, Sizable, StyledExt,
    button::{Button, ButtonVariants},
};
use harbor_domain::{DiffFile, FileStatus};

use crate::{
    visual::color,
    workspace::{AppView, ChangedFileFolderRow, ChangedFileRow, changed_file_status_label},
};

const CHANGED_FILE_TREE_ROW_HEIGHT: f32 = 38.;

pub(crate) fn render_changed_folder_row(
    folder: &ChangedFileFolderRow,
    cx: &mut Context<AppView>,
) -> AnyElement {
    let folder_path = folder.path.clone();
    let chevron = if folder.expanded {
        IconName::ChevronDown
    } else {
        IconName::ChevronRight
    };
    let folder_icon = if folder.expanded {
        IconName::FolderOpen
    } else {
        IconName::FolderClosed
    };

    div()
        .id(format!("folder-row-{}", folder.path))
        .h(px(CHANGED_FILE_TREE_ROW_HEIGHT))
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
                .child(folder.name.clone()),
        )
        .child(
            div()
                .flex_none()
                .text_xs()
                .text_color(color::text_muted())
                .child(folder_review_summary(folder.file_count)),
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
    let show_status = !matches!(file.status, FileStatus::Modified | FileStatus::Changed);
    let review_button = Button::new(format!("file-reviewed-{index}"))
        .icon(Icon::new(if reviewed {
            IconName::Check
        } else {
            IconName::Eye
        }))
        .small()
        .compact()
        .tooltip(if reviewed {
            "Mark as unreviewed"
        } else {
            "Mark as reviewed"
        });
    let review_button = if reviewed {
        review_button.primary()
    } else {
        review_button.ghost()
    };

    div()
        .id(("file-row", index))
        .h(px(CHANGED_FILE_TREE_ROW_HEIGHT))
        .w_full()
        .min_w_0()
        .flex()
        .items_center()
        .overflow_hidden()
        .pl(file_tree_padding(row.depth))
        .pr_2()
        .gap_1()
        .when(selected, |element| {
            element
                .border_l_1()
                .border_color(color::accent())
                .bg(color::row_selected_subtle())
        })
        .hover(|style| style.bg(color::row_hover()))
        .on_click(cx.listener(move |view, _, _, cx| {
            view.select_file(index, cx);
        }))
        .child(
            div()
                .w(px(14.))
                .flex()
                .items_center()
                .justify_center()
                .child(
                    Icon::new(IconName::File)
                        .xsmall()
                        .text_color(color::text_muted()),
                ),
        )
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
                )
                .when(show_status, |element| {
                    element.child(
                        div()
                            .flex_none()
                            .truncate()
                            .text_xs()
                            .text_color(file_status_color(file.status))
                            .child(changed_file_status_label(file.status)),
                    )
                }),
        )
        .child(
            div()
                .flex_none()
                .flex()
                .items_center()
                .gap_1()
                .text_xs()
                .child(
                    div()
                        .text_color(diff_stat_color(file.additions, color::success()))
                        .child(format!("+{}", file.additions)),
                )
                .child(
                    div()
                        .text_color(diff_stat_color(file.deletions, color::danger()))
                        .child(format!("-{}", file.deletions)),
                ),
        )
        .child(
            review_button
                .on_click(cx.listener(move |view, _, _, cx| {
                    view.toggle_changed_file_reviewed(index, cx);
                }))
                .when(!reviewed, |element| element.opacity(0.32)),
        )
        .into_any_element()
}

fn folder_review_summary(file_count: usize) -> String {
    if file_count == 1 {
        "1 file".to_string()
    } else {
        format!("{file_count} files")
    }
}

fn file_tree_padding(depth: usize) -> gpui::Pixels {
    px(10. + depth as f32 * 16.)
}

fn file_status_color(status: FileStatus) -> gpui::Rgba {
    match status {
        FileStatus::Added => color::success(),
        FileStatus::Removed => color::danger(),
        FileStatus::Renamed | FileStatus::Copied => color::accent(),
        FileStatus::Modified | FileStatus::Changed => color::text_muted(),
        FileStatus::Unchanged => color::text_muted(),
    }
}

fn diff_stat_color(count: u32, active_color: gpui::Rgba) -> gpui::Rgba {
    if count == 0 {
        color::text_muted()
    } else {
        active_color
    }
}
