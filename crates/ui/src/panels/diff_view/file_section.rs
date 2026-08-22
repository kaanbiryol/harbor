use gpui::{AnyElement, Entity, IntoElement, div, prelude::*, px};
use gpui_component::{Icon, Sizable, StyledExt, clipboard::Clipboard};
use harbor_domain::DiffFile;

use crate::{
    file_icons::render_file_icon,
    icons::Octicon,
    visual::{color, font, leading_truncated_path},
    workspace::AppView,
};

use super::super::{render_diff_stats, render_file_review_button};
use super::{DIFF_FILE_HEADER_HEIGHT, DIFF_ROW_HEIGHT};

pub(super) fn render_diff_file_section_header(
    file_index: usize,
    file: DiffFile,
    active: bool,
    reviewed: bool,
    expanded: bool,
    sticky: bool,
    view_entity: Entity<AppView>,
) -> AnyElement {
    let header_id = if sticky {
        format!("sticky-diff-file-header-{file_index}")
    } else {
        format!("diff-file-header-{file_index}")
    };
    let review_button = render_file_review_button(
        format!(
            "{}-diff-file-reviewed-{file_index}",
            if sticky { "sticky" } else { "row" }
        ),
        reviewed,
    );
    let review_button = review_button.on_click({
        let view_entity = view_entity.clone();
        move |_, _, cx| {
            view_entity.update(cx, |view, cx| {
                view.toggle_changed_file_reviewed(file_index, cx);
            });
            cx.stop_propagation();
        }
    });
    let path = file.path.clone();
    let display_path = leading_truncated_path(&path, 72);
    let copy_path = path.clone();
    let copied_status_path = path.clone();
    let copy_button = Clipboard::new(format!(
        "{}-diff-file-path-copy-{file_index}",
        if sticky { "sticky" } else { "row" }
    ))
    .tooltip("Copy file path")
    .value(copy_path)
    .on_copied({
        let view_entity = view_entity.clone();
        move |_, _, cx| {
            view_entity.update(cx, |view, cx| {
                view.set_status(format!("Copied {copied_status_path}"), cx);
            });
        }
    });
    let toggle_section_view_entity = view_entity.clone();
    let chevron = if expanded {
        Octicon::ChevronDown
    } else {
        Octicon::ChevronRight
    };

    div()
        .id(header_id)
        .h(px(DIFF_FILE_HEADER_HEIGHT))
        .w_full()
        .min_w_0()
        .flex()
        .items_center()
        .justify_between()
        .gap_3()
        .px_3()
        .border_b_1()
        .border_color(if active || sticky {
            color::border()
        } else {
            color::border_subtle()
        })
        .bg(if active {
            color::row_selected_active()
        } else if reviewed {
            color::content_background()
        } else {
            color::elevated_background()
        })
        .font_family(font::UI)
        .text_color(color::text_primary())
        .whitespace_nowrap()
        .cursor_pointer()
        .when(sticky, |element| element.shadow_lg())
        .hover(move |element| {
            if active {
                element
            } else {
                element.bg(color::row_hover())
            }
        })
        .on_click(move |_, _, cx| {
            toggle_section_view_entity.update(cx, |view, cx| {
                view.toggle_diff_file_section(file_index, cx);
            });
            cx.stop_propagation();
        })
        .child(
            div()
                .min_w_0()
                .flex_1()
                .flex()
                .items_center()
                .gap_2()
                .child(Icon::new(chevron).xsmall().text_color(color::text_muted()))
                .child(render_file_icon(file.status))
                .child(
                    div()
                        .min_w_0()
                        .flex_1()
                        .truncate()
                        .text_sm()
                        .font_medium()
                        .text_color(if reviewed {
                            color::text_muted()
                        } else {
                            color::text_primary()
                        })
                        .child(display_path),
                )
                .child(div().flex_none().child(copy_button)),
        )
        .child(
            div()
                .flex_none()
                .flex()
                .items_center()
                .gap_2()
                .child(render_diff_stats(file.additions, file.deletions))
                .child(review_button),
        )
        .into_any_element()
}

pub(super) fn render_diff_unavailable_row(row_index: usize) -> impl IntoElement {
    div()
        .id(format!("diff-unavailable-{row_index}"))
        .h(px(DIFF_ROW_HEIGHT))
        .w_full()
        .flex()
        .items_center()
        .px_2()
        .bg(color::content_background())
        .font_family(font::UI)
        .text_color(color::warning())
        .whitespace_nowrap()
        .child("Diff unavailable via GitHub API. Local checkout fallback will be added.")
}
