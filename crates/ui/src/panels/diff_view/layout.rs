#![expect(
    clippy::too_many_arguments,
    reason = "diff row layout helpers share explicit review state to keep row math pure and testable"
)]

use std::{collections::HashSet, ops::Range};

use harbor_domain::{DiffFile, ReviewCommentRange, ReviewSide, ReviewThread};

use crate::{
    diff::{DiffLine, DiffLineKind, ParsedDiff},
    diff_reviews::{anchored_review_threads, review_thread_inline_rows, review_threads_for_line},
    workspace::{ReviewComposer, ReviewLineTarget},
};

use super::{
    DIFF_FILE_HEADER_ROWS, LINE_NUMBER_DIGIT_WIDTH, LINE_NUMBER_PADDING, MIN_LINE_NUMBER_WIDTH,
    REVIEW_COMMENT_EDIT_ROWS, REVIEW_COMPOSER_ROWS, REVIEW_COMPOSER_ROWS_WITH_ERROR,
    REVIEW_THREAD_REPLY_ROWS,
};

pub(crate) fn continuous_diff_row_count(
    files: &[DiffFile],
    diffs: &[Option<ParsedDiff>],
    visible_file_indices: &[usize],
    reviewed_file_paths: &HashSet<String>,
    review_threads: &[ReviewThread],
    review_composer: Option<&ReviewComposer>,
    review_comment_error: Option<&str>,
    active_review_thread_reply: Option<&str>,
    active_review_comment_edit: Option<&str>,
) -> usize {
    visible_file_indices
        .iter()
        .filter_map(|file_index| files.get(*file_index).map(|file| (*file_index, file)))
        .map(|(file_index, file)| {
            continuous_diff_section_row_count(
                file_index,
                file,
                diffs,
                reviewed_file_paths,
                review_threads,
                review_composer,
                review_comment_error,
                active_review_thread_reply,
                active_review_comment_edit,
            )
        })
        .sum()
}

pub(crate) fn continuous_diff_file_row_index(
    files: &[DiffFile],
    diffs: &[Option<ParsedDiff>],
    visible_file_indices: &[usize],
    reviewed_file_paths: &HashSet<String>,
    target_file_index: usize,
    review_threads: &[ReviewThread],
    review_composer: Option<&ReviewComposer>,
    review_comment_error: Option<&str>,
    active_review_thread_reply: Option<&str>,
    active_review_comment_edit: Option<&str>,
) -> Option<usize> {
    let mut row_index = 0;

    for file_index in visible_file_indices {
        let file = files.get(*file_index)?;
        if *file_index == target_file_index {
            return Some(row_index);
        }

        row_index += continuous_diff_section_row_count(
            *file_index,
            file,
            diffs,
            reviewed_file_paths,
            review_threads,
            review_composer,
            review_comment_error,
            active_review_thread_reply,
            active_review_comment_edit,
        );
    }

    None
}

pub(crate) fn continuous_diff_hunk_row_index(
    files: &[DiffFile],
    diffs: &[Option<ParsedDiff>],
    visible_file_indices: &[usize],
    reviewed_file_paths: &HashSet<String>,
    target_file_index: usize,
    target_hunk_index: usize,
    review_threads: &[ReviewThread],
    review_composer: Option<&ReviewComposer>,
    review_comment_error: Option<&str>,
    active_review_thread_reply: Option<&str>,
    active_review_comment_edit: Option<&str>,
) -> Option<usize> {
    let mut row_index = 0;

    for file_index in visible_file_indices {
        let file = files.get(*file_index)?;
        let parsed_diff = parsed_diff_for_file(diffs, *file_index);

        if *file_index == target_file_index {
            if file_is_reviewed(file, reviewed_file_paths) {
                return None;
            }

            let parsed_diff = parsed_diff?;
            let local_row_index = diff_hunk_row_index_with_review_controls(
                parsed_diff,
                target_hunk_index,
                file,
                review_threads,
                review_composer,
                review_comment_error,
                active_review_thread_reply,
                active_review_comment_edit,
            )?;

            return Some(row_index + DIFF_FILE_HEADER_ROWS + local_row_index);
        }

        row_index += continuous_diff_section_row_count(
            *file_index,
            file,
            diffs,
            reviewed_file_paths,
            review_threads,
            review_composer,
            review_comment_error,
            active_review_thread_reply,
            active_review_comment_edit,
        );
    }

    None
}

pub(super) fn parsed_diff_for_file(
    diffs: &[Option<ParsedDiff>],
    file_index: usize,
) -> Option<&ParsedDiff> {
    diffs
        .get(file_index)
        .and_then(Option::as_ref)
        .filter(|diff| !diff.is_empty())
}

pub(super) fn file_is_reviewed(file: &DiffFile, reviewed_file_paths: &HashSet<String>) -> bool {
    reviewed_file_paths.contains(&file.path)
}

pub(super) fn continuous_diff_section_body_row_count(
    file_index: usize,
    file: &DiffFile,
    diffs: &[Option<ParsedDiff>],
    reviewed_file_paths: &HashSet<String>,
    review_threads: &[ReviewThread],
    review_composer: Option<&ReviewComposer>,
    review_comment_error: Option<&str>,
    active_review_thread_reply: Option<&str>,
    active_review_comment_edit: Option<&str>,
) -> usize {
    if file_is_reviewed(file, reviewed_file_paths) {
        return 0;
    }

    parsed_diff_for_file(diffs, file_index).map_or(1, |diff| {
        diff_row_count_with_review_controls(
            diff,
            file,
            review_threads,
            review_composer,
            review_comment_error,
            active_review_thread_reply,
            active_review_comment_edit,
        )
    })
}

fn continuous_diff_section_row_count(
    file_index: usize,
    file: &DiffFile,
    diffs: &[Option<ParsedDiff>],
    reviewed_file_paths: &HashSet<String>,
    review_threads: &[ReviewThread],
    review_composer: Option<&ReviewComposer>,
    review_comment_error: Option<&str>,
    active_review_thread_reply: Option<&str>,
    active_review_comment_edit: Option<&str>,
) -> usize {
    DIFF_FILE_HEADER_ROWS
        + continuous_diff_section_body_row_count(
            file_index,
            file,
            diffs,
            reviewed_file_paths,
            review_threads,
            review_composer,
            review_comment_error,
            active_review_thread_reply,
            active_review_comment_edit,
        )
}

#[derive(Debug, PartialEq, Eq)]
pub(super) struct DiffFileSection {
    pub(super) file_index: usize,
    pub(super) header_row_index: usize,
    pub(super) hunk_count: Option<usize>,
    pub(super) reviewed: bool,
}

pub(super) fn continuous_diff_section_for_row(
    files: &[DiffFile],
    diffs: &[Option<ParsedDiff>],
    visible_file_indices: &[usize],
    reviewed_file_paths: &HashSet<String>,
    target_row_index: usize,
    review_threads: &[ReviewThread],
    review_composer: Option<&ReviewComposer>,
    review_comment_error: Option<&str>,
    active_review_thread_reply: Option<&str>,
    active_review_comment_edit: Option<&str>,
) -> Option<DiffFileSection> {
    let mut row_index = 0;

    for file_index in visible_file_indices {
        let file = files.get(*file_index)?;
        let section_row_count = continuous_diff_section_row_count(
            *file_index,
            file,
            diffs,
            reviewed_file_paths,
            review_threads,
            review_composer,
            review_comment_error,
            active_review_thread_reply,
            active_review_comment_edit,
        );

        if target_row_index < row_index + section_row_count {
            return Some(DiffFileSection {
                file_index: *file_index,
                header_row_index: row_index,
                hunk_count: parsed_diff_for_file(diffs, *file_index).map(|diff| diff.hunks.len()),
                reviewed: file_is_reviewed(file, reviewed_file_paths),
            });
        }

        row_index += section_row_count;
    }

    None
}

pub(super) fn row_in_range(row_index: usize, range: &Range<usize>) -> bool {
    row_index >= range.start && row_index < range.end
}

pub(super) fn inline_block_render_anchor(
    block_start_row: usize,
    block_row_count: usize,
    range: &Range<usize>,
) -> Option<(usize, usize)> {
    let block_end_row = block_start_row.saturating_add(block_row_count);
    let render_row = block_start_row.max(range.start);

    (render_row < block_end_row && render_row < range.end)
        .then_some((render_row, render_row - block_start_row))
}

fn diff_row_count_with_review_controls(
    diff: &ParsedDiff,
    file: &DiffFile,
    review_threads: &[ReviewThread],
    review_composer: Option<&ReviewComposer>,
    review_comment_error: Option<&str>,
    active_review_thread_reply: Option<&str>,
    active_review_comment_edit: Option<&str>,
) -> usize {
    let anchored_threads = anchored_review_threads(file, review_threads);
    let mut row_count = diff
        .hunks
        .iter()
        .map(|hunk| hunk.lines.len() + 1)
        .sum::<usize>();

    for hunk in &diff.hunks {
        for line in &hunk.lines {
            row_count += review_threads_for_line(&anchored_threads, line)
                .into_iter()
                .map(|thread| {
                    review_thread_inline_rows_with_controls(
                        thread,
                        active_review_thread_reply,
                        active_review_comment_edit,
                    )
                })
                .sum::<usize>();
        }
    }

    if review_composer
        .is_some_and(|composer| review_comment_range_matches_file(file, &composer.range))
    {
        row_count += review_composer_row_count(review_comment_error);
    }

    row_count
}

fn diff_hunk_row_index_with_review_controls(
    diff: &ParsedDiff,
    target_hunk_index: usize,
    file: &DiffFile,
    review_threads: &[ReviewThread],
    review_composer: Option<&ReviewComposer>,
    review_comment_error: Option<&str>,
    active_review_thread_reply: Option<&str>,
    active_review_comment_edit: Option<&str>,
) -> Option<usize> {
    let anchored_threads = anchored_review_threads(file, review_threads);
    let mut row_index = 0;

    for (hunk_index, hunk) in diff.hunks.iter().enumerate() {
        if hunk_index == target_hunk_index {
            return Some(row_index);
        }

        row_index += 1;
        for (line_index, line) in hunk.lines.iter().enumerate() {
            row_index += 1;

            if review_composer.is_some_and(|composer| {
                review_comment_range_matches_file(file, &composer.range)
                    && composer.anchor.hunk_index == hunk_index
                    && composer.anchor.line_index == line_index
            }) {
                row_index += review_composer_row_count(review_comment_error);
            }

            row_index += review_threads_for_line(&anchored_threads, line)
                .into_iter()
                .map(|thread| {
                    review_thread_inline_rows_with_controls(
                        thread,
                        active_review_thread_reply,
                        active_review_comment_edit,
                    )
                })
                .sum::<usize>();
        }
    }

    None
}

pub(super) fn review_thread_inline_rows_with_controls(
    thread: &ReviewThread,
    active_review_thread_reply: Option<&str>,
    active_review_comment_edit: Option<&str>,
) -> usize {
    review_thread_inline_rows(thread)
        + usize::from(active_review_thread_reply == Some(thread.id.as_str()))
            * REVIEW_THREAD_REPLY_ROWS
        + active_review_comment_edit
            .and_then(|comment_id| {
                thread
                    .comments
                    .iter()
                    .any(|comment| comment.id == comment_id)
                    .then_some(REVIEW_COMMENT_EDIT_ROWS)
            })
            .unwrap_or(0)
}

pub(super) fn review_composer_row_count(error: Option<&str>) -> usize {
    if error.is_some() {
        REVIEW_COMPOSER_ROWS_WITH_ERROR
    } else {
        REVIEW_COMPOSER_ROWS
    }
}

pub(super) fn review_line_target_for_line(
    file: &DiffFile,
    hunk_index: usize,
    line_index: usize,
    line: &DiffLine,
) -> Option<ReviewLineTarget> {
    match line.kind {
        DiffLineKind::Metadata => None,
        DiffLineKind::Removed => {
            let line_number = line.old_line?;
            Some(ReviewLineTarget {
                hunk_index,
                line_index,
                range: ReviewCommentRange {
                    path: file.path.clone(),
                    line: line_number,
                    side: ReviewSide::Left,
                    start_line: None,
                    start_side: None,
                },
            })
        }
        DiffLineKind::Added | DiffLineKind::Context => {
            line.new_line.map(|line_number| ReviewLineTarget {
                hunk_index,
                line_index,
                range: ReviewCommentRange {
                    path: file.path.clone(),
                    line: line_number,
                    side: ReviewSide::Right,
                    start_line: None,
                    start_side: None,
                },
            })
        }
    }
}

pub(super) fn review_comment_range_matches_line(
    file: &DiffFile,
    range: &ReviewCommentRange,
    line: &DiffLine,
) -> bool {
    if !review_comment_range_matches_file(file, range) {
        return false;
    }

    match range.side {
        ReviewSide::Left => line.old_line.is_some_and(|line_number| {
            line_number >= range.start_line.unwrap_or(range.line) && line_number <= range.line
        }),
        ReviewSide::Right => line.new_line.is_some_and(|line_number| {
            line_number >= range.start_line.unwrap_or(range.line) && line_number <= range.line
        }),
    }
}

pub(super) fn review_comment_range_matches_file(
    file: &DiffFile,
    range: &ReviewCommentRange,
) -> bool {
    path_matches_file(file, &range.path)
}

pub(super) fn review_comment_range_label(range: &ReviewCommentRange) -> String {
    let side = match range.side {
        ReviewSide::Left => "left",
        ReviewSide::Right => "right",
    };

    if let Some(start_line) = range.start_line {
        format!("{side} lines {start_line}-{}", range.line)
    } else {
        format!("{side} line {}", range.line)
    }
}

fn path_matches_file(file: &DiffFile, path: &str) -> bool {
    path == file.path || file.previous_path.as_deref() == Some(path)
}

pub(super) fn line_number_width_for_diff(diff: &ParsedDiff) -> f32 {
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
    use harbor_domain::{FileStatus, ReviewComment, ReviewThreadState};

    use crate::{
        diff::parse_unified_diff,
        workspace::{ReviewComposer, review_range_from_targets},
    };

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

    #[test]
    fn selects_right_side_target_for_added_line() {
        let file = test_file("src/lib.rs");
        let diff = parse_unified_diff("@@ -1 +1,2 @@\n context\n+added\n");
        let target = review_line_target_for_line(&file, 0, 1, &diff.hunks[0].lines[1])
            .expect("added line should be commentable");

        assert_eq!(target.range.path, "src/lib.rs");
        assert_eq!(target.range.side, ReviewSide::Right);
        assert_eq!(target.range.line, 2);
        assert_eq!(target.range.start_line, None);
    }

    #[test]
    fn selects_left_side_target_for_removed_line() {
        let file = test_file("src/lib.rs");
        let diff = parse_unified_diff("@@ -10,2 +10 @@\n-removed\n context\n");
        let target = review_line_target_for_line(&file, 0, 0, &diff.hunks[0].lines[0])
            .expect("removed line should be commentable");

        assert_eq!(target.range.path, "src/lib.rs");
        assert_eq!(target.range.side, ReviewSide::Left);
        assert_eq!(target.range.line, 10);
        assert_eq!(target.range.start_line, None);
    }

    #[test]
    fn counts_inline_composer_row() {
        let file = test_file("src/lib.rs");
        let diff = parse_unified_diff("@@ -1 +1,2 @@\n context\n+added\n");
        let target = review_line_target_for_line(&file, 0, 1, &diff.hunks[0].lines[1])
            .expect("added line should be commentable");
        let composer = ReviewComposer {
            anchor: target.clone(),
            range: target.range,
        };

        assert_eq!(
            diff_row_count_with_review_controls(
                &diff,
                &file,
                &[],
                Some(&composer),
                None,
                None,
                None
            ),
            3 + REVIEW_COMPOSER_ROWS
        );
    }

    #[test]
    fn anchors_inline_block_to_first_visible_row() {
        assert_eq!(inline_block_render_anchor(10, 8, &(10..18)), Some((10, 0)));
        assert_eq!(inline_block_render_anchor(10, 8, &(13..18)), Some((13, 3)));
        assert_eq!(inline_block_render_anchor(10, 8, &(18..24)), None);
        assert_eq!(inline_block_render_anchor(10, 8, &(0..10)), None);
    }

    #[test]
    fn expands_review_thread_row_for_active_reply() {
        let thread = test_review_thread("thread-1", "comment-1");

        assert_eq!(
            review_thread_inline_rows_with_controls(&thread, Some("thread-1"), None),
            review_thread_inline_rows(&thread) + REVIEW_THREAD_REPLY_ROWS
        );
    }

    #[test]
    fn counts_continuous_diff_rows_across_visible_files() {
        let files = vec![
            test_file("src/a.rs"),
            test_file("src/generated.bin"),
            test_file("src/b.rs"),
        ];
        let diffs = vec![
            Some(parse_unified_diff("@@ -1 +1,2 @@\n context\n+added\n")),
            None,
            Some(parse_unified_diff("@@ -10 +10 @@\n later\n")),
        ];
        let visible_file_indices = vec![0, 1, 2];
        let reviewed_file_paths = HashSet::new();

        assert_eq!(
            continuous_diff_row_count(
                &files,
                &diffs,
                &visible_file_indices,
                &reviewed_file_paths,
                &[],
                None,
                None,
                None,
                None
            ),
            12
        );
    }

    #[test]
    fn finds_continuous_file_and_hunk_rows_across_missing_patches() {
        let files = vec![
            test_file("src/a.rs"),
            test_file("src/generated.bin"),
            test_file("src/b.rs"),
        ];
        let diffs = vec![
            Some(parse_unified_diff("@@ -1 +1,2 @@\n context\n+added\n")),
            None,
            Some(parse_unified_diff("@@ -10 +10 @@\n later\n")),
        ];
        let visible_file_indices = vec![0, 1, 2];
        let reviewed_file_paths = HashSet::new();

        assert_eq!(
            continuous_diff_file_row_index(
                &files,
                &diffs,
                &visible_file_indices,
                &reviewed_file_paths,
                2,
                &[],
                None,
                None,
                None,
                None
            ),
            Some(8)
        );
        assert_eq!(
            continuous_diff_hunk_row_index(
                &files,
                &diffs,
                &visible_file_indices,
                &reviewed_file_paths,
                2,
                0,
                &[],
                None,
                None,
                None,
                None
            ),
            Some(10)
        );
        assert_eq!(
            continuous_diff_hunk_row_index(
                &files,
                &diffs,
                &visible_file_indices,
                &reviewed_file_paths,
                1,
                0,
                &[],
                None,
                None,
                None,
                None
            ),
            None
        );
    }

    #[test]
    fn collapses_reviewed_file_sections_in_continuous_diff_rows() {
        let files = vec![
            test_file("src/a.rs"),
            test_file("src/generated.bin"),
            test_file("src/b.rs"),
        ];
        let diffs = vec![
            Some(parse_unified_diff("@@ -1 +1,2 @@\n context\n+added\n")),
            None,
            Some(parse_unified_diff("@@ -10 +10 @@\n later\n")),
        ];
        let visible_file_indices = vec![0, 1, 2];
        let reviewed_file_paths = HashSet::from(["src/a.rs".to_string()]);

        assert_eq!(
            continuous_diff_row_count(
                &files,
                &diffs,
                &visible_file_indices,
                &reviewed_file_paths,
                &[],
                None,
                None,
                None,
                None
            ),
            9
        );
        assert_eq!(
            continuous_diff_file_row_index(
                &files,
                &diffs,
                &visible_file_indices,
                &reviewed_file_paths,
                2,
                &[],
                None,
                None,
                None,
                None
            ),
            Some(5)
        );
        assert_eq!(
            continuous_diff_hunk_row_index(
                &files,
                &diffs,
                &visible_file_indices,
                &reviewed_file_paths,
                0,
                0,
                &[],
                None,
                None,
                None,
                None
            ),
            None
        );
        assert_eq!(
            continuous_diff_hunk_row_index(
                &files,
                &diffs,
                &visible_file_indices,
                &reviewed_file_paths,
                2,
                0,
                &[],
                None,
                None,
                None,
                None
            ),
            Some(7)
        );
    }

    #[test]
    fn finds_continuous_diff_section_for_row_across_file_boundaries() {
        let files = vec![
            test_file("src/a.rs"),
            test_file("src/generated.bin"),
            test_file("src/b.rs"),
        ];
        let diffs = vec![
            Some(parse_unified_diff("@@ -1 +1,2 @@\n context\n+added\n")),
            None,
            Some(parse_unified_diff("@@ -10 +10 @@\n later\n")),
        ];
        let visible_file_indices = vec![0, 1, 2];
        let reviewed_file_paths = HashSet::new();

        assert_eq!(
            continuous_diff_section_for_row(
                &files,
                &diffs,
                &visible_file_indices,
                &reviewed_file_paths,
                5,
                &[],
                None,
                None,
                None,
                None
            ),
            Some(DiffFileSection {
                file_index: 1,
                header_row_index: 5,
                hunk_count: None,
                reviewed: false,
            })
        );
        assert_eq!(
            continuous_diff_section_for_row(
                &files,
                &diffs,
                &visible_file_indices,
                &reviewed_file_paths,
                6,
                &[],
                None,
                None,
                None,
                None
            ),
            Some(DiffFileSection {
                file_index: 1,
                header_row_index: 5,
                hunk_count: None,
                reviewed: false,
            })
        );
        assert_eq!(
            continuous_diff_section_for_row(
                &files,
                &diffs,
                &visible_file_indices,
                &reviewed_file_paths,
                9,
                &[],
                None,
                None,
                None,
                None
            ),
            Some(DiffFileSection {
                file_index: 2,
                header_row_index: 8,
                hunk_count: Some(1),
                reviewed: false,
            })
        );
    }

    #[test]
    fn scopes_inline_composer_rows_to_matching_file() {
        let file = test_file("src/a.rs");
        let other_file = test_file("src/b.rs");
        let diff = parse_unified_diff("@@ -1 +1 @@\n context\n@@ -10 +10 @@\n later\n");
        let target = review_line_target_for_line(&file, 0, 0, &diff.hunks[0].lines[0])
            .expect("context line should be commentable");
        let composer = ReviewComposer {
            anchor: target.clone(),
            range: target.range,
        };

        assert_eq!(
            diff_hunk_row_index_with_review_controls(
                &diff,
                1,
                &file,
                &[],
                Some(&composer),
                None,
                None,
                None
            ),
            Some(2 + REVIEW_COMPOSER_ROWS)
        );
        assert_eq!(
            diff_hunk_row_index_with_review_controls(
                &diff,
                1,
                &other_file,
                &[],
                Some(&composer),
                None,
                None,
                None
            ),
            Some(2)
        );
    }

    #[test]
    fn builds_multiline_right_side_review_range() {
        let file = test_file("src/lib.rs");
        let diff = parse_unified_diff("@@ -1 +1,3 @@\n context\n+added\n+again\n");
        let start = review_line_target_for_line(&file, 0, 1, &diff.hunks[0].lines[1])
            .expect("added line should be commentable");
        let end = review_line_target_for_line(&file, 0, 2, &diff.hunks[0].lines[2])
            .expect("added line should be commentable");

        let range = review_range_from_targets(&start, &end).unwrap();

        assert_eq!(range.path, "src/lib.rs");
        assert_eq!(range.side, ReviewSide::Right);
        assert_eq!(range.start_line, Some(2));
        assert_eq!(range.start_side, Some(ReviewSide::Right));
        assert_eq!(range.line, 3);
    }

    #[test]
    fn rejects_mixed_side_review_range() {
        let file = test_file("src/lib.rs");
        let diff = parse_unified_diff("@@ -1 +1 @@\n-old\n+new\n");
        let left = review_line_target_for_line(&file, 0, 0, &diff.hunks[0].lines[0])
            .expect("removed line should be commentable");
        let right = review_line_target_for_line(&file, 0, 1, &diff.hunks[0].lines[1])
            .expect("added line should be commentable");

        let error =
            review_range_from_targets(&left, &right).expect_err("mixed side selection should fail");

        assert_eq!(error, "Review comments can only span one diff side");
    }

    fn test_file(path: &str) -> DiffFile {
        DiffFile {
            path: path.to_string(),
            previous_path: None,
            status: FileStatus::Modified,
            additions: 1,
            deletions: 1,
            changes: 2,
            patch: None,
        }
    }

    fn test_review_thread(thread_id: &str, comment_id: &str) -> ReviewThread {
        ReviewThread {
            id: thread_id.to_string(),
            path: "src/lib.rs".to_string(),
            range: None,
            state: ReviewThreadState::Unresolved,
            comments: vec![ReviewComment {
                id: comment_id.to_string(),
                author: "maria".to_string(),
                author_avatar_url: None,
                body: "Please check this line.".to_string(),
                created_at: chrono::DateTime::parse_from_rfc3339("2026-05-01T10:00:00Z")
                    .expect("valid test timestamp")
                    .with_timezone(&chrono::Utc),
                updated_at: None,
                position: None,
                viewer_did_author: false,
                viewer_can_update: false,
                viewer_can_delete: false,
                viewer_can_react: true,
                reactions: Vec::new(),
            }],
        }
    }
}
