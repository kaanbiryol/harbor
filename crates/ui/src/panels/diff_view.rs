use gpui::{
    AnyElement, Context, IntoElement, ListHorizontalSizingBehavior, UniformListScrollHandle, div,
    prelude::*, px, rgb, uniform_list,
};
use harbor_domain::{DiffFile, ReviewThread, ReviewThreadState};

use crate::diff::{DiffHunk, DiffLine, DiffLineKind, ParsedDiff};
use crate::diff_reviews::{
    anchored_review_threads, diff_row_count_with_reviews, review_threads_for_line,
};
use crate::workspace::AppView;

use super::review::{review_thread_state_label, single_line};

const MIN_LINE_NUMBER_WIDTH: f32 = 28.0;
const LINE_NUMBER_PADDING: f32 = 8.0;
const LINE_NUMBER_DIGIT_WIDTH: f32 = 8.0;
const REVIEW_MARKER_WIDTH: f32 = 24.0;
const PREFIX_WIDTH: f32 = 16.0;

pub(crate) fn render_diff_panel(
    file: Option<&DiffFile>,
    parsed_diff: Option<&ParsedDiff>,
    review_threads: &[ReviewThread],
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

    let row_count = diff_row_count_with_reviews(parsed_diff, file, review_threads);

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
                            let Some(file) = view.active_file() else {
                                return Vec::new();
                            };
                            let Some(parsed_diff) = view.active_diff() else {
                                return Vec::new();
                            };
                            let line_number_width = line_number_width_for_diff(parsed_diff);

                            render_diff_rows(
                                parsed_diff,
                                file,
                                &view.review_threads,
                                view.active_hunk,
                                line_number_width,
                                range,
                            )
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

fn render_diff_rows(
    diff: &ParsedDiff,
    file: &DiffFile,
    review_threads: &[ReviewThread],
    active_hunk: usize,
    line_number_width: f32,
    range: std::ops::Range<usize>,
) -> Vec<AnyElement> {
    let anchored_threads = anchored_review_threads(file, review_threads);
    let review_marker_width = if anchored_threads.is_empty() {
        0.0
    } else {
        REVIEW_MARKER_WIDTH
    };
    let mut rows = Vec::with_capacity(range.len());
    let mut row_index = 0;

    for (hunk_index, hunk) in diff.hunks.iter().enumerate() {
        if row_index >= range.end {
            break;
        }

        if row_in_range(row_index, &range) {
            rows.push(
                render_diff_hunk_row(hunk, hunk_index, hunk_index == active_hunk)
                    .into_any_element(),
            );
        }
        row_index += 1;

        for line in &hunk.lines {
            if row_index >= range.end {
                break;
            }

            let matching_threads = review_threads_for_line(&anchored_threads, line);
            let has_unresolved_thread = matching_threads
                .iter()
                .any(|thread| thread.state == ReviewThreadState::Unresolved);

            if row_in_range(row_index, &range) {
                rows.push(
                    render_diff_line(
                        line,
                        matching_threads.len(),
                        has_unresolved_thread,
                        line_number_width,
                        review_marker_width,
                    )
                    .into_any_element(),
                );
            }
            row_index += 1;

            for thread in matching_threads {
                if row_index >= range.end {
                    break;
                }

                if row_in_range(row_index, &range) {
                    rows.push(
                        render_review_thread_inline(thread, line_number_width).into_any_element(),
                    );
                }
                row_index += 1;
            }
        }
    }

    rows
}

fn row_in_range(row_index: usize, range: &std::ops::Range<usize>) -> bool {
    row_index >= range.start && row_index < range.end
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

pub(crate) fn render_diff_line(
    line: &DiffLine,
    thread_count: usize,
    has_unresolved_thread: bool,
    line_number_width: f32,
    review_marker_width: f32,
) -> impl IntoElement {
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
        .child(render_line_number(line.old_line, line_number_width))
        .child(render_line_number(line.new_line, line_number_width))
        .child(render_review_marker(
            thread_count,
            has_unresolved_thread,
            review_marker_width,
        ))
        .child(
            div()
                .w(px(PREFIX_WIDTH))
                .flex_none()
                .text_color(text_color)
                .child(prefix),
        )
        .child(div().flex_none().child(line.text.clone()))
}

fn render_review_thread_inline(thread: &ReviewThread, line_number_width: f32) -> impl IntoElement {
    let (label, color) = review_thread_state_label(thread.state);
    let latest_comment = thread.comments.last();
    let preview = latest_comment
        .map(|comment| single_line(&comment.body))
        .unwrap_or_else(|| "No comments in this thread".to_string());
    let author = latest_comment
        .map(|comment| comment.author.as_str())
        .unwrap_or("review");

    div()
        .h(px(24.))
        .flex()
        .items_center()
        .bg(rgb(0x171b20))
        .text_color(rgb(0xcbd5e1))
        .whitespace_nowrap()
        .child(render_line_number(None, line_number_width))
        .child(render_line_number(None, line_number_width))
        .child(render_review_marker(
            1,
            thread.state == ReviewThreadState::Unresolved,
            REVIEW_MARKER_WIDTH,
        ))
        .child(
            div()
                .min_w_0()
                .flex_1()
                .flex()
                .items_center()
                .gap_2()
                .child(div().flex_none().text_color(color).child(label))
                .child(
                    div()
                        .flex_none()
                        .text_color(rgb(0x64748b))
                        .child(review_comment_count_label(thread.comments.len())),
                )
                .child(
                    div()
                        .min_w_0()
                        .flex_1()
                        .truncate()
                        .text_color(rgb(0xcbd5e1))
                        .child(format!("{author}: {preview}")),
                ),
        )
}

fn render_review_marker(
    thread_count: usize,
    has_unresolved_thread: bool,
    width: f32,
) -> impl IntoElement {
    let marker = match thread_count {
        0 => String::new(),
        1 => "R".to_string(),
        count => format!("R{count}"),
    };
    let color = if has_unresolved_thread {
        rgb(0xfbbf24)
    } else {
        rgb(0x64748b)
    };

    div()
        .w(px(width))
        .flex_none()
        .text_center()
        .text_color(color)
        .child(marker)
}

fn review_comment_count_label(comment_count: usize) -> String {
    if comment_count == 1 {
        "1 comment".to_string()
    } else {
        format!("{comment_count} comments")
    }
}

fn render_line_number(line: Option<u32>, width: f32) -> impl IntoElement {
    div()
        .w(px(width))
        .flex_none()
        .pr_2()
        .text_right()
        .text_color(rgb(0x64748b))
        .child(line.map_or_else(String::new, |line| line.to_string()))
}

fn line_number_width_for_diff(diff: &ParsedDiff) -> f32 {
    let max_line = diff
        .hunks
        .iter()
        .flat_map(|hunk| hunk.lines.iter())
        .flat_map(|line| [line.old_line, line.new_line])
        .flatten()
        .max()
        .unwrap_or(1);
    let digits = max_line.to_string().len() as f32;

    (digits * LINE_NUMBER_DIGIT_WIDTH + LINE_NUMBER_PADDING).max(MIN_LINE_NUMBER_WIDTH)
}

#[cfg(test)]
mod tests {
    use crate::diff::parse_unified_diff;

    use super::*;

    #[test]
    fn keeps_small_diff_gutters_compact() {
        let diff = parse_unified_diff("@@ -8,2 +8,2 @@\n one\n two\n");

        assert_eq!(line_number_width_for_diff(&diff), MIN_LINE_NUMBER_WIDTH);
    }

    #[test]
    fn expands_gutter_for_large_line_numbers() {
        let diff = parse_unified_diff("@@ -99999,2 +100000,2 @@\n context\n-removed\n+added\n");

        assert_eq!(
            line_number_width_for_diff(&diff),
            6.0 * LINE_NUMBER_DIGIT_WIDTH + LINE_NUMBER_PADDING
        );
    }
}
