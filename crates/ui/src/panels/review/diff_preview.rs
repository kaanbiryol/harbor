use gpui::{IntoElement, SharedString, div, prelude::*};
use harbor_domain::{DiffFile, ReviewComment, ReviewSide, ReviewThread};

use crate::{
    diff::{DiffLine, DiffLineKind, ParsedDiff},
    panels::review_markdown::{
        ReviewSuggestionContext, ReviewSuggestionOriginalLine, review_markdown_has_suggestion,
    },
    visual::{Tone, color, tone_colors},
};

const LEADING_CONTEXT_LINES: usize = 2;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ReviewDiffPreview {
    pub(super) lines: Vec<ReviewDiffPreviewLine>,
    pub(super) suggestion_context: Option<ReviewSuggestionContext>,
}

impl ReviewDiffPreview {
    pub(crate) fn suggestion_context(&self) -> Option<&ReviewSuggestionContext> {
        self.suggestion_context.as_ref()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct ReviewDiffPreviewLine {
    pub(super) line: Option<u32>,
    pub(super) marker: &'static str,
    pub(super) text: String,
    pub(super) tone: Tone,
    pub(super) is_leading_context: bool,
}

pub(crate) fn render_review_diff_preview(
    preview: ReviewDiffPreview,
    mono_font_family: SharedString,
) -> impl IntoElement {
    div().min_w_0().overflow_hidden().py_1().children(
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
    let colors = tone_colors(line.tone);
    // A short excerpt reads better with a marker bar than a fully saturated row fill.
    let background = gpui::Rgba {
        a: 0.45,
        ..colors.background
    };
    let marker_color = if line.tone == Tone::Neutral {
        background
    } else {
        colors.text
    };

    div()
        .flex()
        .items_center()
        .gap_2()
        .px_2()
        .py_1()
        .text_xs()
        .bg(background)
        .border_l_2()
        .border_color(marker_color)
        .text_color(if line.is_leading_context {
            color::text_muted()
        } else {
            color::text_primary()
        })
        .font_family(mono_font_family)
        .child(
            div()
                .w_8()
                .text_right()
                .text_color(color::text_muted())
                .child(line_label),
        )
        .child(
            div()
                .w_3()
                .text_color(color::text_muted())
                .child(line.marker),
        )
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
            is_leading_context: false,
        }],
        suggestion_context: None,
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
        .enumerate()
        .flat_map(|(hunk_index, hunk)| hunk.lines.iter().map(move |line| (hunk_index, line)))
        .collect::<Vec<_>>();
    let Some(start_index) = diff_lines
        .iter()
        .position(|(_, line)| diff_line_matches_target(line, target.start_side, target.start_line))
    else {
        return Some(fallback());
    };
    let Some(end_index) = diff_lines
        .iter()
        .position(|(_, line)| diff_line_matches_target(line, target.end_side, target.end_line))
    else {
        return Some(fallback());
    };
    let range = if start_index <= end_index {
        start_index..=end_index
    } else {
        end_index..=start_index
    };
    let first_index = *range.start();
    let first_hunk = diff_lines[first_index].0;
    let leading_context = diff_lines
        [first_index.saturating_sub(LEADING_CONTEXT_LINES)..first_index]
        .iter()
        .filter(|(hunk_index, line)| {
            *hunk_index == first_hunk && line.kind != DiffLineKind::Metadata
        })
        .map(|(_, line)| ReviewDiffPreviewLine {
            is_leading_context: true,
            ..review_diff_preview_line(line)
        });
    let selected_diff_lines = diff_lines[range]
        .iter()
        .map(|(_, line)| *line)
        .collect::<Vec<_>>();
    let lines = leading_context
        .chain(
            selected_diff_lines
                .iter()
                .map(|line| review_diff_preview_line(line)),
        )
        .collect::<Vec<_>>();
    if lines.is_empty() {
        return Some(fallback());
    }
    let suggestion_context = if review_thread_has_suggestion(thread) {
        review_suggestion_context(&target, selected_diff_lines.iter().copied())
    } else {
        None
    };

    Some(ReviewDiffPreview {
        lines,
        suggestion_context,
    })
}

pub(crate) fn review_thread_suggestion_context(
    thread: &ReviewThread,
    file: &DiffFile,
    diff: &ParsedDiff,
) -> Option<ReviewSuggestionContext> {
    if !review_thread_has_suggestion(thread) {
        return None;
    }

    let comment = thread.comments.first()?;
    let target = review_comment_diff_target(comment, thread)?;
    if file.path != target.path && file.previous_path.as_deref() != Some(target.path.as_str()) {
        return None;
    }

    let lines = diff.hunks.iter().flat_map(|hunk| hunk.lines.iter());
    review_suggestion_context(&target, lines)
}

fn review_thread_has_suggestion(thread: &ReviewThread) -> bool {
    thread
        .comments
        .iter()
        .any(|comment| review_markdown_has_suggestion(&comment.body))
}

fn review_suggestion_context<'a>(
    target: &ReviewDiffTarget,
    lines: impl IntoIterator<Item = &'a DiffLine>,
) -> Option<ReviewSuggestionContext> {
    if target.start_side != target.end_side {
        return None;
    }

    let first_line = target.start_line.min(target.end_line);
    let last_line = target.start_line.max(target.end_line);
    let original_lines = lines
        .into_iter()
        .filter_map(|line| {
            let line_number = match target.end_side {
                ReviewSide::Left => line.old_line,
                ReviewSide::Right => line.new_line,
            }?;
            (first_line..=last_line)
                .contains(&line_number)
                .then(|| ReviewSuggestionOriginalLine {
                    line_number,
                    text: line.text.clone(),
                })
        })
        .collect::<Vec<_>>();

    (!original_lines.is_empty()).then_some(ReviewSuggestionContext {
        original_lines,
        replacement_start_line: first_line,
    })
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

/// Short location label such as `L21` or `L10-12` for a thread header.
pub(crate) fn review_thread_line_label(thread: &ReviewThread) -> Option<String> {
    let comment = thread.comments.first()?;
    let target = review_comment_diff_target(comment, thread)?;
    let first_line = target.start_line.min(target.end_line);
    let last_line = target.start_line.max(target.end_line);
    Some(if first_line == last_line {
        format!("L{last_line}")
    } else {
        format!("L{first_line}-{last_line}")
    })
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
        is_leading_context: false,
    }
}
