use gpui::{IntoElement, SharedString, div, prelude::*};
use harbor_domain::{DiffFile, ReviewComment, ReviewSide, ReviewThread};

use crate::{
    diff::{DiffLine, DiffLineKind, ParsedDiff},
    visual::{Tone, color, tone_colors},
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ReviewDiffPreview {
    pub(super) lines: Vec<ReviewDiffPreviewLine>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct ReviewDiffPreviewLine {
    pub(super) line: Option<u32>,
    pub(super) marker: &'static str,
    pub(super) text: String,
    pub(super) tone: Tone,
}

pub(crate) fn render_review_diff_preview(
    preview: ReviewDiffPreview,
    mono_font_family: SharedString,
) -> impl IntoElement {
    div()
        .min_w_0()
        .overflow_hidden()
        .rounded_xs()
        .border_1()
        .border_color(color::border_subtle())
        .children(
            preview
                .lines
                .into_iter()
                .map(move |line| render_review_diff_preview_line(line, mono_font_family.clone())),
        )
}

fn render_review_diff_preview_line(
    line: ReviewDiffPreviewLine,
    mono_font_family: SharedString,
) -> impl IntoElement {
    let line_label = line
        .line
        .map(|line| line.to_string())
        .unwrap_or_else(|| "-".to_string());

    div()
        .flex()
        .items_center()
        .gap_2()
        .px_2()
        .py_1()
        .text_xs()
        .bg(tone_colors(line.tone).background)
        .text_color(color::text_primary())
        .font_family(mono_font_family)
        .child(div().w_8().text_right().child(line_label))
        .child(div().w_3().child(line.marker))
        .child(div().min_w_0().flex_1().truncate().child(line.text))
}

pub(crate) fn review_thread_diff_preview(
    thread: &ReviewThread,
    files: &[DiffFile],
    diffs: &[Option<ParsedDiff>],
) -> Option<ReviewDiffPreview> {
    let comment = thread.comments.first()?;

    review_comment_diff_preview(comment, thread, files, diffs)
}

fn review_comment_diff_preview(
    comment: &ReviewComment,
    thread: &ReviewThread,
    files: &[DiffFile],
    diffs: &[Option<ParsedDiff>],
) -> Option<ReviewDiffPreview> {
    let target = review_comment_diff_target(comment, thread)?;
    let fallback = || ReviewDiffPreview {
        lines: vec![ReviewDiffPreviewLine {
            line: Some(target.end_line),
            marker: "",
            text: "diff context unavailable".to_string(),
            tone: Tone::Neutral,
        }],
    };
    let Some((_, diff)) = files.iter().zip(diffs.iter()).find(|(file, _)| {
        file.path == target.path || file.previous_path.as_deref() == Some(target.path.as_str())
    }) else {
        return Some(fallback());
    };
    let Some(diff) = diff.as_ref() else {
        return Some(fallback());
    };
    let diff_lines = diff
        .hunks
        .iter()
        .flat_map(|hunk| hunk.lines.iter())
        .collect::<Vec<_>>();
    let Some(start_index) = diff_lines
        .iter()
        .position(|line| diff_line_matches_target(line, target.start_side, target.start_line))
    else {
        return Some(fallback());
    };
    let Some(end_index) = diff_lines
        .iter()
        .position(|line| diff_line_matches_target(line, target.end_side, target.end_line))
    else {
        return Some(fallback());
    };
    let range = if start_index <= end_index {
        start_index..=end_index
    } else {
        end_index..=start_index
    };
    let lines = diff_lines[range]
        .iter()
        .map(|line| review_diff_preview_line(line))
        .collect::<Vec<_>>();
    if lines.is_empty() {
        return Some(fallback());
    }

    Some(ReviewDiffPreview { lines })
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ReviewDiffTarget {
    path: String,
    start_side: ReviewSide,
    start_line: u32,
    end_side: ReviewSide,
    end_line: u32,
}

fn review_comment_diff_target(
    comment: &ReviewComment,
    thread: &ReviewThread,
) -> Option<ReviewDiffTarget> {
    if let Some(range) = thread.range.as_ref() {
        return Some(ReviewDiffTarget {
            path: range.path.clone(),
            start_side: range.start_side.unwrap_or(range.side),
            start_line: range.start_line.unwrap_or(range.line),
            end_side: range.side,
            end_line: range.line,
        });
    }

    if let Some(position) = comment.position.as_ref() {
        let line = match position.side {
            ReviewSide::Left => position.original_line.or(position.line),
            ReviewSide::Right => position.line.or(position.original_line),
        }?;
        return Some(ReviewDiffTarget {
            path: position.path.clone(),
            start_side: position.side,
            start_line: line,
            end_side: position.side,
            end_line: line,
        });
    }

    None
}

fn diff_line_matches_target(line: &DiffLine, side: ReviewSide, target_line: u32) -> bool {
    match side {
        ReviewSide::Left => line.old_line == Some(target_line),
        ReviewSide::Right => line.new_line == Some(target_line),
    }
}

fn review_diff_preview_line(line: &DiffLine) -> ReviewDiffPreviewLine {
    let (marker, tone) = match line.kind {
        DiffLineKind::Added => ("+", Tone::Success),
        DiffLineKind::Removed => ("-", Tone::Danger),
        DiffLineKind::Context => (" ", Tone::Neutral),
        DiffLineKind::Metadata => ("", Tone::Neutral),
    };

    ReviewDiffPreviewLine {
        line: line.new_line.or(line.old_line),
        marker,
        text: line.text.clone(),
        tone,
    }
}
