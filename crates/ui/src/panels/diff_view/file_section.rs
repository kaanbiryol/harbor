use gpui::{AnyElement, Entity, IntoElement, div, prelude::*, px, rgb};
use gpui_component::{
    IconName, Sizable, StyledExt,
    button::{Button, ButtonVariants},
};
use harbor_domain::DiffFile;

use crate::workspace::AppView;

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
    let hunk_label = hunk_count.map_or_else(
        || "no parsed hunks".to_string(),
        |count| {
            if count == 1 {
                "1 hunk".to_string()
            } else {
                format!("{count} hunks")
            }
        },
    );
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
        .border_color(if active {
            rgb(0x3b82f6)
        } else if sticky {
            rgb(0x334155)
        } else {
            rgb(0x2f3a4a)
        })
        .bg(if active {
            rgb(0x18243b)
        } else if reviewed {
            rgb(0x111820)
        } else {
            rgb(0x141c2a)
        })
        .font_family(".SystemUIFont")
        .text_color(rgb(0xf1f5f9))
        .whitespace_nowrap()
        .cursor_pointer()
        .when(sticky, |element| element.shadow_lg())
        .hover(|element| element.bg(rgb(0x172033)))
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
                        .text_size(px(16.0))
                        .font_medium()
                        .text_color(if reviewed {
                            rgb(0xa7b0bd)
                        } else {
                            rgb(0xf1f5f9)
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
                .text_color(rgb(0xa7b0bd))
                .child(
                    div()
                        .text_color(rgb(0xcbd5e1))
                        .child(format!("{:?}", file.status)),
                )
                .child(
                    div()
                        .text_color(rgb(0x34d399))
                        .child(format!("+{}", file.additions)),
                )
                .child(
                    div()
                        .text_color(rgb(0xf87171))
                        .child(format!("-{}", file.deletions)),
                )
                .child(div().text_color(rgb(0x9aa4b2)).child(hunk_label))
                .when(reviewed, |element| {
                    element.child(
                        div()
                            .rounded_xs()
                            .border_1()
                            .border_color(rgb(0x2f4f3e))
                            .bg(rgb(0x12241b))
                            .px_1()
                            .text_xs()
                            .text_color(rgb(0x86efac))
                            .child("reviewed"),
                    )
                }),
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
        .bg(rgb(0x0c0f12))
        .font_family(".SystemUIFont")
        .text_color(rgb(0xfbbf24))
        .whitespace_nowrap()
        .child("Diff unavailable via GitHub API. Local checkout fallback will be added.")
}
