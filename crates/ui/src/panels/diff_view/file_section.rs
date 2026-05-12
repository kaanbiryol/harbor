use gpui::{AnyElement, Entity, IntoElement, div, prelude::*, px};
use gpui_component::{
    IconName, Sizable, StyledExt,
    button::{Button, ButtonVariants},
};
use harbor_domain::{DiffFile, FileStatus};

use crate::{
    visual::{Tone, color, font},
    workspace::AppView,
};

use super::super::render_status_pill;
use super::{DIFF_FILE_HEADER_HEIGHT, DIFF_ROW_HEIGHT};

pub(super) fn render_diff_file_section_header(
    file_index: usize,
    file: DiffFile,
    hunk_count: Option<usize>,
    active: bool,
    reviewed: bool,
    sticky: bool,
    view_entity: Entity<AppView>,
) -> AnyElement {
    let hunk_label = hunk_count.map_or_else(|| "no parsed hunks".to_string(), hunk_count_label);
    let header_id = if sticky {
        format!("sticky-diff-file-header-{file_index}")
    } else {
        format!("diff-file-header-{file_index}")
    };
    let review_button = Button::new(format!(
        "{}-diff-file-reviewed-{file_index}",
        if sticky { "sticky" } else { "row" }
    ))
    .icon(if reviewed {
        IconName::Check
    } else {
        IconName::Eye
    })
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
    let select_view_entity = view_entity.clone();

    div()
        .id(header_id)
        .h(px(DIFF_FILE_HEADER_HEIGHT))
        .w_full()
        .min_w_0()
        .flex()
        .items_center()
        .justify_between()
        .gap_4()
        .px_3()
        .border_1()
        .border_color(if active || sticky {
            color::border_strong()
        } else {
            color::border()
        })
        .bg(if active {
            color::row_selected_subtle()
        } else if reviewed {
            color::content_background()
        } else {
            color::panel_background()
        })
        .font_family(font::UI)
        .text_color(color::text_primary())
        .whitespace_nowrap()
        .cursor_pointer()
        .when(sticky, |element| element.shadow_lg())
        .hover(|element| element.bg(color::row_hover()))
        .on_click(move |_, _, cx| {
            select_view_entity.update(cx, |view, cx| {
                view.select_file(file_index, cx);
            });
        })
        .child(
            div()
                .min_w_0()
                .flex_1()
                .flex()
                .items_center()
                .gap_3()
                .child(review_button)
                .child(
                    div()
                        .min_w_0()
                        .truncate()
                        .text_size(px(15.0))
                        .font_medium()
                        .text_color(if reviewed {
                            color::text_muted()
                        } else {
                            color::text_primary()
                        })
                        .child(path),
                ),
        )
        .child(
            div()
                .flex_none()
                .flex()
                .items_center()
                .gap_3()
                .text_xs()
                .font_medium()
                .text_color(color::text_secondary())
                .child(render_file_status_pill(file.status))
                .child(render_status_pill(
                    format!("+{}", file.additions),
                    Tone::Success,
                ))
                .child(render_status_pill(
                    format!("-{}", file.deletions),
                    Tone::Danger,
                ))
                .child(render_status_pill(hunk_label, Tone::Neutral))
                .when(reviewed, |element| {
                    element.child(render_status_pill("reviewed", Tone::Success))
                }),
        )
        .into_any_element()
}

fn render_file_status_pill(status: FileStatus) -> impl IntoElement {
    let (label, tone) = match status {
        FileStatus::Added => ("added", Tone::Success),
        FileStatus::Modified => ("modified", Tone::Info),
        FileStatus::Removed => ("removed", Tone::Danger),
        FileStatus::Renamed => ("renamed", Tone::Warning),
        FileStatus::Copied => ("copied", Tone::Neutral),
        FileStatus::Changed => ("changed", Tone::Info),
        FileStatus::Unchanged => ("unchanged", Tone::Neutral),
    };

    render_status_pill(label, tone)
}

fn hunk_count_label(count: usize) -> String {
    if count == 1 {
        "1 hunk".to_string()
    } else {
        format!("{count} hunks")
    }
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
