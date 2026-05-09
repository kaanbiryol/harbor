#![expect(
    clippy::too_many_arguments,
    reason = "diff row layout helpers share explicit review state to keep row math pure and testable"
)]

use std::{collections::HashSet, ops::Range};

use harbor_domain::{DiffFile, ReviewThread};

use crate::{
    diff::ParsedDiff,
    diff_reviews::{anchored_review_threads, review_threads_for_line},
    workspace::ReviewComposer,
};

use super::{
    DIFF_FILE_HEADER_ROWS, LINE_NUMBER_DIGIT_WIDTH, LINE_NUMBER_PADDING, MIN_LINE_NUMBER_WIDTH,
    inline_review_layout::{
        review_comment_range_matches_file, review_composer_row_count,
        review_thread_inline_rows_with_controls,
    },
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
    use std::time::{Duration, Instant};

    use harbor_domain::FileStatus;

    use crate::{
        diff::{DiffHunk, DiffLine, DiffLineKind, ParsedDiff, parse_unified_diff},
        panels::diff_view::{
            REVIEW_COMPOSER_ROWS, inline_review_layout::review_line_target_for_line,
        },
        workspace::ReviewComposer,
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
    fn calculates_large_diff_rows_with_linear_cost() {
        let file_count = 200;
        let hunk_count = 4;
        let lines_per_hunk = 25;
        let files = (0..file_count)
            .map(|index| test_file(&format!("src/file_{index}.rs")))
            .collect::<Vec<_>>();
        let diffs = (0..file_count)
            .map(|_| Some(large_test_diff(hunk_count, lines_per_hunk)))
            .collect::<Vec<_>>();
        let visible_file_indices = (0..file_count).collect::<Vec<_>>();
        let reviewed_file_paths = HashSet::new();
        let rows_per_file = DIFF_FILE_HEADER_ROWS + hunk_count * (lines_per_hunk + 1);

        let started_at = Instant::now();
        let row_count = continuous_diff_row_count(
            &files,
            &diffs,
            &visible_file_indices,
            &reviewed_file_paths,
            &[],
            None,
            None,
            None,
            None,
        );
        let elapsed = started_at.elapsed();

        assert_eq!(row_count, file_count * rows_per_file);
        assert!(
            elapsed < Duration::from_millis(250),
            "large diff row counting took {elapsed:?}"
        );
        assert_eq!(
            continuous_diff_file_row_index(
                &files,
                &diffs,
                &visible_file_indices,
                &reviewed_file_paths,
                file_count - 1,
                &[],
                None,
                None,
                None,
                None,
            ),
            Some((file_count - 1) * rows_per_file)
        );
        assert_eq!(
            continuous_diff_hunk_row_index(
                &files,
                &diffs,
                &visible_file_indices,
                &reviewed_file_paths,
                file_count - 1,
                hunk_count - 1,
                &[],
                None,
                None,
                None,
                None,
            ),
            Some(
                (file_count - 1) * rows_per_file
                    + DIFF_FILE_HEADER_ROWS
                    + (hunk_count - 1) * (lines_per_hunk + 1)
            )
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

    fn large_test_diff(hunk_count: usize, lines_per_hunk: usize) -> ParsedDiff {
        let mut hunks = Vec::new();

        for hunk_index in 0..hunk_count {
            let line_start = (hunk_index * lines_per_hunk + 1) as u32;
            let lines = (0..lines_per_hunk)
                .map(|line_index| {
                    let line_number = line_start + line_index as u32;
                    DiffLine {
                        kind: DiffLineKind::Context,
                        old_line: Some(line_number),
                        new_line: Some(line_number),
                        text: format!("line {line_number}"),
                        syntax_highlights: Vec::new(),
                    }
                })
                .collect();

            hunks.push(DiffHunk {
                header: format!(
                    "@@ -{line_start},{lines_per_hunk} +{line_start},{lines_per_hunk} @@"
                ),
                old_start: line_start,
                old_lines: lines_per_hunk as u32,
                new_start: line_start,
                new_lines: lines_per_hunk as u32,
                lines,
            });
        }

        ParsedDiff { hunks }
    }
}
