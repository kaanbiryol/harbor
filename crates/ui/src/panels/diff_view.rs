use gpui::{
    AnyElement, Context, IntoElement, ListHorizontalSizingBehavior, UniformListScrollHandle, div,
    prelude::*, px, rgb, uniform_list,
};
use harbor_domain::DiffFile;

use crate::diff::{DiffHunk, DiffLine, DiffLineKind, ParsedDiff};
use crate::workspace::AppView;

pub(crate) fn render_diff_panel(
    file: Option<&DiffFile>,
    parsed_diff: Option<&ParsedDiff>,
    is_loading: bool,
    error: Option<&str>,
    scroll_handle: UniformListScrollHandle,
    cx: &mut Context<AppView>,
) -> impl IntoElement {
    if is_loading {
        return div()
            .flex()
            .flex_col()
            .flex_1()
            .min_h_0()
            .gap_2()
            .child(
                div()
                    .text_color(rgb(0xf1f5f9))
                    .child("Unified diff preview"),
            )
            .child(
                div()
                    .rounded_sm()
                    .border_1()
                    .border_color(rgb(0x242a31))
                    .bg(rgb(0x0c0f12))
                    .p_3()
                    .text_color(rgb(0x9aa4b2))
                    .child("Loading diff..."),
            )
            .into_any_element();
    }

    if let Some(error) = error {
        return div()
            .flex()
            .flex_col()
            .flex_1()
            .min_h_0()
            .gap_2()
            .child(
                div()
                    .text_color(rgb(0xf1f5f9))
                    .child("Unified diff preview"),
            )
            .child(
                div()
                    .rounded_sm()
                    .border_1()
                    .border_color(rgb(0x242a31))
                    .bg(rgb(0x0c0f12))
                    .p_3()
                    .text_color(rgb(0xf87171))
                    .child(error.to_string()),
            )
            .into_any_element();
    }

    let Some(file) = file else {
        return div()
            .flex()
            .flex_col()
            .flex_1()
            .min_h_0()
            .gap_2()
            .child(
                div()
                    .text_color(rgb(0xf1f5f9))
                    .child("Unified diff preview"),
            )
            .child(
                div()
                    .rounded_sm()
                    .border_1()
                    .border_color(rgb(0x242a31))
                    .bg(rgb(0x0c0f12))
                    .p_3()
                    .text_color(rgb(0x9aa4b2))
                    .child("Select a changed file to preview its diff"),
            )
            .into_any_element();
    };

    let Some(parsed_diff) = parsed_diff else {
        return div()
            .flex()
            .flex_col()
            .flex_1()
            .min_h_0()
            .gap_2()
            .child(render_diff_file_header(file, None))
            .child(
                div()
                    .rounded_sm()
                    .border_1()
                    .border_color(rgb(0x242a31))
                    .bg(rgb(0x0c0f12))
                    .p_3()
                    .text_color(rgb(0xfbbf24))
                    .child(
                        "Diff unavailable via GitHub API. Local checkout fallback will be added.",
                    ),
            )
            .into_any_element();
    };

    let row_count = diff_row_count(parsed_diff);

    div()
        .id("diff-panel")
        .flex()
        .flex_col()
        .flex_1()
        .min_h_0()
        .min_w_0()
        .gap_2()
        .child(render_diff_file_header(file, Some(parsed_diff.hunks.len())))
        .child(
            div()
                .flex()
                .flex_col()
                .flex_1()
                .min_h_0()
                .min_w_0()
                .rounded_sm()
                .border_1()
                .border_color(rgb(0x242a31))
                .bg(rgb(0x0c0f12))
                .overflow_hidden()
                .child(
                    uniform_list(
                        "diff-lines-list",
                        row_count,
                        cx.processor(|view, range: std::ops::Range<usize>, _window, _cx| {
                            let Some(parsed_diff) = view.active_diff() else {
                                return Vec::new();
                            };
                            let mut rows = Vec::with_capacity(range.len());

                            for row_index in range {
                                if let Some(row) =
                                    render_diff_row(parsed_diff, row_index, view.active_hunk)
                                {
                                    rows.push(row);
                                }
                            }

                            rows
                        }),
                    )
                    .with_horizontal_sizing_behavior(ListHorizontalSizingBehavior::Unconstrained)
                    .track_scroll(&scroll_handle)
                    .flex_1()
                    .min_h_0()
                    .min_w_0()
                    .font_family("Menlo")
                    .text_xs(),
                ),
        )
        .into_any_element()
}

pub(crate) fn render_diff_file_header(
    file: &DiffFile,
    hunk_count: Option<usize>,
) -> impl IntoElement {
    let hunk_label = hunk_count.map_or_else(
        || "no parsed hunks".to_string(),
        |count| format!("{count} hunks"),
    );

    div()
        .flex()
        .items_center()
        .justify_between()
        .gap_3()
        .text_color(rgb(0xf1f5f9))
        .child(file.path.clone())
        .child(div().text_xs().text_color(rgb(0x9aa4b2)).child(format!(
            "{:?}  +{} -{}  {}",
            file.status, file.additions, file.deletions, hunk_label
        )))
}

enum DiffRow<'a> {
    Hunk { hunk: &'a DiffHunk, index: usize },
    Line(&'a DiffLine),
}

pub(crate) fn diff_row_count(diff: &ParsedDiff) -> usize {
    diff.hunks.iter().map(|hunk| hunk.lines.len() + 1).sum()
}

pub(crate) fn diff_hunk_row_index(diff: &ParsedDiff, hunk_index: usize) -> Option<usize> {
    let mut row_index = 0;

    for (index, hunk) in diff.hunks.iter().enumerate() {
        if index == hunk_index {
            return Some(row_index);
        }

        row_index += hunk.lines.len() + 1;
    }

    None
}

fn diff_row_at(diff: &ParsedDiff, row_index: usize) -> Option<DiffRow<'_>> {
    let mut cursor = 0;

    for (index, hunk) in diff.hunks.iter().enumerate() {
        if row_index == cursor {
            return Some(DiffRow::Hunk { hunk, index });
        }

        cursor += 1;
        let line_offset = row_index.checked_sub(cursor)?;
        if line_offset < hunk.lines.len() {
            return Some(DiffRow::Line(&hunk.lines[line_offset]));
        }

        cursor += hunk.lines.len();
    }

    None
}

pub(crate) fn render_diff_row(
    diff: &ParsedDiff,
    row_index: usize,
    active_hunk: usize,
) -> Option<AnyElement> {
    match diff_row_at(diff, row_index)? {
        DiffRow::Hunk { hunk, index } => {
            Some(render_diff_hunk_row(hunk, index, index == active_hunk).into_any_element())
        }
        DiffRow::Line(line) => Some(render_diff_line(line).into_any_element()),
    }
}

pub(crate) fn render_diff_hunk_row(
    hunk: &DiffHunk,
    index: usize,
    active: bool,
) -> impl IntoElement {
    div()
        .h(px(24.))
        .flex()
        .items_center()
        .gap_2()
        .px_2()
        .border_1()
        .border_color(if active { rgb(0x3b82f6) } else { rgb(0x1a2029) })
        .bg(if active { rgb(0x172033) } else { rgb(0x1a2029) })
        .text_color(rgb(0x93c5fd))
        .whitespace_nowrap()
        .child(format!("hunk {}  {}", index + 1, hunk.header))
}

pub(crate) fn render_diff_line(line: &DiffLine) -> impl IntoElement {
    let (prefix, bg, text_color) = match line.kind {
        DiffLineKind::Context => (" ", rgb(0x0c0f12), rgb(0xcbd5e1)),
        DiffLineKind::Added => ("+", rgb(0x10231a), rgb(0xa7f3d0)),
        DiffLineKind::Removed => ("-", rgb(0x291516), rgb(0xfca5a5)),
        DiffLineKind::Metadata => ("\\", rgb(0x111827), rgb(0x9aa4b2)),
    };

    div()
        .h(px(24.))
        .flex()
        .items_start()
        .bg(bg)
        .text_color(text_color)
        .whitespace_nowrap()
        .child(render_line_number(line.old_line))
        .child(render_line_number(line.new_line))
        .child(
            div()
                .w(px(20.))
                .flex_none()
                .text_color(text_color)
                .child(prefix),
        )
        .child(div().flex_none().child(line.text.clone()))
}

pub(crate) fn render_line_number(line: Option<u32>) -> impl IntoElement {
    div()
        .w(px(52.))
        .flex_none()
        .pr_2()
        .text_right()
        .text_color(rgb(0x64748b))
        .child(line.map_or_else(String::new, |line| line.to_string()))
}
