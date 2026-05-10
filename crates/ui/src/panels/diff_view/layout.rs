use std::{collections::HashSet, ops::Range};

use gpui::ListState;
use harbor_domain::{DiffFile, ReviewThread};

use crate::{
    diff::ParsedDiff,
    diff_reviews::{anchored_review_threads, review_threads_for_line},
    workspace::ReviewComposer,
};

use super::{
    LINE_NUMBER_DIGIT_WIDTH, LINE_NUMBER_PADDING, MIN_LINE_NUMBER_WIDTH,
    inline_review_layout::review_comment_range_matches_file,
};

#[derive(Clone, Copy)]
pub(crate) struct ContinuousDiffLayoutInput<'a> {
    pub(crate) files: &'a [DiffFile],
    pub(crate) diffs: &'a [Option<ParsedDiff>],
    pub(crate) visible_file_indices: &'a [usize],
    pub(crate) reviewed_file_paths: &'a HashSet<String>,
    pub(crate) review_threads: &'a [ReviewThread],
    pub(crate) review_composer: Option<&'a ReviewComposer>,
}

impl<'a> ContinuousDiffLayoutInput<'a> {
    fn review_controls(self) -> ReviewLayoutControls<'a> {
        ReviewLayoutControls {
            review_threads: self.review_threads,
            review_composer: self.review_composer,
        }
    }
}

#[derive(Clone, Copy)]
struct ReviewLayoutControls<'a> {
    review_threads: &'a [ReviewThread],
    review_composer: Option<&'a ReviewComposer>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum DiffListItem {
    FileHeader {
        file_index: usize,
    },
    Hunk {
        file_index: usize,
        hunk_index: usize,
    },
    Line {
        file_index: usize,
        hunk_index: usize,
        line_index: usize,
    },
    ReviewComposer {
        file_index: usize,
        hunk_index: usize,
        line_index: usize,
    },
    ReviewThread {
        file_index: usize,
        hunk_index: usize,
        line_index: usize,
        thread_id: String,
    },
    DiffUnavailable {
        file_index: usize,
    },
}

pub(crate) fn continuous_diff_items(input: ContinuousDiffLayoutInput<'_>) -> Vec<DiffListItem> {
    let mut items = Vec::new();

    for file_index in input.visible_file_indices {
        let Some(file) = input.files.get(*file_index) else {
            continue;
        };

        items.push(DiffListItem::FileHeader {
            file_index: *file_index,
        });

        if file_is_reviewed(file, input.reviewed_file_paths) {
            continue;
        }

        let Some(diff) = parsed_diff_for_file(input.diffs, *file_index) else {
            items.push(DiffListItem::DiffUnavailable {
                file_index: *file_index,
            });
            continue;
        };

        diff_body_items(&mut items, diff, file, *file_index, input.review_controls());
    }

    items
}

pub(crate) fn sync_diff_list_state(
    list_state: &ListState,
    previous_items: &mut Vec<DiffListItem>,
    next_items: Vec<DiffListItem>,
) {
    if list_state.item_count() != previous_items.len() {
        let current_item_count = list_state.item_count();
        list_state.splice(0..current_item_count, next_items.len());
        *previous_items = next_items;
        return;
    }

    if previous_items == &next_items {
        return;
    }

    if let Some((old_range, inserted_item_count)) = diff_list_splice(previous_items, &next_items) {
        list_state.splice(old_range, inserted_item_count);
    }
    *previous_items = next_items;
}

fn diff_list_splice(
    previous_items: &[DiffListItem],
    next_items: &[DiffListItem],
) -> Option<(Range<usize>, usize)> {
    let prefix_len = previous_items
        .iter()
        .zip(next_items)
        .take_while(|(previous, next)| previous == next)
        .count();

    if prefix_len == previous_items.len() && prefix_len == next_items.len() {
        return None;
    }

    let mut previous_suffix_start = previous_items.len();
    let mut next_suffix_start = next_items.len();
    while previous_suffix_start > prefix_len
        && next_suffix_start > prefix_len
        && previous_items[previous_suffix_start - 1] == next_items[next_suffix_start - 1]
    {
        previous_suffix_start -= 1;
        next_suffix_start -= 1;
    }

    Some((
        prefix_len..previous_suffix_start,
        next_suffix_start - prefix_len,
    ))
}

pub(crate) fn continuous_diff_file_item_index(
    input: ContinuousDiffLayoutInput<'_>,
    target_file_index: usize,
) -> Option<usize> {
    continuous_diff_items(input).iter().position(|item| {
        matches!(item, DiffListItem::FileHeader { file_index } if *file_index == target_file_index)
    })
}

pub(crate) fn continuous_diff_hunk_item_index(
    input: ContinuousDiffLayoutInput<'_>,
    target_file_index: usize,
    target_hunk_index: usize,
) -> Option<usize> {
    continuous_diff_items(input).iter().position(|item| {
        matches!(
            item,
            DiffListItem::Hunk {
                file_index,
                hunk_index,
            } if *file_index == target_file_index && *hunk_index == target_hunk_index
        )
    })
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

#[derive(Debug, PartialEq, Eq)]
pub(super) struct DiffFileSection {
    pub(super) file_index: usize,
    pub(super) header_item_index: usize,
    pub(super) hunk_count: Option<usize>,
    pub(super) reviewed: bool,
}

pub(super) fn continuous_diff_section_for_item(
    input: ContinuousDiffLayoutInput<'_>,
    items: &[DiffListItem],
    target_item_index: usize,
) -> Option<DiffFileSection> {
    let target_item_index = target_item_index.min(items.len().checked_sub(1)?);

    for header_item_index in (0..=target_item_index).rev() {
        if let DiffListItem::FileHeader { file_index } = items[header_item_index] {
            let file = input.files.get(file_index)?;
            return Some(DiffFileSection {
                file_index,
                header_item_index,
                hunk_count: parsed_diff_for_file(input.diffs, file_index)
                    .map(|diff| diff.hunks.len()),
                reviewed: file_is_reviewed(file, input.reviewed_file_paths),
            });
        }
    }

    None
}

fn diff_body_items(
    items: &mut Vec<DiffListItem>,
    diff: &ParsedDiff,
    file: &DiffFile,
    file_index: usize,
    controls: ReviewLayoutControls<'_>,
) {
    let anchored_threads = anchored_review_threads(file, controls.review_threads);

    for (hunk_index, hunk) in diff.hunks.iter().enumerate() {
        items.push(DiffListItem::Hunk {
            file_index,
            hunk_index,
        });

        for (line_index, line) in hunk.lines.iter().enumerate() {
            items.push(DiffListItem::Line {
                file_index,
                hunk_index,
                line_index,
            });

            if controls.review_composer.is_some_and(|composer| {
                review_comment_range_matches_file(file, &composer.range)
                    && composer.anchor.hunk_index == hunk_index
                    && composer.anchor.line_index == line_index
            }) {
                items.push(DiffListItem::ReviewComposer {
                    file_index,
                    hunk_index,
                    line_index,
                });
            }

            for thread in review_threads_for_line(&anchored_threads, line) {
                items.push(DiffListItem::ReviewThread {
                    file_index,
                    hunk_index,
                    line_index,
                    thread_id: thread.id.clone(),
                });
            }
        }
    }
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

    use gpui::{ListAlignment, ListOffset, px};
    use harbor_domain::{FileStatus, ReviewThreadState};

    use crate::{
        diff::{DiffHunk, DiffLine, DiffLineKind, ParsedDiff, parse_unified_diff},
        panels::diff_view::inline_review_layout::review_line_target_for_line,
        test_fixtures::review_thread as test_review_thread,
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
    fn keeps_three_digit_line_numbers_on_one_visual_line() {
        let diff = parse_unified_diff("@@ -143,1 +143,1 @@\n context\n");

        assert_eq!(
            line_number_width_for_diff(&diff),
            3.0 * LINE_NUMBER_DIGIT_WIDTH + LINE_NUMBER_PADDING
        );
        assert!(line_number_width_for_diff(&diff) >= 40.0);
    }

    #[test]
    fn calculates_minimal_diff_list_splice_for_inline_composer() {
        let previous_items = vec![
            DiffListItem::FileHeader { file_index: 0 },
            DiffListItem::Hunk {
                file_index: 0,
                hunk_index: 0,
            },
            DiffListItem::Line {
                file_index: 0,
                hunk_index: 0,
                line_index: 0,
            },
            DiffListItem::Line {
                file_index: 0,
                hunk_index: 0,
                line_index: 1,
            },
            DiffListItem::FileHeader { file_index: 1 },
        ];
        let mut next_items = previous_items.clone();
        next_items.insert(
            3,
            DiffListItem::ReviewComposer {
                file_index: 0,
                hunk_index: 0,
                line_index: 0,
            },
        );

        assert_eq!(
            diff_list_splice(&previous_items, &next_items),
            Some((3..3, 1))
        );
    }

    #[test]
    fn sync_diff_list_state_preserves_scroll_top_when_inline_composer_is_inserted() {
        let mut previous_items = vec![
            DiffListItem::FileHeader { file_index: 0 },
            DiffListItem::Hunk {
                file_index: 0,
                hunk_index: 0,
            },
            DiffListItem::Line {
                file_index: 0,
                hunk_index: 0,
                line_index: 0,
            },
            DiffListItem::Line {
                file_index: 0,
                hunk_index: 0,
                line_index: 1,
            },
            DiffListItem::FileHeader { file_index: 1 },
        ];
        let mut next_items = previous_items.clone();
        next_items.insert(
            3,
            DiffListItem::ReviewComposer {
                file_index: 0,
                hunk_index: 0,
                line_index: 0,
            },
        );
        let list_state = ListState::new(previous_items.len(), ListAlignment::Top, px(0.0));
        list_state.scroll_to(ListOffset {
            item_ix: 4,
            offset_in_item: px(0.0),
        });

        sync_diff_list_state(&list_state, &mut previous_items, next_items.clone());

        assert_eq!(list_state.item_count(), next_items.len());
        assert_eq!(previous_items, next_items);
        assert_eq!(list_state.logical_scroll_top().item_ix, 5);
        assert_eq!(list_state.logical_scroll_top().offset_in_item, px(0.0));
    }

    #[test]
    fn builds_diff_items_across_visible_files() {
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
            continuous_diff_items(test_layout_input(
                &files,
                &diffs,
                &visible_file_indices,
                &reviewed_file_paths
            )),
            vec![
                DiffListItem::FileHeader { file_index: 0 },
                DiffListItem::Hunk {
                    file_index: 0,
                    hunk_index: 0,
                },
                DiffListItem::Line {
                    file_index: 0,
                    hunk_index: 0,
                    line_index: 0,
                },
                DiffListItem::Line {
                    file_index: 0,
                    hunk_index: 0,
                    line_index: 1,
                },
                DiffListItem::FileHeader { file_index: 1 },
                DiffListItem::DiffUnavailable { file_index: 1 },
                DiffListItem::FileHeader { file_index: 2 },
                DiffListItem::Hunk {
                    file_index: 2,
                    hunk_index: 0,
                },
                DiffListItem::Line {
                    file_index: 2,
                    hunk_index: 0,
                    line_index: 0,
                },
            ]
        );
    }

    #[test]
    fn calculates_large_diff_items_with_linear_cost() {
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
        let items_per_file = 1 + hunk_count * (lines_per_hunk + 1);

        let started_at = Instant::now();
        let item_count = continuous_diff_items(test_layout_input(
            &files,
            &diffs,
            &visible_file_indices,
            &reviewed_file_paths,
        ))
        .len();
        let elapsed = started_at.elapsed();

        assert_eq!(item_count, file_count * items_per_file);
        assert!(
            elapsed < Duration::from_millis(250),
            "large diff item building took {elapsed:?}"
        );
        assert_eq!(
            continuous_diff_file_item_index(
                test_layout_input(&files, &diffs, &visible_file_indices, &reviewed_file_paths),
                file_count - 1,
            ),
            Some((file_count - 1) * items_per_file)
        );
        assert_eq!(
            continuous_diff_hunk_item_index(
                test_layout_input(&files, &diffs, &visible_file_indices, &reviewed_file_paths),
                file_count - 1,
                hunk_count - 1,
            ),
            Some((file_count - 1) * items_per_file + 1 + (hunk_count - 1) * (lines_per_hunk + 1))
        );
    }

    #[test]
    fn finds_file_and_hunk_items_across_missing_patches() {
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
            continuous_diff_file_item_index(
                test_layout_input(&files, &diffs, &visible_file_indices, &reviewed_file_paths),
                2,
            ),
            Some(6)
        );
        assert_eq!(
            continuous_diff_hunk_item_index(
                test_layout_input(&files, &diffs, &visible_file_indices, &reviewed_file_paths),
                2,
                0,
            ),
            Some(7)
        );
        assert_eq!(
            continuous_diff_hunk_item_index(
                test_layout_input(&files, &diffs, &visible_file_indices, &reviewed_file_paths),
                1,
                0,
            ),
            None
        );
    }

    #[test]
    fn collapses_reviewed_file_sections_in_diff_items() {
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
            continuous_diff_items(test_layout_input(
                &files,
                &diffs,
                &visible_file_indices,
                &reviewed_file_paths
            ))
            .len(),
            6
        );
        assert_eq!(
            continuous_diff_file_item_index(
                test_layout_input(&files, &diffs, &visible_file_indices, &reviewed_file_paths),
                2,
            ),
            Some(3)
        );
        assert_eq!(
            continuous_diff_hunk_item_index(
                test_layout_input(&files, &diffs, &visible_file_indices, &reviewed_file_paths),
                0,
                0,
            ),
            None
        );
        assert_eq!(
            continuous_diff_hunk_item_index(
                test_layout_input(&files, &diffs, &visible_file_indices, &reviewed_file_paths),
                2,
                0,
            ),
            Some(4)
        );
    }

    #[test]
    fn skips_hidden_files_when_building_diff_items() {
        let files = vec![test_file("src/hidden.rs"), test_file("src/visible.rs")];
        let diffs = vec![
            Some(parse_unified_diff("@@ -1 +1 @@\n hidden\n")),
            Some(parse_unified_diff("@@ -1 +1 @@\n visible\n")),
        ];
        let visible_file_indices = vec![1];
        let reviewed_file_paths = HashSet::new();

        assert_eq!(
            continuous_diff_items(test_layout_input(
                &files,
                &diffs,
                &visible_file_indices,
                &reviewed_file_paths
            )),
            vec![
                DiffListItem::FileHeader { file_index: 1 },
                DiffListItem::Hunk {
                    file_index: 1,
                    hunk_index: 0,
                },
                DiffListItem::Line {
                    file_index: 1,
                    hunk_index: 0,
                    line_index: 0,
                },
            ]
        );
    }

    #[test]
    fn finds_diff_section_for_item_across_file_boundaries() {
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
        let layout_input =
            test_layout_input(&files, &diffs, &visible_file_indices, &reviewed_file_paths);
        let items = continuous_diff_items(layout_input);

        assert_eq!(
            continuous_diff_section_for_item(layout_input, &items, 5,),
            Some(DiffFileSection {
                file_index: 1,
                header_item_index: 4,
                hunk_count: None,
                reviewed: false,
            })
        );
        assert_eq!(
            continuous_diff_section_for_item(layout_input, &items, 7,),
            Some(DiffFileSection {
                file_index: 2,
                header_item_index: 6,
                hunk_count: Some(1),
                reviewed: false,
            })
        );
    }

    #[test]
    fn places_inline_review_items_before_later_hunks() {
        let file = test_file("src/a.rs");
        let diff = parse_unified_diff("@@ -1 +1 @@\n context\n@@ -10 +10 @@\n later\n");
        let target = review_line_target_for_line(&file, 0, 0, &diff.hunks[0].lines[0])
            .expect("context line should be commentable");
        let composer = ReviewComposer {
            anchor: target.clone(),
            range: target.range,
        };
        let files = vec![file];
        let diffs = vec![Some(diff)];
        let visible_file_indices = vec![0];
        let reviewed_file_paths = HashSet::new();

        assert_eq!(
            continuous_diff_hunk_item_index(
                ContinuousDiffLayoutInput {
                    files: &files,
                    diffs: &diffs,
                    visible_file_indices: &visible_file_indices,
                    reviewed_file_paths: &reviewed_file_paths,
                    review_threads: &[],
                    review_composer: Some(&composer),
                },
                0,
                1,
            ),
            Some(4)
        );
    }

    #[test]
    fn places_inline_review_thread_items_after_matching_line() {
        let file = test_file("src/lib.rs");
        let diff = parse_unified_diff("@@ -12 +12,2 @@\n first line\n+added line\n");
        let target = review_line_target_for_line(&file, 0, 0, &diff.hunks[0].lines[0])
            .expect("context line should be commentable");
        let composer = ReviewComposer {
            anchor: target.clone(),
            range: target.range,
        };
        let thread = test_review_thread(ReviewThreadState::Unresolved);
        let files = vec![file];
        let diffs = vec![Some(diff)];
        let visible_file_indices = vec![0];
        let reviewed_file_paths = HashSet::new();

        assert_eq!(
            continuous_diff_items(ContinuousDiffLayoutInput {
                files: &files,
                diffs: &diffs,
                visible_file_indices: &visible_file_indices,
                reviewed_file_paths: &reviewed_file_paths,
                review_threads: std::slice::from_ref(&thread),
                review_composer: Some(&composer),
            }),
            vec![
                DiffListItem::FileHeader { file_index: 0 },
                DiffListItem::Hunk {
                    file_index: 0,
                    hunk_index: 0,
                },
                DiffListItem::Line {
                    file_index: 0,
                    hunk_index: 0,
                    line_index: 0,
                },
                DiffListItem::ReviewComposer {
                    file_index: 0,
                    hunk_index: 0,
                    line_index: 0,
                },
                DiffListItem::ReviewThread {
                    file_index: 0,
                    hunk_index: 0,
                    line_index: 0,
                    thread_id: "thread-1".to_string(),
                },
                DiffListItem::Line {
                    file_index: 0,
                    hunk_index: 0,
                    line_index: 1,
                },
            ]
        );
    }

    fn test_layout_input<'a>(
        files: &'a [DiffFile],
        diffs: &'a [Option<ParsedDiff>],
        visible_file_indices: &'a [usize],
        reviewed_file_paths: &'a HashSet<String>,
    ) -> ContinuousDiffLayoutInput<'a> {
        ContinuousDiffLayoutInput {
            files,
            diffs,
            visible_file_indices,
            reviewed_file_paths,
            review_threads: &[],
            review_composer: None,
        }
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
