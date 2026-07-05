use std::time::{Duration, Instant};

use gpui::{ListAlignment, ListOffset, px};
use harbor_domain::{
    FileStatus, FileViewedState, ReviewCommentRange, ReviewSide, ReviewThreadState,
};

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
        2,
        DiffListItem::ReviewComposer {
            file_index: 0,
            hunk_index: 0,
            line_index: 0,
        },
    );

    assert_eq!(
        diff_list_splice(&previous_items, &next_items),
        Some((2..2, 1))
    );
}

#[test]
fn sync_diff_list_state_preserves_scroll_top_when_inline_composer_is_inserted() {
    let mut previous_items = vec![
        DiffListItem::FileHeader { file_index: 0 },
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
        2,
        DiffListItem::ReviewComposer {
            file_index: 0,
            hunk_index: 0,
            line_index: 0,
        },
    );
    let list_state = ListState::new(previous_items.len(), ListAlignment::Top, px(0.0));
    list_state.scroll_to(ListOffset {
        item_ix: 3,
        offset_in_item: px(0.0),
    });

    sync_diff_list_state(&list_state, &mut previous_items, next_items.clone());

    assert_eq!(list_state.item_count(), next_items.len());
    assert_eq!(previous_items, next_items);
    assert_eq!(list_state.logical_scroll_top().item_ix, 4);
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
    let items_per_file = 1 + hunk_count * lines_per_hunk;

    let started_at = Instant::now();
    let items = continuous_diff_items(test_layout_input(
        &files,
        &diffs,
        &visible_file_indices,
        &reviewed_file_paths,
    ));
    let item_count = items.len();
    let elapsed = started_at.elapsed();

    assert_eq!(item_count, file_count * items_per_file);
    assert!(
        elapsed < Duration::from_millis(250),
        "large diff item building took {elapsed:?}"
    );
    assert_eq!(
        diff_file_item_index(&items, file_count - 1),
        Some((file_count - 1) * items_per_file)
    );
    assert_eq!(
        diff_hunk_item_index(&items, file_count - 1, hunk_count - 1),
        Some((file_count - 1) * items_per_file + 1 + (hunk_count - 1) * lines_per_hunk)
    );
}

#[test]
fn finds_file_and_diff_section_items_across_missing_patches() {
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
    let items = continuous_diff_items(test_layout_input(
        &files,
        &diffs,
        &visible_file_indices,
        &reviewed_file_paths,
    ));

    assert_eq!(diff_file_item_index(&items, 2), Some(5));
    assert_eq!(diff_hunk_item_index(&items, 2, 0), Some(6));
    assert_eq!(diff_hunk_item_index(&items, 1, 0), None);
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
        5
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
        continuous_diff_section_for_item(layout_input, &items, 4,),
        Some(DiffFileSection {
            file_index: 1,
            header_item_index: 3,
            reviewed: false,
        })
    );
    assert_eq!(
        continuous_diff_section_for_item(layout_input, &items, 6,),
        Some(DiffFileSection {
            file_index: 2,
            header_item_index: 5,
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
        Some(3)
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

#[test]
fn places_loaded_review_thread_range_on_added_line() {
    let file = test_file("src/review_thread_comments_125.rs");
    let diff = parse_unified_diff(
        "@@ -0,0 +1,3 @@\n+pub fn review_thread_comment_target() -> &'static str {\n+    \"single thread with many replies\"\n+}",
    );
    let mut thread = test_review_thread(ReviewThreadState::Unresolved);
    thread.path = "src/review_thread_comments_125.rs".to_string();
    thread.range = Some(ReviewCommentRange {
        path: "src/review_thread_comments_125.rs".to_string(),
        line: 2,
        side: ReviewSide::Right,
        start_line: Some(2),
        start_side: None,
    });
    let files = vec![file];
    let diffs = vec![Some(diff)];
    let visible_file_indices = vec![0];
    let reviewed_file_paths = HashSet::new();

    assert!(
        continuous_diff_items(ContinuousDiffLayoutInput {
            files: &files,
            diffs: &diffs,
            visible_file_indices: &visible_file_indices,
            reviewed_file_paths: &reviewed_file_paths,
            review_threads: &[thread],
            review_composer: None,
        })
        .contains(&DiffListItem::ReviewThread {
            file_index: 0,
            hunk_index: 0,
            line_index: 1,
            thread_id: "thread-1".to_string(),
        })
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
        viewed_state: FileViewedState::Unviewed,
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
            header: format!("@@ -{line_start},{lines_per_hunk} +{line_start},{lines_per_hunk} @@"),
            old_start: line_start,
            old_lines: lines_per_hunk as u32,
            new_start: line_start,
            new_lines: lines_per_hunk as u32,
            lines,
        });
    }

    ParsedDiff { hunks }
}
